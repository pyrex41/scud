use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);

    let tags: Vec<String> = if let Some(t) = tag {
        vec![t.to_string()]
    } else {
        // Get all tags by listing all groups from storage
        storage.load_tasks()?.keys().cloned().collect()
    };

    println!("{}", "Active Sessions".bold());
    println!();

    let mut found_any = false;

    for tag in tags {
        if let Ok(phase) = storage.load_group(&tag) {
            let locked_tasks: Vec<_> = phase.tasks.iter().filter(|t| t.is_locked()).collect();

            if !locked_tasks.is_empty() {
                found_any = true;
                println!("  {} {}", "Tag:".dimmed(), tag.cyan());

                for task in locked_tasks {
                    let age = task.lock_age_hours().unwrap_or(0.0);
                    let stale_marker = if age > 1.0 {
                        " (STALE)".red().to_string()
                    } else {
                        String::new()
                    };
                    let locked_by = task.locked_by.as_deref().unwrap_or("unknown");

                    println!(
                        "    {} | {} | {} | {:.1}h{}",
                        task.id.yellow(),
                        truncate(&task.title, 30),
                        locked_by.green(),
                        age,
                        stale_marker
                    );
                }
                println!();
            }
        }
    }

    if !found_any {
        println!("  {}", "No active sessions".dimmed());
    }

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
