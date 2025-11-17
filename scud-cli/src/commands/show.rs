use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, task_id: &str) -> Result<()> {
    let storage = Storage::new(project_root);

    // OPTIMIZED: Load only active epic (uses cache + lazy loading)
    let epic = storage.load_active_epic()?;

    let task = epic.get_task(task_id).ok_or_else(|| {
        anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic.name)
    })?;

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

    if let Some(analysis) = &task.complexity_analysis {
        println!("\n{}", "Complexity Analysis:".yellow());
        println!("{}", analysis);
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
