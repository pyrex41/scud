use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    status_str: &str,
    tag: Option<&str>,
) -> Result<()> {
    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let storage = Storage::new(project_root);

    let epic_tag = resolve_group_tag(&storage, tag, true)?;
    let mut epic = storage.load_group(&epic_tag)?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    // Auto-release lock when marking task as done (fulfills the promise in claim messaging)
    let was_locked = task.is_locked();
    let is_done = new_status == TaskStatus::Done;
    if is_done && was_locked {
        task.release();
        task.assigned_to = None;
    }

    task.set_status(new_status);

    storage.update_group(&epic_tag, &epic)?;

    // Show lock release message if applicable
    if is_done && was_locked {
        println!(
            "{} Task {} → {} (lock released)",
            "✓".green(),
            task_id.cyan(),
            status_str.green()
        );
        return Ok(());
    }

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}
