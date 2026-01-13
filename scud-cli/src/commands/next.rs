use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

/// Result of finding the next task
pub enum NextTaskResult<'a> {
    /// Found a task with dependencies met
    Available(&'a crate::models::task::Task),
    /// No pending tasks at all
    NoPendingTasks,
    /// Pending tasks exist but blocked by dependencies
    BlockedByDependencies,
}

/// Find the next available task
/// all_tasks should contain tasks from all phases for cross-tag dependency resolution
pub fn find_next_available<'a>(
    phase: &'a crate::models::phase::Phase,
    all_tasks: &[&Task],
) -> NextTaskResult<'a> {
    let pending_tasks: Vec<_> = phase
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .collect();

    if pending_tasks.is_empty() {
        return NextTaskResult::NoPendingTasks;
    }

    // Find tasks with dependencies met (checking across all phases)
    let deps_met: Vec<_> = pending_tasks
        .iter()
        .filter(|t| t.has_dependencies_met_refs(all_tasks))
        .collect();

    if deps_met.is_empty() {
        return NextTaskResult::BlockedByDependencies;
    }

    NextTaskResult::Available(deps_met[0])
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    spawn: bool,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let tasks = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&tasks);

    if all_tags {
        // Search across ALL phases for the next available task
        run_all_tags(&tasks, &all_tasks_flat, spawn)
    } else {
        // Standard single-phase behavior
        let phase_tag = resolve_group_tag(&storage, tag, true)?;
        let phase = tasks
            .get(&phase_tag)
            .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

        run_single_phase(phase, &phase_tag, &all_tasks_flat, spawn)
    }
}

fn run_single_phase(
    phase: &crate::models::phase::Phase,
    phase_tag: &str,
    all_tasks_flat: &[&Task],
    spawn: bool,
) -> Result<()> {
    // Handle --spawn mode (machine-readable JSON output)
    if spawn {
        match find_next_available(phase, all_tasks_flat) {
            NextTaskResult::Available(task) => {
                let output = serde_json::json!({
                    "task_id": task.id,
                    "title": task.title,
                    "tag": phase_tag,
                    "complexity": task.complexity,
                });
                println!("{}", serde_json::to_string(&output)?);
            }
            _ => {
                println!("null");
            }
        }
        return Ok(());
    }

    match find_next_available(phase, all_tasks_flat) {
        NextTaskResult::Available(task) => {
            print_task_details(task);
            print_standard_instructions(&task.id);
        }
        NextTaskResult::NoPendingTasks => {
            println!("{}", "All tasks completed or in progress!".green().bold());
            println!("Run: scud list --status in-progress");
        }
        NextTaskResult::BlockedByDependencies => {
            println!(
                "{}",
                "No available tasks - all pending tasks blocked by dependencies".yellow()
            );
            println!("Run: scud list --status pending");
            println!("Run: scud doctor  # to diagnose stuck states");
        }
    }

    Ok(())
}

fn run_all_tags(
    all_phases: &std::collections::HashMap<String, crate::models::phase::Phase>,
    all_tasks_flat: &[&Task],
    spawn: bool,
) -> Result<()> {
    // Collect pending tasks from ALL phases, filtering for actionable ones
    let mut pending_tasks: Vec<(&Task, &str)> = Vec::new();

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

    // Find first task with all dependencies met (including cross-tag and inherited)
    let available = pending_tasks
        .iter()
        .find(|(task, _)| task.has_dependencies_met_refs(all_tasks_flat));

    if spawn {
        match available {
            Some((task, tag)) => {
                let output = serde_json::json!({
                    "task_id": task.id,
                    "title": task.title,
                    "tag": tag,
                    "complexity": task.complexity,
                });
                println!("{}", serde_json::to_string(&output)?);
            }
            None => {
                println!("null");
            }
        }
        return Ok(());
    }

    match available {
        Some((task, tag)) => {
            println!("{} {}", "Phase:".dimmed(), tag.cyan());
            print_task_details(task);
            print_standard_instructions(&task.id);
        }
        None => {
            if pending_tasks.is_empty() {
                println!("{}", "All tasks completed or in progress!".green().bold());
                println!("Run: scud list --status in-progress");
            } else {
                println!(
                    "{}",
                    "No available tasks - all pending tasks blocked by dependencies".yellow()
                );
                println!(
                    "Pending tasks exist in {} phase(s), but all are blocked.",
                    pending_tasks.len()
                );
                println!("Run: scud waves --all-tags  # to see dependency graph");
                println!("Run: scud doctor  # to diagnose stuck states");
            }
        }
    }

    Ok(())
}

fn print_task_details(task: &crate::models::task::Task) {
    println!("{}", "Next Available Task:".green().bold());
    println!();
    println!("{:<20} {}", "ID:".yellow(), task.id.cyan());
    println!("{:<20} {}", "Title:".yellow(), task.title.bold());
    println!("{:<20} {}", "Complexity:".yellow(), task.complexity);
    println!("{:<20} {:?}", "Priority:".yellow(), task.priority);

    if let Some(ref assigned) = task.assigned_to {
        println!("{:<20} {}", "Assigned to:".yellow(), assigned.green());
    }

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
}

fn print_standard_instructions(task_id: &str) {
    println!();
    println!("{}", "To start this task:".blue());
    println!("  scud set-status {} in-progress", task_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use crate::models::task::{Task, TaskStatus};

    fn create_test_phase() -> Phase {
        let mut phase = Phase::new("test-phase".to_string());

        let mut task1 = Task::new("1".to_string(), "Task 1".to_string(), "Desc 1".to_string());
        task1.set_status(TaskStatus::Done);

        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), "Desc 2".to_string());
        task2.dependencies = vec!["1".to_string()];
        // task2 is pending with deps met

        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), "Desc 3".to_string());
        task3.dependencies = vec!["2".to_string()];
        // task3 is pending with deps NOT met

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);

        phase
    }

    /// Helper to get task refs from phase for testing
    fn get_task_refs(phase: &Phase) -> Vec<&Task> {
        phase.tasks.iter().collect()
    }

    #[test]
    fn test_find_next_available_basic() {
        let phase = create_test_phase();
        let all_tasks = get_task_refs(&phase);

        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::Available(task) => {
                assert_eq!(task.id, "2");
            }
            _ => panic!("Expected Available result"),
        }
    }

    #[test]
    fn test_find_next_no_pending() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Done".to_string(), "Desc".to_string());
        task.set_status(TaskStatus::Done);
        phase.add_task(task);

        let all_tasks = get_task_refs(&phase);

        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::NoPendingTasks => {}
            _ => panic!("Expected NoPendingTasks result"),
        }
    }

    #[test]
    fn test_find_next_blocked_by_deps() {
        let mut phase = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "Task 1".to_string(), "Desc".to_string());
        // task1 is pending

        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), "Desc".to_string());
        task2.dependencies = vec!["1".to_string()];
        // task2 depends on pending task1

        // Add task2 first, task1 second (so task2 is checked first)
        phase.add_task(task2);
        phase.add_task(task1);

        let all_tasks = get_task_refs(&phase);

        // task1 should be found since it has no deps
        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::Available(task) => {
                assert_eq!(task.id, "1");
            }
            _ => panic!("Expected task 1 to be available"),
        }
    }

    #[test]
    fn test_find_next_all_blocked() {
        let mut phase = Phase::new("test".to_string());

        let mut task1 = Task::new("1".to_string(), "Task 1".to_string(), "Desc".to_string());
        task1.dependencies = vec!["nonexistent".to_string()];
        // task1 depends on non-existent task

        phase.add_task(task1);

        let all_tasks = get_task_refs(&phase);

        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::BlockedByDependencies => {}
            _ => panic!("Expected BlockedByDependencies result"),
        }
    }

    #[test]
    fn test_find_next_cross_tag_dependency() {
        // Create a phase with a task that depends on a task from another "phase"
        let mut phase = Phase::new("api".to_string());
        let mut api_task = Task::new(
            "api:1".to_string(),
            "API Task".to_string(),
            "Desc".to_string(),
        );
        api_task.dependencies = vec!["auth:1".to_string()]; // Depends on auth phase
        phase.add_task(api_task);

        // Create "auth" task (simulating another phase)
        let mut auth_task = Task::new(
            "auth:1".to_string(),
            "Auth Task".to_string(),
            "Desc".to_string(),
        );
        auth_task.set_status(TaskStatus::Done);

        // Combine all tasks (simulating flattened all_phases)
        let all_tasks: Vec<&Task> = vec![&phase.tasks[0], &auth_task];

        // With cross-tag tasks included, dependency should be met
        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::Available(task) => {
                assert_eq!(task.id, "api:1");
            }
            _ => panic!("Expected Available result with cross-tag dependency met"),
        }
    }

    #[test]
    fn test_find_next_cross_tag_dependency_not_met() {
        // Create a phase with a task that depends on a task from another "phase"
        let mut phase = Phase::new("api".to_string());
        let mut api_task = Task::new(
            "api:1".to_string(),
            "API Task".to_string(),
            "Desc".to_string(),
        );
        api_task.dependencies = vec!["auth:1".to_string()]; // Depends on auth phase
        phase.add_task(api_task);

        // Create "auth" task (NOT done)
        let auth_task = Task::new(
            "auth:1".to_string(),
            "Auth Task".to_string(),
            "Desc".to_string(),
        );

        // Combine all tasks (simulating flattened all_phases)
        let all_tasks: Vec<&Task> = vec![&phase.tasks[0], &auth_task];

        // With cross-tag dep NOT met, should be blocked
        match find_next_available(&phase, &all_tasks) {
            NextTaskResult::BlockedByDependencies => {}
            _ => panic!("Expected BlockedByDependencies with cross-tag dep not met"),
        }
    }
}
