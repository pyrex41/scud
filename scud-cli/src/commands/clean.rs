use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, force: bool, tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let mut all_tasks = storage.load_tasks()?;

    if all_tasks.is_empty() {
        println!("{}", "No tasks to clean.".yellow());
        return Ok(());
    }

    // Build confirmation message
    let (confirm_msg, action_desc) = if let Some(tag_name) = tag {
        if !all_tasks.contains_key(tag_name) {
            anyhow::bail!("Tag '{}' not found", tag_name);
        }
        let task_count = all_tasks.get(tag_name).map(|p| p.tasks.len()).unwrap_or(0);
        (
            format!(
                "Delete {} tasks from tag '{}'?",
                task_count.to_string().red(),
                tag_name.cyan()
            ),
            format!("Cleared tag '{}'", tag_name),
        )
    } else {
        let total_tasks: usize = all_tasks.values().map(|p| p.tasks.len()).sum();
        let tag_count = all_tasks.len();
        (
            format!(
                "Delete ALL {} tasks across {} tags?",
                total_tasks.to_string().red(),
                tag_count.to_string().red()
            ),
            "Cleared all tasks".to_string(),
        )
    };

    // Confirm unless --force
    if !force {
        println!();
        println!(
            "{}",
            "⚠ WARNING: This action cannot be undone!".red().bold()
        );
        println!();

        let confirmed = Confirm::new()
            .with_prompt(confirm_msg)
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Cancelled.".yellow());
            return Ok(());
        }
    }

    // Perform the clean
    if let Some(tag_name) = tag {
        all_tasks.remove(tag_name);
        // Clear active tag if it was the one we deleted
        if let Ok(Some(active)) = storage.get_active_group() {
            if active == tag_name {
                let _ = storage.clear_active_group();
            }
        }
    } else {
        all_tasks.clear();
        let _ = storage.clear_active_group();
    }

    storage.save_tasks(&all_tasks)?;

    println!();
    println!("{} {}", "✓".green(), action_desc.green());
    println!();

    Ok(())
}
