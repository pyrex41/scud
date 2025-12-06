use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;

use crate::commands::helpers::flatten_all_tasks;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        println!("{}", "SCUD not initialized. Run: scud init".yellow());
        return Ok(());
    }

    println!("{}", "SCUD Session Warmup".cyan().bold());
    println!("{}", "=".repeat(50).dimmed());

    // 1. Show working directory
    let cwd = std::env::current_dir()?;
    println!("\n{} {}", "Working directory:".bold(), cwd.display());

    // 2. Show recent git commits
    println!("\n{}", "Recent commits:".bold());
    match Command::new("git")
        .args(["log", "--oneline", "-5", "--no-decorate"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let commits = String::from_utf8_lossy(&output.stdout);
            if commits.trim().is_empty() {
                println!("  {}", "(no commits yet)".dimmed());
            } else {
                for line in commits.lines() {
                    println!("  {}", line.dimmed());
                }
            }
        }
        _ => println!("  {}", "(not a git repository)".dimmed()),
    }

    // 3. Show active tag and stats
    println!("\n{}", "Task status:".bold());
    match storage.get_active_group()? {
        Some(tag) => {
            println!("  Active tag: {}", tag.green());

            // Load and show stats
            if let Ok(phase) = storage.load_group(&tag) {
                let stats = phase.get_stats();
                println!(
                    "  Progress: {}/{} tasks done ({}%)",
                    stats.done.to_string().green(),
                    stats.total,
                    if stats.total > 0 {
                        (stats.done * 100 / stats.total).to_string()
                    } else {
                        "0".to_string()
                    }
                );
                println!(
                    "  Status: {} pending, {} in-progress, {} blocked",
                    stats.pending.to_string().yellow(),
                    stats.in_progress.to_string().cyan(),
                    stats.blocked.to_string().red()
                );
            }
        }
        None => {
            println!("  {}", "No active tag set".yellow());
            println!("  Run: scud tags <tag-name>");
        }
    }

    // 4. Show current assignments
    println!("\n{}", "Current assignments:".bold());
    let tasks = storage.load_tasks()?;
    let mut found_assignments = false;

    for (tag, phase) in &tasks {
        for task in &phase.tasks {
            if let Some(ref assigned_to) = task.assigned_to {
                found_assignments = true;
                let status_str = task.status.as_str();
                println!(
                    "  {} | {} | {} | {}",
                    tag.dimmed(),
                    task.id.cyan(),
                    assigned_to.yellow(),
                    status_str
                );
            }
        }
    }
    if !found_assignments {
        println!("  {}", "(no assignments)".dimmed());
    }

    // 5. Show next available task (with cross-tag dependency checking)
    println!("\n{}", "Next available task:".bold());
    let all_tasks_flat = flatten_all_tasks(&tasks);
    if let Some(tag) = storage.get_active_group()? {
        if let Some(phase) = tasks.get(&tag) {
            let available: Vec<_> = phase
                .tasks
                .iter()
                .filter(|t| {
                    t.status == crate::models::TaskStatus::Pending
                        && t.has_dependencies_met_refs(&all_tasks_flat)
                })
                .collect();

            if let Some(task) = available.first() {
                println!(
                    "  {} {} (complexity: {})",
                    task.id.cyan(),
                    task.title,
                    task.complexity
                );
                println!("  Run: {}", "scud set-status <task-id> in-progress".green());
            } else if phase
                .tasks
                .iter()
                .all(|t| t.status == crate::models::TaskStatus::Done)
            {
                println!("  {}", "All tasks complete!".green());
            } else {
                println!("  {}", "(no tasks available - check dependencies)".yellow());
            }
        }
    } else {
        println!("  {}", "(set active tag first)".dimmed());
    }

    println!("\n{}", "=".repeat(50).dimmed());
    println!(
        "Ready to work. Use {} to find your next task.",
        "scud next".cyan()
    );

    Ok(())
}
