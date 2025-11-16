use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};
use crate::models::{Priority, Task};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct ExpandedTask {
    title: String,
    description: String,
    priority: String,
    complexity: u32,
    #[serde(default)]
    dependencies: Vec<String>,
}

pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    expand_all: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let active_epic = storage
        .get_active_epic()?
        .ok_or_else(|| anyhow::anyhow!("No active epic. Run: scud use-tag <epic-tag>"))?;

    let mut all_tasks = storage.load_tasks()?;
    let epic = all_tasks
        .get_mut(&active_epic)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", active_epic))?;

    let client = LLMClient::new()?;

    // Determine which tasks to expand
    let task_ids: Vec<String> = if let Some(id) = task_id {
        vec![id.to_string()]
    } else if expand_all {
        epic.tasks
            .iter()
            .filter(|t| t.needs_expansion())
            .map(|t| t.id.clone())
            .collect()
    } else {
        anyhow::bail!("Specify a task ID or use --all to expand all tasks with complexity >13");
    };

    if task_ids.is_empty() {
        println!("{}", "No tasks need expansion (all complexity ≤13)".green());
        return Ok(());
    }

    println!(
        "{} {} task(s)...",
        "Expanding".blue(),
        task_ids.len()
    );

    for id in task_ids {
        let task = epic
            .get_task(&id)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found", id))?;

        if !task.needs_expansion() {
            println!(
                "{} Task {} doesn't need expansion (complexity: {})",
                "⊘".yellow(),
                id.cyan(),
                task.complexity
            );
            continue;
        }

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );
        spinner.set_message(format!("Expanding task {}: {}", id, task.title));
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        let prompt = Prompts::expand_task(
            &task.title,
            &task.description,
            task.complexity,
            task.details.as_deref(),
        );

        let expanded_tasks: Vec<ExpandedTask> = client.complete_json(&prompt).await?;

        spinner.finish_with_message(format!(
            "{} Task {} expanded into {} subtasks",
            "✓".green(),
            id.cyan(),
            expanded_tasks.len()
        ));

        // Get the highest current task ID to start numbering from
        let max_id: u32 = epic
            .tasks
            .iter()
            .filter_map(|t| t.id.parse::<u32>().ok())
            .max()
            .unwrap_or(0);

        // Create new subtasks
        let mut new_subtask_ids = Vec::new();
        for (idx, expanded) in expanded_tasks.iter().enumerate() {
            let new_id = (max_id + idx as u32 + 1).to_string();

            let priority = match expanded.priority.to_lowercase().as_str() {
                "high" => Priority::High,
                "low" => Priority::Low,
                _ => Priority::Medium,
            };

            let mut new_task = Task::new(
                new_id.clone(),
                expanded.title.clone(),
                expanded.description.clone(),
            );
            new_task.complexity = expanded.complexity;
            new_task.priority = priority;

            // Map dependency references
            // If dependencies refer to indices in the expanded array, map them to actual IDs
            new_task.dependencies = expanded
                .dependencies
                .iter()
                .filter_map(|dep| {
                    if let Ok(dep_idx) = dep.parse::<usize>() {
                        new_subtask_ids.get(dep_idx).cloned()
                    } else {
                        Some(dep.clone())
                    }
                })
                .collect();

            new_subtask_ids.push(new_id.clone());
            epic.add_task(new_task);

            println!(
                "  {} Created subtask {}: {} [complexity: {}]",
                "+".green(),
                new_id.cyan(),
                expanded.title,
                expanded.complexity.to_string().yellow()
            );
        }

        // Update original task to mark it as expanded (parent)
        let original_task = epic.get_task_mut(&id).unwrap();
        original_task.title = format!("[PARENT] {}", original_task.title);
        original_task.description = format!(
            "{}\n\n[This task has been expanded into subtasks: {}]",
            original_task.description,
            new_subtask_ids.join(", ")
        );
        original_task.update();
    }

    storage.save_tasks(&all_tasks)?;

    println!("\n{}", "✅ Task expansion complete!".green().bold());
    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review tasks: scud list");
    println!("  2. Continue with /tm-architect");
    println!();

    Ok(())
}
