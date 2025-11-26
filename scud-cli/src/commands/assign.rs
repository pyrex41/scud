use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    assignee: &str,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let epic_tag = resolve_epic_tag(&storage, tag, true)?;

    let mut all_tasks = storage.load_tasks()?;
    let epic = all_tasks
        .get_mut(&epic_tag)
        .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", epic_tag))?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    task.assign(assignee);
    storage.save_tasks(&all_tasks)?;

    println!(
        "{} Task {} assigned to {}",
        "✓".green(),
        task_id.cyan(),
        assignee.green()
    );

    Ok(())
}
