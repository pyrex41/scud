use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, task_id: &str, tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);

    let epic_tag = resolve_epic_tag(&storage, tag, true)?;
    let tasks = storage.load_tasks()?;
    let epic = tasks
        .get(&epic_tag)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", epic_tag))?;

    let task = epic
        .get_task(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    println!("\n{}", "Task Details".blue().bold());
    println!("{}", "=============".blue());
    println!("{:<20} {}", "ID:".yellow(), task.id.cyan());
    println!("{:<20} {}", "Title:".yellow(), task.title.bold());
    println!("{:<20} {}", "Status:".yellow(), task.status.as_str());
    println!("{:<20} {}", "Complexity:".yellow(), task.complexity);
    println!("{:<20} {:?}", "Priority:".yellow(), task.priority);

    if !task.dependencies.is_empty() {
        println!("{:<20} {:?}", "Dependencies:".yellow(), task.dependencies);
    }

    println!("\n{}", "Description:".yellow());
    println!("{}", task.description);

    if let Some(details) = &task.details {
        println!("\n{}", "Technical Details:".yellow());
        println!("{}", details);
    }

    if let Some(test_strategy) = &task.test_strategy {
        println!("\n{}", "Test Strategy:".yellow());
        println!("{}", test_strategy);
    }

    if let Some(created) = &task.created_at {
        println!("\n{:<20} {}", "Created:".yellow(), created);
    }

    if let Some(updated) = &task.updated_at {
        println!("{:<20} {}", "Updated:".yellow(), updated);
    }

    println!();
    Ok(())
}
