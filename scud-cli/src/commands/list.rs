use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::formats::{natural_sort_ids, serialize_scg};
use crate::models::{Phase, Priority, TaskStatus};
use crate::storage::Storage;

/// Format status for human display
fn format_status(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Pending => "○ Pending".normal().to_string(),
        TaskStatus::InProgress => "◐ In Progress".yellow().to_string(),
        TaskStatus::Done => "● Done".green().to_string(),
        TaskStatus::Review => "◑ Review".cyan().to_string(),
        TaskStatus::Blocked => "✗ Blocked".red().to_string(),
        TaskStatus::Deferred => "◌ Deferred".dimmed().to_string(),
        TaskStatus::Cancelled => "⊘ Cancelled".dimmed().to_string(),
        TaskStatus::Expanded => "◈ Expanded".blue().to_string(),
        TaskStatus::Failed => "✗ Failed".red().bold().to_string(),
    }
}

/// Format priority for human display
fn format_priority(priority: &Priority) -> String {
    match priority {
        Priority::Critical => "Crit".red().bold().to_string(),
        Priority::High => "High".yellow().to_string(),
        Priority::Medium => "Med".normal().to_string(),
        Priority::Low => "Low".dimmed().to_string(),
    }
}

/// Format agent type for human display
fn format_agent_type(agent_type: &Option<String>) -> String {
    match agent_type {
        Some(at) => at.clone(),
        None => "-".to_string(),
    }
}

/// Truncate long task IDs for display (e.g., UUIDs)
/// Shows first 8 chars with "..." for IDs longer than 12 chars
fn format_task_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..8])
    } else {
        id.to_string()
    }
}

/// Print human-readable task list
fn print_human_readable(phase: &Phase, phase_tag: &str) {
    println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());

    if phase.tasks.is_empty() {
        println!("{}", "(no tasks)".dimmed());
        return;
    }

    // Header - use 11 char width for ID column to fit "8chars..." format
    println!(
        "{:>4}  {:<11} {:<32} {:<14} {:>4}  {:<5} {}",
        "#".dimmed(),
        "ID".dimmed(),
        "Title".dimmed(),
        "Status".dimmed(),
        "Cplx".dimmed(),
        "Pri".dimmed(),
        "Agent".dimmed()
    );
    println!("{}", "─".repeat(90).dimmed());

    // Sort tasks by ID for display
    let mut sorted_tasks = phase.tasks.clone();
    sorted_tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

    for (idx, task) in sorted_tasks.iter().enumerate() {
        let title = if task.title.len() > 30 {
            format!("{}...", &task.title[..27])
        } else {
            task.title.clone()
        };

        println!(
            "{:>4}  {:<11} {:<32} {:<14} {:>4}  {:<5} {}",
            (idx + 1).to_string().dimmed(),
            format_task_id(&task.id).cyan(),
            title,
            format_status(&task.status),
            task.complexity,
            format_priority(&task.priority),
            format_agent_type(&task.agent_type).dimmed()
        );
    }

    println!();
    println!("{} {} tasks", "Total:".dimmed(), phase.tasks.len());
}

pub fn run(
    project_root: Option<PathBuf>,
    status_filter: Option<&str>,
    tag: Option<&str>,
    json_output: bool,
    verbose: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    let phase_tag = resolve_group_tag(&storage, tag, true)?;
    let tasks = storage.load_tasks()?;
    let phase = tasks
        .get(&phase_tag)
        .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

    let filter_status = status_filter
        .map(|s| {
            TaskStatus::from_str(s).ok_or_else(|| {
                anyhow::anyhow!("Invalid status: {}. Valid: {:?}", s, TaskStatus::all())
            })
        })
        .transpose()?;

    let filtered_phase = if filter_status.is_some() {
        let filtered_tasks: Vec<_> = phase
            .tasks
            .iter()
            .filter(|t| {
                filter_status
                    .as_ref()
                    .map(|fs| t.status == *fs)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let mut filtered = Phase::new(phase.name.clone());
        filtered.tasks = filtered_tasks;
        filtered
    } else {
        phase.clone()
    };

    if filtered_phase.tasks.is_empty() {
        if json_output {
            println!("[]");
        } else if verbose {
            println!("# SCUD Graph v1");
            println!("# Phase: {}", phase_tag);
            println!();
            println!("@nodes");
            println!("# id | title | status | complexity | priority");
            println!("# (no tasks)");
        } else {
            println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());
            println!("{}", "(no tasks)".dimmed());
        }
        return Ok(());
    }

    if json_output {
        let json = serde_json::to_string_pretty(&filtered_phase.tasks)?;
        println!("{}", json);
    } else if verbose {
        // Raw SCG format
        let scg = serialize_scg(&filtered_phase);
        print!("{}", scg);
    } else {
        // Human-readable format (default)
        print_human_readable(&filtered_phase, &phase_tag);
    }

    Ok(())
}
