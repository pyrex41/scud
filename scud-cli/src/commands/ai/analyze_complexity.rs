use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct ComplexityAnalysis {
    complexity: u32,
    reasoning: String,
}

pub async fn run(project_root: Option<PathBuf>, task_id: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let active_epic = storage
        .get_active_epic()?
        .ok_or_else(|| anyhow::anyhow!("No active epic. Run: scud use-tag <epic-tag>"))?;

    let mut all_tasks = storage.load_tasks()?;
    let epic = all_tasks
        .get_mut(&active_epic)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", active_epic))?;

    let client = LLMClient::new()?;

    // Determine which tasks to analyze
    let task_ids: Vec<String> = if let Some(id) = task_id {
        vec![id.to_string()]
    } else {
        epic.tasks.iter().map(|t| t.id.clone()).collect()
    };

    if task_ids.is_empty() {
        println!("{}", "No tasks to analyze".yellow());
        return Ok(());
    }

    println!(
        "{} {} task(s)...",
        "Analyzing complexity for".blue(),
        task_ids.len()
    );

    for id in task_ids {
        let task = epic
            .get_task_mut(&id)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found", id))?;

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );
        spinner.set_message(format!("Analyzing task {}: {}", id, task.title));
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let prompt =
            Prompts::analyze_complexity(&task.title, &task.description, task.details.as_deref());

        let analysis: ComplexityAnalysis = client.complete_json(&prompt).await?;

        task.complexity = analysis.complexity;
        task.complexity_analysis = Some(analysis.reasoning.clone());
        task.update();

        spinner.finish_with_message(format!(
            "{} Task {}: {} → complexity {}",
            "✓".green(),
            id.cyan(),
            task.title,
            analysis.complexity.to_string().yellow()
        ));

        if analysis.complexity > 13 {
            println!(
                "  {} Task complexity >13. Consider running: scud expand {}",
                "⚠".yellow(),
                id
            );
        }
    }

    // Get stats and tasks needing expansion before saving (to avoid borrow checker issues)
    let stats = epic.get_stats();
    let tasks_needing_expansion: Vec<_> = epic
        .get_tasks_needing_expansion()
        .iter()
        .map(|t| (t.id.clone(), t.title.clone(), t.complexity))
        .collect();

    storage.save_tasks(&all_tasks)?;

    println!("\n{}", "✅ Complexity analysis complete!".green().bold());

    // Show summary
    println!();
    println!(
        "{:<25} {}",
        "Total complexity:".yellow(),
        stats.total_complexity
    );

    if !tasks_needing_expansion.is_empty() {
        println!();
        println!(
            "{} {} task(s) with complexity >13:",
            "⚠".yellow(),
            tasks_needing_expansion.len()
        );
        for (id, title, complexity) in tasks_needing_expansion {
            println!("  {} {} [{}]", id.cyan(), title, complexity);
        }
        println!();
        println!("{}", "Run: scud expand --all".blue());
    }

    Ok(())
}
