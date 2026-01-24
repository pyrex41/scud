//! Ready command - beads-style continuous worker support
//!
//! Returns tasks that are ready to execute (pending, dependencies met).
//! Unlike `next-batch`, this command:
//! - Sorts by priority (high first) then complexity (low first)
//! - Supports both human-readable and JSON output
//! - Is designed for continuous worker loops (beads/GUPP pattern)
//!
//! Note: Tasks that are "InProgress" are considered "claimed" and filtered out by default.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::models::task::{Priority, Task, TaskStatus};
use crate::storage::Storage;

/// Information about a ready task including its tag
#[derive(Debug, Clone)]
pub struct ReadyTask<'a> {
    pub task: &'a Task,
    pub tag: String,
}

/// Get all ready tasks across specified tags
///
/// A task is "ready" if:
/// - Status is Pending
/// - Not expanded (has subtasks)
/// - If subtask, parent must be expanded
/// - All dependencies are met (status Done)
///
/// If `include_in_progress` is true, also includes tasks with InProgress status.
pub fn get_ready_tasks<'a>(
    all_phases: &'a std::collections::HashMap<String, crate::models::phase::Phase>,
    all_tasks_flat: &[&Task],
    tags: &[String],
    include_in_progress: bool,
    limit: Option<usize>,
) -> Vec<ReadyTask<'a>> {
    let mut ready_tasks: Vec<ReadyTask<'a>> = Vec::new();

    for tag in tags {
        if let Some(phase) = all_phases.get(tag) {
            for task in &phase.tasks {
                // Check status - must be Pending (or InProgress if requested)
                let valid_status = match task.status {
                    TaskStatus::Pending => true,
                    TaskStatus::InProgress => include_in_progress,
                    _ => false,
                };
                if !valid_status {
                    continue;
                }

                // Skip expanded parent tasks (work on subtasks instead)
                if task.is_expanded() {
                    continue;
                }

                // If subtask, parent must be expanded
                if let Some(ref parent_id) = task.parent_id {
                    let parent_expanded = phase
                        .get_task(parent_id)
                        .map(|p| p.is_expanded())
                        .unwrap_or(false);
                    if !parent_expanded {
                        continue;
                    }
                }

                // Dependencies must be met
                if !task.has_dependencies_met_refs(all_tasks_flat) {
                    continue;
                }

                ready_tasks.push(ReadyTask {
                    task,
                    tag: tag.clone(),
                });
            }
        }
    }

    // Sort by priority (High > Medium > Low > None) then complexity (low first)
    ready_tasks.sort_by(|a, b| {
        // Priority: Critical (0) comes before Low (3)
        let priority_order = |p: &Priority| -> u8 {
            match p {
                Priority::Critical => 0,
                Priority::High => 1,
                Priority::Medium => 2,
                Priority::Low => 3,
            }
        };

        let pa = priority_order(&a.task.priority);
        let pb = priority_order(&b.task.priority);

        // Compare priority first, then complexity (lower is easier/faster)
        pa.cmp(&pb).then(a.task.complexity.cmp(&b.task.complexity))
    });

    // Apply limit if specified
    if let Some(limit) = limit {
        ready_tasks.truncate(limit);
    }

    ready_tasks
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
    limit: usize,
    json: bool,
    include_in_progress: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Load all phases for cross-tag dependency checking
    let all_phases = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&all_phases);

    // Determine which tags to search
    let tags: Vec<String> = if all_tags {
        all_phases.keys().cloned().collect()
    } else {
        let phase_tag = resolve_group_tag(&storage, tag, !json)?;
        vec![phase_tag]
    };

    let ready = get_ready_tasks(
        &all_phases,
        &all_tasks_flat,
        &tags,
        include_in_progress,
        Some(limit),
    );

    if json {
        output_json(&ready)?;
    } else {
        output_human(&ready)?;
    }

    Ok(())
}

fn output_json(ready: &[ReadyTask]) -> Result<()> {
    let output = serde_json::json!({
        "count": ready.len(),
        "tasks": ready.iter().map(|r| {
            let mut obj = serde_json::json!({
                "id": r.task.id,
                "title": r.task.title,
                "complexity": r.task.complexity,
                "priority": format!("{:?}", r.task.priority),
                "tag": r.tag,
            });
            // Mark if task is already in-progress (claimed)
            if r.task.status == TaskStatus::InProgress {
                obj["in_progress"] = serde_json::Value::Bool(true);
            }
            if let Some(ref assigned) = r.task.assigned_to {
                obj["assigned_to"] = serde_json::Value::String(assigned.clone());
            }
            obj
        }).collect::<Vec<_>>()
    });

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn output_human(ready: &[ReadyTask]) -> Result<()> {
    if ready.is_empty() {
        println!("{}", "No ready tasks".yellow());
        println!();
        println!("This could mean:");
        println!("  - All tasks are complete");
        println!("  - All pending tasks are blocked by dependencies");
        println!("  - All ready tasks are already in-progress (use --include-in-progress to see)");
        println!();
        println!("Run: scud list --status pending    # see blocked tasks");
        println!("Run: scud waves --all-tags         # see dependency graph");
        return Ok(());
    }

    println!(
        "{} {} task(s)",
        "Ready:".green().bold(),
        ready.len().to_string().cyan()
    );
    println!();

    for (i, r) in ready.iter().enumerate() {
        let priority_str = match r.task.priority {
            Priority::Critical => "CRIT".red().bold().to_string(),
            Priority::High => "HIGH".red().to_string(),
            Priority::Medium => "MED".yellow().to_string(),
            Priority::Low => "LOW".dimmed().to_string(),
        };

        let status_str = if r.task.status == TaskStatus::InProgress {
            format!(" {}", "(in-progress)".yellow())
        } else {
            String::new()
        };

        println!(
            "{}. {} [{}] {} (C:{}){}",
            (i + 1).to_string().dimmed(),
            r.task.id.cyan(),
            r.tag.blue(),
            r.task.title.bold(),
            r.task.complexity,
            status_str
        );
        println!("   Priority: {}", priority_str);

        // Truncate description for display
        let truncated: String = r.task.description.chars().take(100).collect();
        let suffix = if r.task.description.len() > 100 {
            "..."
        } else {
            ""
        };
        println!("   {}{}", truncated.dimmed(), suffix.dimmed());
        println!();
    }

    println!("{}", "To start a task:".blue());
    if let Some(first) = ready.first() {
        println!(
            "  scud set-status {} in-progress --tag {}",
            first.task.id, first.tag
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use std::collections::HashMap;

    fn create_test_phases() -> HashMap<String, Phase> {
        let mut phases = HashMap::new();

        // Create "core" phase
        let mut core_phase = Phase::new("core".to_string());

        let mut task1 =
            Task::new("1".to_string(), "Setup database".to_string(), "Desc".to_string());
        task1.set_status(TaskStatus::Done);
        task1.priority = Priority::High;

        let mut task2 =
            Task::new("2".to_string(), "Create models".to_string(), "Desc".to_string());
        task2.dependencies = vec!["1".to_string()];
        task2.priority = Priority::High;
        task2.complexity = 3;

        let mut task3 = Task::new("3".to_string(), "Add logging".to_string(), "Desc".to_string());
        task3.priority = Priority::Low;
        task3.complexity = 1;

        core_phase.add_task(task1);
        core_phase.add_task(task2);
        core_phase.add_task(task3);

        phases.insert("core".to_string(), core_phase);

        // Create "api" phase with cross-tag dependency
        let mut api_phase = Phase::new("api".to_string());

        let mut api_task = Task::new(
            "api:1".to_string(),
            "Create endpoints".to_string(),
            "Desc".to_string(),
        );
        api_task.dependencies = vec!["core:2".to_string()]; // Depends on core phase
        api_task.priority = Priority::Medium;

        api_phase.add_task(api_task);

        phases.insert("api".to_string(), api_phase);

        phases
    }

    #[test]
    fn test_get_ready_tasks_basic() {
        let phases = create_test_phases();
        let all_tasks_flat: Vec<&Task> = phases.values().flat_map(|p| &p.tasks).collect();

        let ready = get_ready_tasks(&phases, &all_tasks_flat, &["core".to_string()], false, None);

        // Should have task 2 (deps met) and task 3 (no deps)
        assert_eq!(ready.len(), 2);

        // Task 2 (High priority) should come before task 3 (Low priority)
        assert_eq!(ready[0].task.id, "2"); // High priority
        assert_eq!(ready[1].task.id, "3"); // Low priority
    }

    #[test]
    fn test_get_ready_tasks_respects_limit() {
        let phases = create_test_phases();
        let all_tasks_flat: Vec<&Task> = phases.values().flat_map(|p| &p.tasks).collect();

        let ready =
            get_ready_tasks(&phases, &all_tasks_flat, &["core".to_string()], false, Some(1));

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.id, "2"); // Highest priority
    }

    #[test]
    fn test_get_ready_tasks_filters_in_progress() {
        let mut phases = create_test_phases();

        // Mark task 2 as in-progress
        if let Some(phase) = phases.get_mut("core") {
            if let Some(task) = phase.get_task_mut("2") {
                task.set_status(TaskStatus::InProgress);
            }
        }

        let all_tasks_flat: Vec<&Task> = phases.values().flat_map(|p| &p.tasks).collect();

        // Without include_in_progress, should only get task 3
        let ready = get_ready_tasks(&phases, &all_tasks_flat, &["core".to_string()], false, None);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.id, "3");

        // With include_in_progress, should get both
        let ready_with_in_progress =
            get_ready_tasks(&phases, &all_tasks_flat, &["core".to_string()], true, None);
        assert_eq!(ready_with_in_progress.len(), 2);
    }

    #[test]
    fn test_get_ready_tasks_cross_tag() {
        let mut phases = HashMap::new();

        // Create "core" phase with namespaced IDs
        let mut core_phase = Phase::new("core".to_string());

        let mut task1 =
            Task::new("core:1".to_string(), "Setup database".to_string(), "Desc".to_string());
        task1.set_status(TaskStatus::Done);
        task1.priority = Priority::High;

        let mut task2 =
            Task::new("core:2".to_string(), "Create models".to_string(), "Desc".to_string());
        task2.dependencies = vec!["core:1".to_string()];
        task2.priority = Priority::High;
        task2.complexity = 3;
        task2.set_status(TaskStatus::Done); // Mark as done so api:1 is ready

        let mut task3 = Task::new("core:3".to_string(), "Add logging".to_string(), "Desc".to_string());
        task3.priority = Priority::Low;
        task3.complexity = 1;

        core_phase.add_task(task1);
        core_phase.add_task(task2);
        core_phase.add_task(task3);

        phases.insert("core".to_string(), core_phase);

        // Create "api" phase with cross-tag dependency
        let mut api_phase = Phase::new("api".to_string());

        let mut api_task = Task::new(
            "api:1".to_string(),
            "Create endpoints".to_string(),
            "Desc".to_string(),
        );
        api_task.dependencies = vec!["core:2".to_string()]; // Depends on core phase
        api_task.priority = Priority::Medium;

        api_phase.add_task(api_task);

        phases.insert("api".to_string(), api_phase);

        let all_tasks_flat: Vec<&Task> = phases.values().flat_map(|p| &p.tasks).collect();

        let ready = get_ready_tasks(
            &phases,
            &all_tasks_flat,
            &["core".to_string(), "api".to_string()],
            false,
            None,
        );

        // Should have task core:3 (pending, no deps) and api:1 (dep core:2 is done)
        let ids: Vec<&str> = ready.iter().map(|r| r.task.id.as_str()).collect();
        assert!(ids.contains(&"core:3"), "Expected core:3 to be ready, got: {:?}", ids);
        assert!(ids.contains(&"api:1"), "Expected api:1 to be ready, got: {:?}", ids);
    }
}
