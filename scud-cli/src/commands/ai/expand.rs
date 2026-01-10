#![allow(clippy::type_complexity)]

use anyhow::Result;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::llm::{LLMClient, Prompts};
use crate::models::{IdFormat, Priority, Task, TaskStatus};
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
    tag: String,
    parent_id: String,
    parent_priority: Priority,
    expanded_tasks: Vec<ExpandedTask>,
}

/// Number of concurrent LLM requests
const CONCURRENCY: usize = 10;

pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    all_tags: bool,
    tag: Option<&str>,
    no_guidance: bool,
    model: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());
    let mut all_tasks = storage.load_tasks()?;
    let model_str = model.map(|s| s.to_string());

    // Use project_root for LLM client to find config.toml in correct location
    let client = Arc::new(if let Some(root) = project_root.clone() {
        LLMClient::new_with_project_root(root)?
    } else {
        LLMClient::new()?
    });

    // Load guidance unless disabled
    let guidance = if no_guidance {
        None
    } else {
        match storage.load_guidance() {
            Ok(g) if !g.is_empty() => {
                println!("{}", "Loading project guidance...".blue());
                Some(g)
            }
            Ok(_) => None,
            Err(e) => {
                eprintln!("{} Failed to load guidance: {}", "Warning:".yellow(), e);
                None
            }
        }
    };

    // Determine which tags to process
    let tags_to_process: Vec<String> = if all_tags {
        // --all flag: expand across ALL tags
        all_tasks.keys().cloned().collect()
    } else {
        // Default or --tag: expand in current/specified tag only
        let epic_tag = crate::commands::helpers::resolve_group_tag(&storage, tag, true)?;
        if !all_tasks.contains_key(&epic_tag) {
            anyhow::bail!("Tag '{}' not found", epic_tag);
        }
        vec![epic_tag]
    };

    // Collect tasks to expand from all relevant tags
    // Format: (tag, task_id, title, description, details, complexity, priority)
    let mut tasks_to_expand: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        u32,
        Priority,
    )> = Vec::new();

    if let Some(id) = task_id {
        // Specific task requested - find it in the appropriate tag
        let search_tag = if tags_to_process.len() == 1 {
            tags_to_process[0].clone()
        } else {
            // When --all is used with --task, search all tags for the task
            let mut found_tag = None;
            for tag_name in &tags_to_process {
                if let Some(phase) = all_tasks.get(tag_name) {
                    if phase.get_task(id).is_some() {
                        found_tag = Some(tag_name.clone());
                        break;
                    }
                }
            }
            found_tag.ok_or_else(|| anyhow::anyhow!("Task {} not found in any tag", id))?
        };

        let epic = all_tasks
            .get(&search_tag)
            .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found", search_tag))?;
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
        tasks_to_expand.push((
            search_tag,
            task.id.clone(),
            task.title.clone(),
            task.description.clone(),
            task.details.clone(),
            task.complexity,
            task.priority.clone(),
        ));
    } else {
        // No specific task - expand all matching tasks in the target tag(s)
        for tag_name in &tags_to_process {
            if let Some(phase) = all_tasks.get(tag_name) {
                for task in &phase.tasks {
                    if task.needs_expansion() {
                        tasks_to_expand.push((
                            tag_name.clone(),
                            task.id.clone(),
                            task.title.clone(),
                            task.description.clone(),
                            task.details.clone(),
                            task.complexity,
                            task.priority.clone(),
                        ));
                    }
                }
            }
        }
    };

    if tasks_to_expand.is_empty() {
        println!(
            "{}",
            "No tasks need expansion (all complexity <5 or already expanded)".green()
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

    // Clone guidance and model for async blocks
    let guidance_arc = Arc::new(guidance);
    let model_arc = Arc::new(model_str);

    // Process tasks in parallel with bounded concurrency
    let results: Vec<Result<TaskExpansionResult, (String, String, anyhow::Error)>> =
        stream::iter(tasks_to_expand)
            .map(
                |(tag, id, title, description, details, complexity, priority)| {
                    let client = Arc::clone(&client);
                    let mp = multi_progress.clone();
                    let overall = overall_progress.clone();
                    let guidance_clone = Arc::clone(&guidance_arc);
                    let model_ref = Arc::clone(&model_arc);

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
                            guidance_clone.as_deref(),
                        );

                        // Retry logic
                        let mut last_error = None;
                        for attempt in 1..=3 {
                            match client.complete_json_with_model::<Vec<ExpandedTask>>(&prompt, model_ref.as_deref()).await {
                                Ok(expanded) => {
                                    spinner.finish_and_clear();
                                    overall.inc(1);
                                    return Ok(TaskExpansionResult {
                                        tag,
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
                        Err((tag, id, last_error.unwrap()))
                    }
                },
            )
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
                let tag = &expansion.tag;
                let parent_id = &expansion.parent_id;
                let subtask_count = expansion.expanded_tasks.len();

                println!(
                    "{} Task {} expanded into {} subtasks",
                    "✓".green(),
                    parent_id.cyan(),
                    subtask_count
                );

                // Get the phase for this task's tag
                let epic = all_tasks
                    .get_mut(tag)
                    .expect("Tag should exist since task came from it");

                // Check if this phase uses UUID format
                let use_uuid = epic.id_format == IdFormat::Uuid;

                // Create subtasks
                // For UUID format, pre-generate all IDs so we can map dependencies
                let subtask_ids: Vec<String> = if use_uuid {
                    expansion
                        .expanded_tasks
                        .iter()
                        .map(|_| Uuid::new_v4().to_string().replace("-", ""))
                        .collect()
                } else {
                    expansion
                        .expanded_tasks
                        .iter()
                        .enumerate()
                        .map(|(idx, _)| format!("{}.{}", parent_id, idx + 1))
                        .collect()
                };

                let mut new_subtask_ids = Vec::new();
                for (idx, expanded) in expansion.expanded_tasks.iter().enumerate() {
                    let new_id = subtask_ids[idx].clone();

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

                    // Map dependency references to actual subtask IDs
                    // LLM returns dependencies as 1-indexed references to other subtasks
                    new_task.dependencies = expanded
                        .dependencies
                        .iter()
                        .filter_map(|dep| {
                            if let Ok(dep_idx) = dep.parse::<usize>() {
                                // Map 1-indexed reference to actual subtask ID
                                if dep_idx > 0 && dep_idx <= idx + 1 {
                                    Some(subtask_ids[dep_idx - 1].clone())
                                } else {
                                    None
                                }
                            } else {
                                // Already a full ID reference
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
            Err((_tag, id, e)) => {
                println!("{} Task {} failed: {}", "✗".red(), id.cyan(), e);
                error_count += 1;
            }
        }
    }

    storage.save_tasks(&all_tasks)?;

    // Check if there are multiple phases for cross-tag hint
    let has_multiple_phases = all_tasks.len() > 1;

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

    // Hint about dependency analysis if we expanded tasks and have multiple phases
    if success_count > 0 && has_multiple_phases {
        println!();
        println!(
            "{} New subtasks may have cross-phase dependencies.",
            "Tip:".cyan()
        );
        println!("     Run '{}' to check.", "scud reanalyze-deps".green());
    }

    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review tasks: scud list");
    if success_count > 0 && has_multiple_phases {
        println!("  2. Check dependencies: scud reanalyze-deps");
        println!("  3. Start working: scud next");
    } else {
        println!("  2. Start working: scud next");
    }
    println!();

    Ok(())
}
