use anyhow::Result;
use colored::Colorize;
use dialoguer::Select;
use std::collections::HashMap;

use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

/// Flatten all tasks from all phases into a single Vec for cross-tag dependency checking
pub fn flatten_all_tasks(all_phases: &HashMap<String, Phase>) -> Vec<&Task> {
    all_phases
        .values()
        .flat_map(|phase| phase.tasks.iter())
        .collect()
}

/// Check if we're running in an interactive terminal
pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}

/// Find the next available task across phases
///
/// Returns the task and its tag if found
pub fn find_next_task(
    storage: &Storage,
    tag: Option<&str>,
    all_tags: bool,
) -> Option<(Task, String)> {
    let tasks = storage.load_tasks().ok()?;
    let all_tasks_flat = flatten_all_tasks(&tasks);

    if all_tags {
        for (phase_tag, phase) in &tasks {
            for task in &phase.tasks {
                if is_task_ready(task, phase, &all_tasks_flat) {
                    return Some((task.clone(), phase_tag.clone()));
                }
            }
        }
        None
    } else {
        let phase_tag = tag
            .map(String::from)
            .or_else(|| storage.get_active_group().ok().flatten())?;
        let phase = tasks.get(&phase_tag)?;

        for task in &phase.tasks {
            if is_task_ready(task, phase, &all_tasks_flat) {
                return Some((task.clone(), phase_tag.clone()));
            }
        }
        None
    }
}

/// Check if a task is a candidate for execution (pending, not expanded, parent expanded).
///
/// This is the shared base predicate used by spawn, swarm, and beads modes.
/// It does NOT check dependency satisfaction — callers that need that should
/// use [`is_task_ready`] instead, or handle deps separately (e.g., Kahn's algorithm).
pub fn is_task_spawnable(task: &Task, phase: &Phase) -> bool {
    if task.status != TaskStatus::Pending {
        return false;
    }
    if task.is_expanded() {
        return false;
    }
    if let Some(ref parent_id) = task.parent_id {
        let parent_expanded = phase
            .get_task(parent_id)
            .map(|p| p.is_expanded())
            .unwrap_or(false);
        if !parent_expanded {
            return false;
        }
    }
    true
}

/// Check if a task is ready to execute: spawnable AND all dependencies met.
pub fn is_task_ready(task: &Task, phase: &Phase, all_tasks: &[&Task]) -> bool {
    is_task_spawnable(task, phase) && task.has_dependencies_met_refs(all_tasks)
}

/// Resolve task group tag with fallback to active group and interactive selection
///
/// Priority:
/// 1. Explicit --tag argument
/// 2. Active group (from .scud/active-tag)
/// 3. Interactive selection (if TTY available)
/// 4. Error with helpful message
pub fn resolve_group_tag(
    storage: &Storage,
    explicit_tag: Option<&str>,
    allow_interactive: bool,
) -> Result<String> {
    // Priority 1: Explicit --tag argument
    if let Some(tag) = explicit_tag {
        let tasks = storage.load_tasks()?;
        if !tasks.contains_key(tag) {
            anyhow::bail!("Task group '{}' not found. Run: scud tags", tag);
        }
        return Ok(tag.to_string());
    }

    // Priority 2: Active group
    if let Some(active) = storage.get_active_group()? {
        return Ok(active);
    }

    // Priority 3: Interactive selection
    if allow_interactive && is_interactive() {
        let tasks = storage.load_tasks()?;
        if tasks.is_empty() {
            anyhow::bail!(
                "No task groups found. Create one with: scud parse-prd <file> --tag <tag>"
            );
        }

        let mut tags: Vec<&String> = tasks.keys().collect();
        tags.sort();

        // Show selection prompt
        println!("{}", "No active task group set.".yellow());
        let selection = Select::new()
            .with_prompt("Select a task group")
            .items(&tags)
            .default(0)
            .interact()?;

        let selected = tags[selection].clone();

        // Set as active for next time
        storage.set_active_group(&selected)?;
        println!("{} {}", "Active group set to:".green(), selected.green());

        return Ok(selected);
    }

    // Priority 4: Error
    anyhow::bail!("No active task group. Use --tag <tag> or run: scud tags <tag>")
}
