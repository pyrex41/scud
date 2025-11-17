use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, task_id: &str, status_str: &str) -> Result<()> {
    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let storage = Storage::new(project_root);

    // OPTIMIZED: Get active epic from cache
    let active_tag = storage
        .get_active_epic()?
        .ok_or_else(|| anyhow::anyhow!("No active epic. Run: scud use-tag <epic-tag>"))?;

    // OPTIMIZED: Load only active epic
    let mut epic = storage.load_epic(&active_tag)?;

    let task = epic.get_task_mut(task_id).ok_or_else(|| {
        anyhow::anyhow!("Task {} not found in epic '{}'", task_id, active_tag)
    })?;

    task.set_status(new_status);

    // OPTIMIZED: Save only active epic
    storage.update_epic(&active_tag, &epic)?;

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}
