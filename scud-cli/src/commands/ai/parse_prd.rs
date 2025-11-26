use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::llm::{LLMClient, Prompts};
use crate::models::{Epic, Priority, Task};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct ParsedTask {
    title: String,
    description: String,
    priority: String,
    complexity: u32,
    #[serde(default)]
    dependencies: Vec<String>,
}

pub async fn run(project_root: Option<PathBuf>, file_path: &Path, tag: &str) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Read the PRD file
    println!("{} {}", "Reading PRD from:".blue(), file_path.display());
    let prd_content = storage.read_file(file_path)?;

    // Create LLM client with proper project root
    let client = match project_root {
        Some(root) => LLMClient::new_with_project_root(root)?,
        None => LLMClient::new()?,
    };

    // Show progress
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    spinner.set_message("Parsing PRD with AI...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Call LLM to parse the PRD
    let prompt = Prompts::parse_prd(&prd_content);
    let parsed_tasks: Vec<ParsedTask> = client.complete_json(&prompt).await?;

    spinner.finish_with_message(format!(
        "{} Parsed {} tasks",
        "✓".green(),
        parsed_tasks.len()
    ));

    // Convert to our task model
    let mut group = Epic::new(tag.to_string());

    for (idx, parsed) in parsed_tasks.iter().enumerate() {
        let task_id = (idx + 1).to_string();

        let priority = match parsed.priority.to_lowercase().as_str() {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Medium,
        };

        let mut task = Task::new(
            task_id.clone(),
            parsed.title.clone(),
            parsed.description.clone(),
        );
        task.complexity = parsed.complexity;
        task.priority = priority;
        task.dependencies = parsed.dependencies.clone();

        group.add_task(task);
    }

    // Load existing tasks (propagate errors - don't silently swallow them)
    let mut all_tasks = storage.load_tasks()?;

    if all_tasks.contains_key(tag) {
        println!(
            "{}",
            format!("⚠ Task group '{}' already exists. Overwriting...", tag).yellow()
        );
    }

    all_tasks.insert(tag.to_string(), group);
    storage.save_tasks(&all_tasks)?;

    // Set as active group
    storage.set_active_group(tag)?;

    println!(
        "\n{}",
        "✅ PRD parsed and task group created!".green().bold()
    );
    println!();
    println!("{:<20} {}", "Tag:".yellow(), tag.cyan());
    println!("{:<20} {}", "Tasks created:".yellow(), parsed_tasks.len());
    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review tasks: scud list");
    println!("  2. Analyze complexity: scud analyze-complexity");
    println!("  3. Use /scud-architect to add technical details");
    println!();

    Ok(())
}
