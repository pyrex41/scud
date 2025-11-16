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
    let active_epic = storage
        .get_active_epic()?
        .ok_or_else(|| anyhow::anyhow!("No active epic. Run: scud use-tag <epic-tag>"))?;

    let mut all_tasks = storage.load_tasks()?;
    let epic = all_tasks
        .get_mut(&active_epic)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", active_epic))?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, active_epic))?;

    task.set_status(new_status);

    storage.save_tasks(&all_tasks)?;

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}
