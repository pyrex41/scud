use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, status_filter: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);

    // OPTIMIZED: Load only active epic (uses cache + lazy loading)
    let epic = storage.load_active_epic()?;

    // Parse filter status once
    let filter_status = status_filter
        .map(|s| {
            TaskStatus::from_str(s).ok_or_else(|| {
                anyhow::anyhow!("Invalid status: {}. Valid: {:?}", s, TaskStatus::all())
            })
        })
        .transpose()?;

    // OPTIMIZED: Use iterator instead of clone
    let task_iter = epic.tasks.iter().filter(|t| {
        filter_status
            .as_ref()
            .map(|fs| t.status == *fs)
            .unwrap_or(true)
    });

    if task_iter.clone().count() == 0 {
        println!("{}", "No tasks found".yellow());
        return Ok(());
    }

    println!("{} {}", "Tasks in epic:".blue().bold(), epic.name.green());
    println!();

    for task in task_iter {
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
