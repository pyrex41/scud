use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    first_arg: Option<&str>,
    rest: &[String],
    from: Option<&str>,
    to: Option<&str>,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Determine mode based on arguments
    match (from, to, first_arg, rest.is_empty()) {
        // Mode 1: Bulk transition (--from X --to Y)
        (Some(from_str), Some(to_str), _, _) => {
            run_bulk_transition(&storage, from_str, to_str, tag, all_tags)
        }
        // Mode 2: Multiple task IDs (status task1 task2 ...)
        (None, None, Some(first), false) => {
            // first is status, rest are task IDs
            run_multi_task(&storage, first, rest, tag, all_tags)
        }
        // Mode 3: Single task with no status - error
        (None, None, Some(_first), true) => {
            anyhow::bail!(
                "Missing status or task IDs. Usage:\n  \
                 scud set-status <task_id> <status>\n  \
                 scud set-status <status> <task_id> [task_id...]\n  \
                 scud set-status --from <status> --to <status>"
            )
        }
        // Invalid: only --from or only --to
        (Some(_), None, _, _) | (None, Some(_), _, _) => {
            anyhow::bail!("Both --from and --to must be specified together")
        }
        // No arguments at all
        (None, None, None, _) => {
            anyhow::bail!(
                "Usage:\n  \
                 scud set-status <task_id> <status>\n  \
                 scud set-status <status> <task_id> [task_id...]\n  \
                 scud set-status --from <status> --to <status>"
            )
        }
    }
}

/// Bulk transition: change all tasks from one status to another
fn run_bulk_transition(
    storage: &Storage,
    from_str: &str,
    to_str: &str,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let from_status = TaskStatus::from_str(from_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid --from status: {}. Valid: {:?}",
            from_str,
            TaskStatus::all()
        )
    })?;

    let to_status = TaskStatus::from_str(to_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid --to status: {}. Valid: {:?}",
            to_str,
            TaskStatus::all()
        )
    })?;

    let tags = get_target_tags(storage, tag, all_tags)?;
    let mut total_changed = 0;

    for epic_tag in &tags {
        let mut epic = storage.load_group(epic_tag)?;
        let mut changed_in_tag = 0;

        for task in &mut epic.tasks {
            if task.status == from_status {
                task.set_status(to_status.clone());
                println!("  {} {} → {}", "✓".green(), task.id.cyan(), to_str.green());
                changed_in_tag += 1;
            }
        }

        if changed_in_tag > 0 {
            storage.update_group(epic_tag, &epic)?;
            total_changed += changed_in_tag;
        }
    }

    if total_changed == 0 {
        println!("{} No tasks found with status '{}'", "!".yellow(), from_str);
    } else {
        println!(
            "\n{} Changed {} task(s) from {} to {}",
            "✓".green(),
            total_changed,
            from_str.yellow(),
            to_str.green()
        );
    }

    Ok(())
}

/// Multi-task mode: set specific tasks to a status
fn run_multi_task(
    storage: &Storage,
    status_str: &str,
    task_ids: &[String],
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    // Check if this might be old-style single-task syntax
    // Old: set-status <task_id> <status> where task_ids would have 1 element (the status)
    // New: set-status <status> <task_id> [task_id...] where status_str is status

    // Heuristic: if status_str looks like a task ID (contains ':') and
    // task_ids[0] looks like a status, swap them
    if task_ids.len() == 1
        && TaskStatus::from_str(&task_ids[0]).is_some()
        && TaskStatus::from_str(status_str).is_none()
    {
        // Old syntax: task_id status
        return run_single_task(storage, status_str, &task_ids[0], tag);
    }

    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let tags = get_target_tags(storage, tag, all_tags)?;
    let mut changed = 0;
    let mut not_found: Vec<String> = task_ids.to_vec();

    for epic_tag in &tags {
        let mut epic = storage.load_group(epic_tag)?;
        let mut modified = false;

        for task in &mut epic.tasks {
            if not_found.contains(&task.id) {
                task.set_status(new_status.clone());
                println!(
                    "  {} {} → {}",
                    "✓".green(),
                    task.id.cyan(),
                    status_str.green()
                );
                not_found.retain(|id| id != &task.id);
                modified = true;
                changed += 1;
            }
        }

        if modified {
            storage.update_group(epic_tag, &epic)?;
        }
    }

    if !not_found.is_empty() {
        println!(
            "\n{} Task(s) not found: {}",
            "!".yellow(),
            not_found.join(", ")
        );
    }

    if changed > 0 {
        println!(
            "\n{} Changed {} task(s) to {}",
            "✓".green(),
            changed,
            status_str.green()
        );
    }

    Ok(())
}

/// Single task mode (backward compatible)
fn run_single_task(
    storage: &Storage,
    task_id: &str,
    status_str: &str,
    tag: Option<&str>,
) -> Result<()> {
    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let epic_tag = resolve_group_tag(storage, tag, true)?;
    let mut epic = storage.load_group(&epic_tag)?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    task.set_status(new_status);

    storage.update_group(&epic_tag, &epic)?;

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}

/// Get list of tags to operate on
fn get_target_tags(storage: &Storage, tag: Option<&str>, all_tags: bool) -> Result<Vec<String>> {
    if all_tags {
        let phases = storage.load_tasks()?;
        Ok(phases.keys().cloned().collect())
    } else {
        let epic_tag = resolve_group_tag(storage, tag, true)?;
        Ok(vec![epic_tag])
    }
}
