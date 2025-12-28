use anyhow::Result;
use std::path::PathBuf;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    limit: usize,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Load all phases for cross-tag dependency checking
    let all_phases = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&all_phases);

    if all_tags {
        run_all_tags(&all_phases, &all_tasks_flat, limit)
    } else {
        let phase_tag = resolve_group_tag(&storage, tag, true)?;
        let phase = all_phases
            .get(&phase_tag)
            .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

        run_single_phase(phase, &phase_tag, &all_tasks_flat, limit)
    }
}

fn run_single_phase(
    phase: &crate::models::phase::Phase,
    phase_tag: &str,
    all_tasks_flat: &[&crate::models::task::Task],
    limit: usize,
) -> Result<()> {
    // Filter tasks with cross-tag aware dependency checking
    let ready_tasks: Vec<_> = phase
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .filter(|t| t.has_dependencies_met_refs(all_tasks_flat))
        .take(limit)
        .collect();

    let output = serde_json::json!({
        "tag": phase_tag,
        "count": ready_tasks.len(),
        "tasks": ready_tasks.iter().map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "complexity": t.complexity,
                "priority": format!("{:?}", t.priority)
            })
        }).collect::<Vec<_>>()
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_all_tags(
    all_phases: &std::collections::HashMap<String, crate::models::phase::Phase>,
    all_tasks_flat: &[&crate::models::task::Task],
    limit: usize,
) -> Result<()> {
    // Collect pending tasks from ALL phases, filtering for actionable ones
    let mut pending_tasks: Vec<(&crate::models::task::Task, &str)> = Vec::new();

    for (tag, phase) in all_phases {
        for task in &phase.tasks {
            // Only include pending, non-expanded tasks
            if task.status == TaskStatus::Pending {
                // If it's a subtask, only include if parent is expanded
                if let Some(ref parent_id) = task.parent_id {
                    let parent_expanded = phase
                        .get_task(parent_id)
                        .map(|p| p.is_expanded())
                        .unwrap_or(false);
                    if parent_expanded {
                        pending_tasks.push((task, tag.as_str()));
                    }
                } else if !task.is_expanded() {
                    // Top-level task that's not expanded
                    pending_tasks.push((task, tag.as_str()));
                }
            }
        }
    }

    // Filter to only those with dependencies met
    let ready_tasks: Vec<_> = pending_tasks
        .iter()
        .filter(|(task, _)| task.has_dependencies_met_refs(all_tasks_flat))
        .take(limit)
        .collect();

    let output = serde_json::json!({
        "tag": "all",
        "count": ready_tasks.len(),
        "tasks": ready_tasks.iter().map(|(t, tag)| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "tag": tag,
                "complexity": t.complexity,
                "priority": format!("{:?}", t.priority)
            })
        }).collect::<Vec<_>>()
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
