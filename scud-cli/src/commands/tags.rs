use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);
    let tasks = storage.load_tasks()?;
    let active_epic = storage.get_active_epic()?;

    if tasks.is_empty() {
        println!("{}", "No epic tags found".yellow());
        println!("Create an epic with: scud parse-prd <file> --tag <tag>");
        return Ok(());
    }

    println!("{}", "Epic Tags:".blue().bold());
    for (tag, epic) in tasks.iter() {
        let task_count = epic.tasks.len();
        if Some(tag) == active_epic.as_ref() {
            println!("  {} {} ({} tasks)", "●".green(), tag.green().bold(), task_count);
        } else {
            println!("  {} {} ({} tasks)", "○".white(), tag, task_count);
        }
    }

    if let Some(active) = active_epic {
        println!("\n{} {}", "Active epic:".blue(), active.green());
    } else {
        println!("\n{}", "No active epic. Run: scud use-tag <tag>".yellow());
    }

    Ok(())
}
