use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    status_filter: Option<&str>,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Resolve phase tag (explicit --tag, active phase, or interactive selection)
    let phase_tag = resolve_group_tag(&storage, tag, true)?;
    let tasks = storage.load_tasks()?;
    let phase = tasks
        .get(&phase_tag)
        .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

    // Parse filter status once
    let filter_status = status_filter
        .map(|s| {
            TaskStatus::from_str(s).ok_or_else(|| {
                anyhow::anyhow!("Invalid status: {}. Valid: {:?}", s, TaskStatus::all())
            })
        })
        .transpose()?;

    // OPTIMIZED: Use iterator instead of clone
    let task_iter = phase.tasks.iter().filter(|t| {
        filter_status
            .as_ref()
            .map(|fs| t.status == *fs)
            .unwrap_or(true)
    });

    if task_iter.clone().count() == 0 {
        println!("{}", "No tasks found".yellow());
        return Ok(());
    }

    println!("{} {}", "Tasks in phase:".blue().bold(), phase.name.green());
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
