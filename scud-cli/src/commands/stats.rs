use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    // OPTIMIZED: Load only active epic (uses cache + lazy loading)
    let epic = storage.load_active_epic()?;
    let active_epic = &epic.name;

    let stats = epic.get_stats();

    let completion_pct = if stats.total > 0 {
        (stats.done as f32 / stats.total as f32 * 100.0) as u32
    } else {
        0
    };

    println!(
        "\n{} {}",
        "Epic Statistics:".blue().bold(),
        active_epic.green()
    );
    println!("{}", "=================".blue());
    println!();
    println!("{:<20} {}", "Total Tasks:".yellow(), stats.total);
    println!("{:<20} {}", "Pending:".yellow(), stats.pending);
    println!("{:<20} {}", "In Progress:".yellow(), stats.in_progress);
    println!(
        "{:<20} {}",
        "Done:".yellow(),
        stats.done.to_string().green()
    );
    println!(
        "{:<20} {}",
        "Blocked:".yellow(),
        stats.blocked.to_string().red()
    );
    println!();
    println!(
        "{:<20} {}",
        "Total Complexity:".yellow(),
        stats.total_complexity
    );
    println!(
        "{:<20} {}%",
        "Completion:".yellow(),
        completion_pct.to_string().green()
    );

    // Show progress bar
    let bar_length = 50;
    let filled = (completion_pct as f32 / 100.0 * bar_length as f32) as usize;
    let empty = bar_length - filled;
    let bar = format!("[{}{}]", "=".repeat(filled).green(), " ".repeat(empty));
    println!("\n{}", bar);
    println!();

    Ok(())
}
