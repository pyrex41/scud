use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    force: bool,
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

    if !task.is_locked() {
        println!("{}", "⊘ Task is not locked".yellow());
        return Ok(());
    }

    if !force {
        if let Some(ref locked_by) = task.locked_by {
            println!("{}", "⚠ Task is locked".yellow());
            println!("{:<20} {}", "Locked by:".yellow(), locked_by.green());
            if let Some(age) = task.lock_age_hours() {
                println!("{:<20} {:.1}h ago", "Locked:".yellow(), age);
            }
            println!();
            println!("To force release: scud release {} --force", task_id);
            return Ok(());
        }
    }

    let was_locked_by = task.locked_by.clone();
    task.release();
    storage.save_tasks(&all_tasks)?;

    println!("{} Task {} released", "✓".green(), task_id.cyan());
    if let Some(locked_by) = was_locked_by {
        println!("  Previously locked by: {}", locked_by);
    }

    Ok(())
}
