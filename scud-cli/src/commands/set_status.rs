use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
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

    let epic_tag = resolve_epic_tag(&storage, tag, true)?;
    let mut epic = storage.load_epic(&epic_tag)?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    task.set_status(new_status);

    storage.update_epic(&epic_tag, &epic)?;

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}
