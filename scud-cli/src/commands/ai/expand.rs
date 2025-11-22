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
    #[serde(default)]
    priority: String,
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
        anyhow::bail!("Specify a task ID or use --all to expand all tasks with complexity ≥3");
    };

    if task_ids.is_empty() {
        println!(
            "{}",
            "No tasks need expansion (all complexity <3 or already expanded)".green()
        );
        return Ok(());
    }

    println!("{} {} task(s)...", "Expanding".blue(), task_ids.len());

    for id in task_ids {
        let task = epic
            .get_task(&id)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found", id))?;

        if !task.needs_expansion() {
            let reason = if task.title.starts_with("[PARENT]") {
                "already expanded"
            } else {
                "complexity too low"
            };
            println!(
                "{} Task {} doesn't need expansion ({}, complexity: {})",
                "⊘".yellow(),
                id.cyan(),
                reason,
                task.complexity
            );
            continue;
        }

        let recommended_subtasks = task.recommended_subtasks();

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
            recommended_subtasks,
        );

        // Retry logic for LLM calls
        let mut expanded_tasks: Option<Vec<ExpandedTask>> = None;
        let mut last_error = None;

        for attempt in 1..=3 {
            match client.complete_json::<Vec<ExpandedTask>>(&prompt).await {
                Ok(tasks) => {
                    expanded_tasks = Some(tasks);
                    break;
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 3 {
                        spinner.set_message(format!(
                            "Expanding task {} (attempt {}/3): {}",
                            id,
                            attempt + 1,
                            task.title
                        ));
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }

        let expanded_tasks = match expanded_tasks {
            Some(tasks) => tasks,
            None => {
                spinner.finish_with_message(format!(
                    "{} Task {} failed after 3 attempts",
                    "✗".red(),
                    id.cyan()
                ));
                return Err(last_error.unwrap());
            }
        };

        spinner.finish_with_message(format!(
            "{} Task {} expanded into {} subtasks",
            "✓".green(),
            id.cyan(),
            expanded_tasks.len()
        ));

        // Clone parent task priority before the loop
        let parent_priority = task.priority.clone();

        // Create new subtasks with nested IDs (e.g., 1.1, 1.2, 1.3)
        let mut new_subtask_ids = Vec::new();
        for (idx, expanded) in expanded_tasks.iter().enumerate() {
            let new_id = format!("{}.{}", id, idx + 1);

            let priority = if !expanded.priority.is_empty() {
                match expanded.priority.to_lowercase().as_str() {
                    "high" => Priority::High,
                    "low" => Priority::Low,
                    _ => Priority::Medium,
                }
            } else {
                parent_priority.clone()
            };

            let mut new_task = Task::new(
                new_id.clone(),
                expanded.title.clone(),
                expanded.description.clone(),
            );
            new_task.priority = priority;
            new_task.complexity = 0; // Subtasks don't have complexity

            // Map dependency references to nested IDs
            new_task.dependencies = expanded
                .dependencies
                .iter()
                .filter_map(|dep| {
                    // If dependency is a number, it refers to a subtask index
                    if let Ok(dep_idx) = dep.parse::<usize>() {
                        if dep_idx > 0 && dep_idx <= idx + 1 {
                            Some(format!("{}.{}", id, dep_idx))
                        } else {
                            None
                        }
                    } else {
                        // Otherwise it's a task ID reference
                        Some(dep.clone())
                    }
                })
                .collect();

            new_subtask_ids.push(new_id.clone());
            epic.add_task(new_task);

            println!(
                "  {} Created subtask {}: {}",
                "+".green(),
                new_id.cyan(),
                expanded.title
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
