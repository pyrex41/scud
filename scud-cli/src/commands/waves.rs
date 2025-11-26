use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

#[derive(Debug)]
pub struct Wave {
    pub number: usize,
    pub tasks: Vec<String>,
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    max_parallel: usize,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let all_tasks = storage.load_tasks()?;

    // Determine which epic(s) to plan
    let epic_tags: Vec<String> = if all_tags {
        all_tasks.keys().cloned().collect()
    } else if let Some(t) = tag {
        if !all_tasks.contains_key(t) {
            anyhow::bail!("Epic '{}' not found. Run: scud tags", t);
        }
        vec![t.to_string()]
    } else {
        // Use active epic
        let active = storage.get_active_epic()?;
        match active {
            Some(t) => vec![t],
            None => anyhow::bail!("No active epic. Use --tag <epic-tag> or run: scud tags"),
        }
    };

    // Collect actionable tasks from specified epic(s)
    let mut actionable: Vec<&Task> = Vec::new();
    for tag in &epic_tags {
        if let Some(epic) = all_tasks.get(tag) {
            for task in &epic.tasks {
                // Only include actionable tasks (not done, not expanded parents, not cancelled)
                if task.status != TaskStatus::Done
                    && task.status != TaskStatus::Expanded
                    && task.status != TaskStatus::Cancelled
                {
                    // If it's a subtask, only include if parent is expanded
                    if let Some(ref parent_id) = task.parent_id {
                        let parent_expanded = epic
                            .get_task(parent_id)
                            .map(|p| p.is_expanded())
                            .unwrap_or(false);
                        if parent_expanded {
                            actionable.push(task);
                        }
                    } else {
                        // Top-level task that's not expanded
                        actionable.push(task);
                    }
                }
            }
        }
    }

    if actionable.is_empty() {
        println!("{}", "No actionable tasks found.".yellow());
        println!("All tasks may be completed, expanded, or cancelled.");
        return Ok(());
    }

    // Build dependency graph and compute waves
    let waves = compute_waves(&actionable, max_parallel);

    // Display waves
    println!(
        "\n{} {}",
        "Execution Waves".blue().bold(),
        format!("(max {} parallel)", max_parallel).dimmed()
    );
    println!("{}", "=".repeat(50).blue());
    println!();

    let mut total_tasks = 0;
    for wave in &waves {
        total_tasks += wave.tasks.len();

        let batch_info = if wave.tasks.len() > max_parallel {
            format!(
                " (batched into {} rounds)",
                wave.tasks.len().div_ceil(max_parallel)
            )
        } else {
            String::new()
        };

        println!(
            "{} {} task{}{}",
            format!("Wave {}:", wave.number).yellow().bold(),
            wave.tasks.len(),
            if wave.tasks.len() == 1 { "" } else { "s" },
            batch_info.dimmed()
        );

        for (round_idx, chunk) in wave.tasks.chunks(max_parallel).enumerate() {
            if wave.tasks.len() > max_parallel {
                println!("  {} {}", "Round".dimmed(), round_idx + 1);
            }

            for task_id in chunk {
                // Find task details
                if let Some(task) = actionable.iter().find(|t| &t.id == task_id) {
                    let status_indicator = match task.status {
                        TaskStatus::Pending => "○".white(),
                        TaskStatus::InProgress => "●".cyan(),
                        TaskStatus::Blocked => "✗".red(),
                        _ => "?".dimmed(),
                    };

                    let deps = if task.dependencies.is_empty() {
                        String::new()
                    } else {
                        format!(" <- {}", task.dependencies.join(", "))
                            .dimmed()
                            .to_string()
                    };

                    let complexity = if task.complexity > 0 {
                        format!(" [{}]", task.complexity).dimmed().to_string()
                    } else {
                        String::new()
                    };

                    println!(
                        "    {} {} {}{}{}",
                        status_indicator,
                        task_id.cyan(),
                        task.title,
                        complexity,
                        deps
                    );
                }
            }
        }
        println!();
    }

    // Summary
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(30).blue());

    let total_waves = waves.len();
    let total_rounds: usize = waves
        .iter()
        .map(|w| w.tasks.len().div_ceil(max_parallel))
        .sum();

    println!("  Total tasks:   {}", total_tasks);
    println!("  Total waves:   {}", total_waves);
    println!("  Total rounds:  {}", total_rounds);

    if total_tasks > 0 && total_rounds > 0 {
        let speedup = total_tasks as f64 / total_rounds as f64;
        println!("  Speedup:       {}", format!("{:.1}x", speedup).green());
        println!(
            "  {}",
            format!(
                "(from {} sequential to {} parallel rounds)",
                total_tasks, total_rounds
            )
            .dimmed()
        );
    }

    // Show blocked tasks if any
    let blocked: Vec<_> = actionable
        .iter()
        .filter(|t| t.status == TaskStatus::Blocked)
        .collect();
    if !blocked.is_empty() {
        println!();
        println!("{}", "Blocked Tasks:".red().bold());
        for task in blocked {
            println!("  {} {}", task.id.red(), task.title);
        }
    }

    println!();

    Ok(())
}

/// Compute execution waves using Kahn's algorithm (topological sort with level assignment)
fn compute_waves(tasks: &[&Task], _max_parallel: usize) -> Vec<Wave> {
    let task_ids: HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();

    // Build in-degree map (how many dependencies does each task have within our set?)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for task in tasks {
        in_degree.entry(task.id.as_str()).or_insert(0);

        for dep in &task.dependencies {
            // Only count dependencies that are in our actionable task set
            if task_ids.contains(dep.as_str()) {
                *in_degree.entry(task.id.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(task.id.as_str());
            }
        }
    }

    // Kahn's algorithm with wave tracking
    let mut waves: Vec<Wave> = Vec::new();
    let mut remaining = in_degree.clone();
    let mut wave_number = 1;

    while !remaining.is_empty() {
        // Find all tasks with no remaining dependencies (in-degree = 0)
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        if ready.is_empty() {
            // Circular dependency detected
            println!("{}", "Warning: Circular dependency detected!".red().bold());
            println!("The following tasks have unresolved dependencies:");
            for id in remaining.keys() {
                if let Some(task) = tasks.iter().find(|t| t.id.as_str() == *id) {
                    let unmet_deps: Vec<_> = task
                        .dependencies
                        .iter()
                        .filter(|d| remaining.contains_key(d.as_str()))
                        .collect();
                    println!("  {} depends on {:?}", id, unmet_deps);
                }
            }
            break;
        }

        // Remove ready tasks from remaining and update dependents
        for &task_id in &ready {
            remaining.remove(task_id);

            if let Some(deps) = dependents.get(task_id) {
                for &dep_id in deps {
                    if let Some(deg) = remaining.get_mut(dep_id) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        waves.push(Wave {
            number: wave_number,
            tasks: ready.iter().map(|&s| s.to_string()).collect(),
        });
        wave_number += 1;
    }

    waves
}
