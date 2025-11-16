use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, status_filter: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let active_epic = storage
        .get_active_epic()?
        .ok_or_else(|| anyhow::anyhow!("No active epic. Run: scud use-tag <epic-tag>"))?;

    let tasks = storage.load_tasks()?;
    let epic = tasks
        .get(&active_epic)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", active_epic))?;

    let mut task_list = epic.tasks.clone();

    // Filter by status if provided
    if let Some(status_str) = status_filter {
        let filter_status = TaskStatus::from_str(status_str).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid status: {}. Valid: {:?}",
                status_str,
                TaskStatus::all()
            )
        })?;
        task_list.retain(|t| t.status == filter_status);
    }

    if task_list.is_empty() {
        println!("{}", "No tasks found".yellow());
        return Ok(());
    }

    println!("{} {}", "Tasks in epic:".blue().bold(), active_epic.green());
    println!();

    for task in task_list {
        let status_color = match task.status {
            TaskStatus::Done => "done".green(),
            TaskStatus::InProgress => "in-progress".yellow(),
            TaskStatus::Blocked => "blocked".red(),
            TaskStatus::Pending => "pending".white(),
            _ => task.status.as_str().white(),
        };

        let complexity_str = if task.complexity > 0 {
            format!("[{}]", task.complexity)
        } else {
            "".to_string()
        };

        println!(
            "  {:<4} {:<15} {} {}",
            task.id.cyan(),
            status_color,
            task.title,
            complexity_str.yellow()
        );
    }

    Ok(())
}
