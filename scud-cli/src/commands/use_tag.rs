use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, tag: &str) -> Result<()> {
    let storage = Storage::new(project_root);
    storage.set_active_epic(tag)?;

    println!("{} {}", "✓ Active epic set to:".green(), tag.green().bold());

    let tasks = storage.load_tasks()?;
    if let Some(epic) = tasks.get(tag) {
        let stats = epic.get_stats();
        println!("  Tasks: {}", stats.total);
        println!("  Pending: {}", stats.pending);
        println!("  In Progress: {}", stats.in_progress);
        println!("  Done: {}", stats.done);
    }

    Ok(())
}
