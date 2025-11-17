use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    // OPTIMIZED: Load only active epic (uses cache + lazy loading)
    let epic = storage.load_active_epic()?;

    match epic.find_next_task() {
        Some(task) => {
            println!("{}", "Next Available Task:".green().bold());
            println!();
            println!("{:<20} {}", "ID:".yellow(), task.id.cyan());
            println!("{:<20} {}", "Title:".yellow(), task.title.bold());
            println!("{:<20} {}", "Complexity:".yellow(), task.complexity);
            println!("{:<20} {:?}", "Priority:".yellow(), task.priority);
            println!();
            println!("{}", "Description:".yellow());
            println!("{}", task.description);

            if let Some(details) = &task.details {
                println!();
                println!("{}", "Technical Details:".yellow());
                println!("{}", details);
            }

            if let Some(test_strategy) = &task.test_strategy {
                println!();
                println!("{}", "Test Strategy:".yellow());
                println!("{}", test_strategy);
            }

            println!();
            println!("{}", "To start this task:".blue());
            println!("  scud set-status {} in-progress", task.id);
        }
        None => {
            println!(
                "{}",
                "No available tasks with all dependencies met".yellow()
            );
            println!("Run: scud list --status pending");
        }
    }

    Ok(())
}
