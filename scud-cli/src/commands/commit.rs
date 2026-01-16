use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, message: Option<&str>, all: bool) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    // Get current task ID from environment or .scud/current-task
    let task_id = get_current_task_id(&storage)?;

    // Get task details if we have a task ID
    let task_context = if let Some(ref id) = task_id {
        get_task_context(&storage, id)
    } else {
        None
    };

    // Build commit message
    let commit_message = build_commit_message(message, task_id.as_deref(), task_context.as_ref())?;

    // Show what we're about to do
    println!("{}", "SCUD Commit".cyan().bold());
    println!("{}", "-".repeat(40).dimmed());

    if let Some(ref id) = task_id {
        println!("Task: {}", id.cyan());
    }
    println!("Message: {}", commit_message.lines().next().unwrap_or(""));

    // Stage files if --all
    if all {
        println!("\n{}", "Staging all changes...".dimmed());
        let status = Command::new("git")
            .args(["add", "-A"])
            .status()
            .context("Failed to run git add")?;

        if !status.success() {
            anyhow::bail!("git add failed");
        }
    }

    // Check if there are staged changes
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .context("Failed to check staged changes")?;

    if staged.success() {
        println!("\n{}", "No staged changes to commit.".yellow());
        println!(
            "Use {} to stage changes, or {} to stage all.",
            "git add <files>".cyan(),
            "scud commit --all".cyan()
        );
        return Ok(());
    }

    // Show staged files
    println!("\n{}", "Staged files:".bold());
    let staged_output = Command::new("git")
        .args(["diff", "--cached", "--name-status"])
        .output()
        .context("Failed to get staged files")?;

    for line in String::from_utf8_lossy(&staged_output.stdout).lines() {
        println!("  {}", line.dimmed());
    }

    // Create commit
    println!("\n{}", "Creating commit...".dimmed());
    let status = Command::new("git")
        .args(["commit", "-m", &commit_message])
        .status()
        .context("Failed to run git commit")?;

    if !status.success() {
        anyhow::bail!("git commit failed");
    }

    println!("\n{} Commit created successfully", "✓".green());

    // Show the commit
    let log = Command::new("git")
        .args(["log", "-1", "--oneline"])
        .output()
        .context("Failed to get commit info")?;

    println!("  {}", String::from_utf8_lossy(&log.stdout).trim().dimmed());

    Ok(())
}

fn get_current_task_id(storage: &Storage) -> Result<Option<String>> {
    // First check environment variable
    if let Ok(id) = std::env::var("SCUD_TASK_ID") {
        if !id.is_empty() {
            return Ok(Some(id));
        }
    }

    // Then check .scud/current-task file
    let current_task_file = storage.scud_dir().join("current-task");
    if current_task_file.exists() {
        let content = std::fs::read_to_string(&current_task_file)?;
        let id = content.trim();
        if !id.is_empty() {
            return Ok(Some(id.to_string()));
        }
    }

    Ok(None)
}

struct TaskContext {
    title: String,
}

fn get_task_context(storage: &Storage, task_id: &str) -> Option<TaskContext> {
    // Try to find the task in the active phase first
    if let Ok(Some(tag)) = storage.get_active_group() {
        if let Ok(phase) = storage.load_group(&tag) {
            if let Some(task) = phase.tasks.iter().find(|t| t.id == task_id) {
                return Some(TaskContext {
                    title: task.title.clone(),
                });
            }
        }
    }

    // Search all phases
    if let Ok(all_tasks) = storage.load_tasks() {
        for (_tag, phase) in all_tasks {
            if let Some(task) = phase.tasks.iter().find(|t| t.id == task_id) {
                return Some(TaskContext {
                    title: task.title.clone(),
                });
            }
        }
    }

    None
}

fn build_commit_message(
    user_message: Option<&str>,
    task_id: Option<&str>,
    task_context: Option<&TaskContext>,
) -> Result<String> {
    let mut message = String::new();

    // Add task prefix if available
    if let Some(id) = task_id {
        message.push_str(&format!("[{}] ", id));
    }

    // Add user message or task title
    if let Some(msg) = user_message {
        message.push_str(msg);
    } else if let Some(ctx) = task_context {
        // Use task title as default message
        message.push_str(&ctx.title);
    } else {
        anyhow::bail!("No commit message provided and no task context available.\nUse: scud commit -m \"your message\"");
    }

    Ok(message)
}
