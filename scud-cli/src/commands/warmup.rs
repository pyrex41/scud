use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;

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

    // 4. Show active sessions (who's working on what)
    println!("\n{}", "Active sessions:".bold());
    let tasks = storage.load_tasks()?;
    let mut found_sessions = false;

    for (tag, phase) in &tasks {
        for task in &phase.tasks {
            if task.is_locked() {
                found_sessions = true;
                let age = task
                    .lock_age_hours()
                    .map(|h| format!("{:.1}h", h))
                    .unwrap_or_else(|| "?".to_string());
                let stale = task.is_stale_lock(1.0);
                let stale_marker = if stale {
                    " (STALE)".red().to_string()
                } else {
                    "".to_string()
                };
                println!(
                    "  {} | {} | {} | {}{}",
                    tag.dimmed(),
                    task.id.cyan(),
                    task.locked_by.as_deref().unwrap_or("?").yellow(),
                    age,
                    stale_marker
                );
            }
        }
    }
    if !found_sessions {
        println!("  {}", "(no active sessions)".dimmed());
    }

    // 5. Show next available task
    println!("\n{}", "Next available task:".bold());
    if let Some(tag) = storage.get_active_group()? {
        if let Ok(phase) = storage.load_group(&tag) {
            let available: Vec<_> = phase
                .tasks
                .iter()
                .filter(|t| {
                    t.status == crate::models::TaskStatus::Pending
                        && t.has_dependencies_met(&phase.tasks)
                        && !t.is_locked()
                })
                .collect();

            if let Some(task) = available.first() {
                println!(
                    "  {} {} (complexity: {})",
                    task.id.cyan(),
                    task.title,
                    task.complexity
                );
                println!("  Run: {}", "scud next --claim --name <your-name>".green());
            } else if phase.tasks.iter().all(|t| t.status == crate::models::TaskStatus::Done) {
                println!("  {}", "All tasks complete!".green());
            } else {
                println!("  {}", "(no tasks available - check dependencies or locks)".yellow());
            }
        }
    } else {
        println!("  {}", "(set active tag first)".dimmed());
    }

    // 6. Check for stale locks
    let stale_count: usize = tasks
        .values()
        .flat_map(|p| p.tasks.iter())
        .filter(|t| t.is_stale_lock(1.0))
        .count();

    if stale_count > 0 {
        println!(
            "\n{} {} stale lock(s) detected. Run: {}",
            "Warning:".yellow().bold(),
            stale_count,
            "scud doctor --fix".cyan()
        );
    }

    println!("\n{}", "=".repeat(50).dimmed());
    println!(
        "Ready to work. Use {} to find your next task.",
        "scud next".cyan()
    );

    Ok(())
}
