use anyhow::Result;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

use crate::llm::{LLMClient, Prompts};
use crate::models::{Priority, Task, TaskStatus};
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

/// Result of expanding a single task
struct TaskExpansionResult {
    parent_id: String,
    parent_priority: Priority,
    expanded_tasks: Vec<ExpandedTask>,
}

/// Number of concurrent LLM requests
const CONCURRENCY: usize = 5;

pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    expand_all: bool,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let epic_tag = crate::commands::helpers::resolve_epic_tag(&storage, tag, true)?;

    let mut all_tasks = storage.load_tasks()?;
    let epic = all_tasks
        .get_mut(&epic_tag)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", epic_tag))?;

    let client = Arc::new(LLMClient::new()?);

    // Determine which tasks to expand and gather their data
    let tasks_to_expand: Vec<(String, String, String, Option<String>, u32, Priority)> =
        if let Some(id) = task_id {
            let task = epic
                .get_task(id)
                .ok_or_else(|| anyhow::anyhow!("Task {} not found", id))?;
            if !task.needs_expansion() {
                let reason = if task.is_expanded() {
                    "already expanded"
                } else if task.is_subtask() {
                    "is a subtask"
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
                return Ok(());
            }
            vec![(
                task.id.clone(),
                task.title.clone(),
                task.description.clone(),
                task.details.clone(),
                task.complexity,
                task.priority.clone(),
            )]
        } else if expand_all {
            epic.tasks
                .iter()
                .filter(|t| t.needs_expansion())
                .map(|t| {
                    (
                        t.id.clone(),
                        t.title.clone(),
                        t.description.clone(),
                        t.details.clone(),
                        t.complexity,
                        t.priority.clone(),
                    )
                })
                .collect()
        } else {
            anyhow::bail!("Specify a task ID or use --all to expand all tasks with complexity ≥3");
        };

    if tasks_to_expand.is_empty() {
        println!(
            "{}",
            "No tasks need expansion (all complexity <3 or already expanded)".green()
        );
        return Ok(());
    }

    let task_count = tasks_to_expand.len();
    println!(
        "{} {} task(s) with {} concurrent requests...",
        "Expanding".blue(),
        task_count,
        CONCURRENCY
    );

    // Set up multi-progress display
    let multi_progress = MultiProgress::new();
    let overall_progress = multi_progress.add(ProgressBar::new(task_count as u64));
    overall_progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.blue} [{bar:40.cyan/blue}] {pos}/{len} tasks")
            .unwrap()
            .progress_chars("█▓░"),
    );

    // Process tasks in parallel with bounded concurrency
    let results: Vec<Result<TaskExpansionResult, (String, anyhow::Error)>> =
        stream::iter(tasks_to_expand)
            .map(|(id, title, description, details, complexity, priority)| {
                let client = Arc::clone(&client);
                let mp = multi_progress.clone();
                let overall = overall_progress.clone();

                async move {
                    let spinner = mp.add(ProgressBar::new_spinner());
                    spinner.set_style(
                        ProgressStyle::default_spinner()
                            .template("{spinner:.blue} {msg}")
                            .unwrap(),
                    );
                    spinner.set_message(format!("Task {}: {}", id, title));
                    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

                    let recommended_subtasks =
                        Task::recommended_subtasks_for_complexity(complexity);
                    let prompt = Prompts::expand_task(
                        &title,
                        &description,
                        complexity,
                        details.as_deref(),
                        recommended_subtasks,
                    );

                    // Retry logic
                    let mut last_error = None;
                    for attempt in 1..=3 {
                        match client.complete_json::<Vec<ExpandedTask>>(&prompt).await {
                            Ok(expanded) => {
                                spinner.finish_and_clear();
                                overall.inc(1);
                                return Ok(TaskExpansionResult {
                                    parent_id: id,
                                    parent_priority: priority,
                                    expanded_tasks: expanded,
                                });
                            }
                            Err(e) => {
                                last_error = Some(e);
                                if attempt < 3 {
                                    spinner.set_message(format!(
                                        "Task {} (retry {}/3): {}",
                                        id,
                                        attempt + 1,
                                        title
                                    ));
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                }
                            }
                        }
                    }

                    spinner.finish_and_clear();
                    overall.inc(1);
                    Err((id, last_error.unwrap()))
                }
            })
            .buffer_unordered(CONCURRENCY)
            .collect()
            .await;

    overall_progress.finish_and_clear();

    // Process results and create subtasks
    let mut success_count = 0;
    let mut error_count = 0;
    let mut total_subtasks = 0;

    for result in results {
        match result {
            Ok(expansion) => {
                let parent_id = &expansion.parent_id;
                let subtask_count = expansion.expanded_tasks.len();

                println!(
                    "{} Task {} expanded into {} subtasks",
                    "✓".green(),
                    parent_id.cyan(),
                    subtask_count
                );

                // Create subtasks
                let mut new_subtask_ids = Vec::new();
                for (idx, expanded) in expansion.expanded_tasks.iter().enumerate() {
                    let new_id = format!("{}.{}", parent_id, idx + 1);

                    let priority = if !expanded.priority.is_empty() {
                        match expanded.priority.to_lowercase().as_str() {
                            "high" => Priority::High,
                            "low" => Priority::Low,
                            _ => Priority::Medium,
                        }
                    } else {
                        expansion.parent_priority.clone()
                    };

                    let mut new_task = Task::new(
                        new_id.clone(),
                        expanded.title.clone(),
                        expanded.description.clone(),
                    );
                    new_task.priority = priority;
                    new_task.complexity = 0;
                    new_task.parent_id = Some(parent_id.clone());

                    // Map dependency references to nested IDs
                    new_task.dependencies = expanded
                        .dependencies
                        .iter()
                        .filter_map(|dep| {
                            if let Ok(dep_idx) = dep.parse::<usize>() {
                                if dep_idx > 0 && dep_idx <= idx + 1 {
                                    Some(format!("{}.{}", parent_id, dep_idx))
                                } else {
                                    None
                                }
                            } else {
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

                // Update parent task
                if let Some(parent_task) = epic.get_task_mut(parent_id) {
                    parent_task.status = TaskStatus::Expanded;
                    parent_task.subtasks = new_subtask_ids;
                    parent_task.update();
                }

                total_subtasks += subtask_count;
                success_count += 1;
            }
            Err((id, e)) => {
                println!("{} Task {} failed: {}", "✗".red(), id.cyan(), e);
                error_count += 1;
            }
        }
    }

    storage.save_tasks(&all_tasks)?;

    // Summary
    println!("\n{}", "✅ Task expansion complete!".green().bold());
    println!();
    println!(
        "{:<25} {} ({} succeeded, {} failed)",
        "Expanded:".yellow(),
        task_count,
        success_count.to_string().green(),
        if error_count > 0 {
            error_count.to_string().red()
        } else {
            error_count.to_string().normal()
        }
    );
    println!(
        "{:<25} {}",
        "Total subtasks created:".yellow(),
        total_subtasks
    );
    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review tasks: scud list");
    println!("  2. Continue with /tm-architect");
    println!();

    Ok(())
}
