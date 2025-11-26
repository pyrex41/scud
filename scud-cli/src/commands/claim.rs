use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    name: &str,
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

    // Try to claim the task
    match task.claim(name) {
        Ok(()) => {
            // Get task title before saving (to avoid borrow checker issues)
            let task_title = task.title.clone();

            storage.save_tasks(&all_tasks)?;

            println!("{}", "✅ Task claimed successfully!".green().bold());
            println!();
            println!("{:<20} {}", "Task ID:".yellow(), task_id.cyan());
            println!("{:<20} {}", "Title:".yellow(), task_title.bold());
            println!("{:<20} {}", "Claimed by:".yellow(), name.green());
            println!("{:<20} {}", "Status:".yellow(), "locked".yellow());
            println!();
            println!("{}", "Next steps:".blue());
            println!("  1. Start working on the task");
            println!("  2. Run: scud set-status {} in-progress", task_id);
            println!("  3. When done: scud set-status {} done", task_id);
            println!("  4. Task will auto-release when marked done");
            println!();
        }
        Err(err) => {
            anyhow::bail!("Failed to claim task: {}", err);
        }
    }

    Ok(())
}
