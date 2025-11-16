use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, group_id: &str) -> Result<()> {
    let storage = Storage::new(project_root);
    let groups = storage.load_groups()?;

    let group = groups
        .get_group(group_id)
        .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group_id))?;

    let tasks = storage.load_tasks()?;

    println!("\n{} {}", "Group:".blue().bold(), group.name.green());
    println!("{}", "=".repeat(50).blue());
    println!("{:<20} {}", "ID:".yellow(), group.id);
    println!("{:<20} {:?}", "Status:".yellow(), group.status);
    if let Some(ref desc) = group.description {
        println!("{:<20} {}", "Description:".yellow(), desc);
    }
    println!();

    // Aggregate stats across all epics in group
    let mut total_tasks = 0;
    let mut pending = 0;
    let mut in_progress = 0;
    let mut done = 0;
    let mut blocked = 0;
    let mut total_complexity = 0;

    println!("{}", "Epics in Group:".blue().bold());
    for epic_tag in &group.epic_tags {
        if let Some(epic) = tasks.get(epic_tag) {
            let stats = epic.get_stats();
            println!("  {} {} tasks", epic_tag.cyan(), stats.total);

            total_tasks += stats.total;
            pending += stats.pending;
            in_progress += stats.in_progress;
            done += stats.done;
            blocked += stats.blocked;
            total_complexity += stats.total_complexity;
        }
    }

    println!();
    println!("{}", "Aggregate Statistics:".blue().bold());
    println!("{:<20} {}", "Total Tasks:".yellow(), total_tasks);
    println!("{:<20} {}", "Pending:".yellow(), pending);
    println!("{:<20} {}", "In Progress:".yellow(), in_progress);
    println!("{:<20} {}", "Done:".yellow(), done.to_string().green());
    println!("{:<20} {}", "Blocked:".yellow(), blocked.to_string().red());
    println!();
    println!("{:<20} {}", "Total Complexity:".yellow(), total_complexity);

    let completion_pct = if total_tasks > 0 {
        (done as f32 / total_tasks as f32 * 100.0) as u32
    } else {
        0
    };
    println!(
        "{:<20} {}%",
        "Completion:".yellow(),
        completion_pct.to_string().green()
    );

    // Progress bar
    let bar_length = 50;
    let filled = (completion_pct as f32 / 100.0 * bar_length as f32) as usize;
    let empty = bar_length - filled;
    let bar = format!("[{}{}]", "=".repeat(filled).green(), " ".repeat(empty));
    println!("\n{}", bar);
    println!();

    Ok(())
}
