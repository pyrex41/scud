use anyhow::Result;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

use crate::llm::{LLMClient, Prompts};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct ComplexityAnalysis {
    complexity: u32,
    reasoning: String,
}

/// Result of analyzing a single task's complexity
struct TaskAnalysisResult {
    id: String,
    title: String,
    complexity: u32,
    #[allow(dead_code)]
    reasoning: String, // Parsed from LLM response but not stored
}

/// Number of concurrent LLM requests
const CONCURRENCY: usize = 5;

pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    tag: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());
    let group_tag = crate::commands::helpers::resolve_group_tag(&storage, tag, true)?;
    let model_str = model.map(|s| s.to_string());

    let mut all_tasks = storage.load_tasks()?;
    let group = all_tasks
        .get_mut(&group_tag)
        .ok_or_else(|| anyhow::anyhow!("Task group '{}' not found", group_tag))?;

    // Use project_root for LLM config resolution
    let client = Arc::new(match project_root {
        Some(root) => LLMClient::new_with_project_root(root)?,
        None => LLMClient::new()?,
    });

    // Show model info
    let model_info = client.smart_model_info(model);
    println!("{} {}", "Using".blue(), model_info.to_string().cyan());

    // Determine which tasks to analyze
    let tasks_to_analyze: Vec<(String, String, String, Option<String>)> = if let Some(id) = task_id
    {
        let task = group
            .get_task(id)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found", id))?;
        vec![(
            task.id.clone(),
            task.title.clone(),
            task.description.clone(),
            task.details.clone(),
        )]
    } else {
        group
            .tasks
            .iter()
            .map(|t| {
                (
                    t.id.clone(),
                    t.title.clone(),
                    t.description.clone(),
                    t.details.clone(),
                )
            })
            .collect()
    };

    if tasks_to_analyze.is_empty() {
        println!("{}", "No tasks to analyze".yellow());
        return Ok(());
    }

    let task_count = tasks_to_analyze.len();
    println!(
        "{} {} task(s) with {} concurrent requests...",
        "Analyzing complexity for".blue(),
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
    let model_arc = Arc::new(model_str);
    let results: Vec<Result<TaskAnalysisResult, (String, anyhow::Error)>> =
        stream::iter(tasks_to_analyze)
            .map(|(id, title, description, details)| {
                let client = Arc::clone(&client);
                let mp = multi_progress.clone();
                let overall = overall_progress.clone();
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

                    let prompt =
                        Prompts::analyze_complexity(&title, &description, details.as_deref());

                    // Retry logic (use fast model for generation tasks)
                    let mut last_error = None;
                    for attempt in 1..=3 {
                        match client
                            .complete_json_fast::<ComplexityAnalysis>(&prompt, model_ref.as_deref())
                            .await
                        {
                            Ok(analysis) => {
                                spinner.finish_and_clear();
                                overall.inc(1);
                                return Ok(TaskAnalysisResult {
                                    id,
                                    title,
                                    complexity: analysis.complexity,
                                    reasoning: analysis.reasoning,
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

    // Process results and update tasks
    let mut success_count = 0;
    let mut error_count = 0;
    let mut high_complexity_tasks = Vec::new();

    for result in results {
        match result {
            Ok(analysis) => {
                if let Some(task) = group.get_task_mut(&analysis.id) {
                    task.complexity = analysis.complexity;
                    task.update();

                    println!(
                        "{} Task {}: {} → complexity {}",
                        "✓".green(),
                        analysis.id.cyan(),
                        analysis.title,
                        analysis.complexity.to_string().yellow()
                    );

                    if analysis.complexity > 13 {
                        high_complexity_tasks.push(analysis.id.clone());
                    }
                    success_count += 1;
                }
            }
            Err((id, e)) => {
                println!("{} Task {} failed: {}", "✗".red(), id.cyan(), e);
                error_count += 1;
            }
        }
    }

    // Get stats before saving
    let stats = group.get_stats();
    let tasks_needing_expansion: Vec<_> = group
        .get_tasks_needing_expansion()
        .iter()
        .map(|t| (t.id.clone(), t.title.clone(), t.complexity))
        .collect();

    storage.save_tasks(&all_tasks)?;

    // Summary
    println!("\n{}", "✅ Complexity analysis complete!".green().bold());
    println!();
    println!(
        "{:<25} {} ({} succeeded, {} failed)",
        "Analyzed:".yellow(),
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
        "Total complexity:".yellow(),
        stats.total_complexity
    );

    if !tasks_needing_expansion.is_empty() {
        println!();
        println!(
            "{} {} task(s) with complexity ≥3 need expansion:",
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
