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
    /// All pending tasks are locked by others
    AllLocked,
}

/// Find the next available task, considering locks
/// all_tasks should contain tasks from all phases for cross-tag dependency resolution
pub fn find_next_available<'a>(
    phase: &'a crate::models::phase::Phase,
    all_tasks: &[&Task],
    exclude_locked: bool,
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

    // Filter out locked tasks if requested
    if exclude_locked {
        let unlocked: Vec<_> = deps_met.iter().filter(|t| !t.is_locked()).collect();
        if unlocked.is_empty() {
            return NextTaskResult::AllLocked;
        }
        return NextTaskResult::Available(unlocked[0]);
    }

    NextTaskResult::Available(deps_met[0])
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    claim: bool,
    name: Option<&str>,
    release: bool,
    spawn: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let phase_tag = resolve_group_tag(&storage, tag, true)?;

    // Handle --release mode
    if release {
        let agent_name =
            name.ok_or_else(|| anyhow::anyhow!("--name is required with --release"))?;
        return handle_release(&storage, &phase_tag, agent_name);
    }

    // Handle --claim mode (experimental dynamic-wave)
    if claim {
        let agent_name = name.ok_or_else(|| anyhow::anyhow!("--name is required with --claim"))?;
        return handle_claim(&storage, &phase_tag, agent_name);
    }

    // Standard next task behavior (read-only)
    let tasks = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&tasks);
    let phase = tasks
        .get(&phase_tag)
        .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

    // Handle --spawn mode (machine-readable JSON output)
    if spawn {
        match find_next_available(phase, &all_tasks_flat, true) {
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

    match find_next_available(phase, &all_tasks_flat, false) {
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
        NextTaskResult::AllLocked => {
            println!("{}", "All available tasks are currently locked".yellow());
            println!("Run: scud whois  # to see who's working on what");
        }
    }

    Ok(())
}

fn handle_claim(storage: &Storage, phase_tag: &str, agent_name: &str) -> Result<()> {
    println!(
        "{}",
        "[EXPERIMENTAL] Dynamic-wave mode: claiming next task"
            .yellow()
            .bold()
    );
    println!();

    // Load all phases for cross-tag dependency checking
    let all_phases = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&all_phases);

    // Use atomic update_group to hold lock across read-modify-write cycle
    // This prevents race conditions when multiple agents claim simultaneously
    let mut phase = storage.load_group(phase_tag)?;

    // Find next available task (exclude locked ones)
    let task_id = {
        let pending_tasks: Vec<_> = phase
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect();

        if pending_tasks.is_empty() {
            println!("{}", "No pending tasks available".yellow());
            println!();
            println!("{}", "All tasks may be:".blue());
            println!("  - Already done");
            println!("  - In progress by others");
            println!("  - Blocked by dependencies");
            println!();
            println!("Run: scud list  # to see all tasks");
            println!("Run: scud stats  # to see completion status");
            return Ok(());
        }

        // Find first task with dependencies met that isn't locked (cross-tag aware)
        let available: Vec<_> = pending_tasks
            .iter()
            .filter(|t| t.has_dependencies_met_refs(&all_tasks_flat) && !t.is_locked())
            .collect();

        if available.is_empty() {
            // Check if blocked by deps or by locks
            let deps_met: Vec<_> = pending_tasks
                .iter()
                .filter(|t| t.has_dependencies_met_refs(&all_tasks_flat))
                .collect();

            if deps_met.is_empty() {
                println!(
                    "{}",
                    "No tasks available - all pending tasks blocked by dependencies"
                        .yellow()
                        .bold()
                );
                println!();
                println!("{}", "Possible causes:".blue());
                println!("  - Dependencies not marked as done");
                println!("  - Circular dependency issues");
                println!("  - Dependencies on cancelled/blocked tasks");
                println!();
                println!("Run: scud doctor  # to diagnose stuck states");
            } else {
                println!(
                    "{}",
                    "No tasks available - all eligible tasks are locked by other agents"
                        .yellow()
                        .bold()
                );
                println!();
                println!("{}", "Currently locked tasks:".blue());
                for task in deps_met {
                    if let Some(ref locked_by) = task.locked_by {
                        println!(
                            "  {} - {} (locked by {})",
                            task.id.cyan(),
                            task.title,
                            locked_by.green()
                        );
                    }
                }
                println!();
                println!("Run: scud whois  # to see all assignments");
                println!("Run: scud doctor  # to check for stale locks");
            }
            return Ok(());
        }

        available[0].id.clone()
    };

    // Claim the task
    let task = phase
        .get_task_mut(&task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found", task_id))?;

    task.claim(agent_name).map_err(|e| anyhow::anyhow!(e))?;
    task.set_status(TaskStatus::InProgress);

    // Get task details before saving
    let task_title = task.title.clone();
    let task_description = task.description.clone();
    let task_complexity = task.complexity;
    let task_details = task.details.clone();
    let task_test_strategy = task.test_strategy.clone();

    // Use atomic update_group which holds lock across read-modify-write
    storage.update_group(phase_tag, &phase)?;

    // Print claimed task details
    println!("{}", "Task claimed successfully!".green().bold());
    println!();
    println!("{:<20} {}", "ID:".yellow(), task_id.cyan());
    println!("{:<20} {}", "Title:".yellow(), task_title.bold());
    println!("{:<20} {}", "Complexity:".yellow(), task_complexity);
    println!("{:<20} {}", "Claimed by:".yellow(), agent_name.green());
    println!("{:<20} {}", "Status:".yellow(), "in-progress".cyan());
    println!();
    println!("{}", "Description:".yellow());
    println!("{}", task_description);

    if let Some(details) = &task_details {
        println!();
        println!("{}", "Technical Details:".yellow());
        println!("{}", details);
    }

    if let Some(test_strategy) = &task_test_strategy {
        println!();
        println!("{}", "Test Strategy:".yellow());
        println!("{}", test_strategy);
    }

    // Critical: Status discipline messaging
    println!();
    println!("{}", "=".repeat(60).yellow());
    println!("{}", "IMPORTANT: Status Update Required".red().bold());
    println!("{}", "=".repeat(60).yellow());
    println!();
    println!(
        "{}",
        "When you complete this task, you MUST run:".yellow().bold()
    );
    println!();
    println!(
        "    {}",
        format!("scud set-status {} done", task_id).cyan().bold()
    );
    println!();
    println!(
        "{}",
        "This ensures the workflow stays healthy and other agents".dimmed()
    );
    println!("{}", "can claim dependent tasks.".dimmed());
    println!();

    Ok(())
}

fn handle_release(storage: &Storage, phase_tag: &str, agent_name: &str) -> Result<()> {
    println!(
        "{}",
        "[EXPERIMENTAL] Releasing tasks for agent".yellow().bold()
    );
    println!();

    // Use atomic update_group to hold lock across read-modify-write cycle
    let mut phase = storage.load_group(phase_tag)?;

    // Find tasks locked by this agent
    let mut released_count = 0;
    for task in &mut phase.tasks {
        if task.is_locked_by(agent_name) {
            let task_id = task.id.clone();
            let task_title = task.title.clone();
            // Clear both lock and assignment for clean release
            task.release();
            task.assigned_to = None;
            // Reset status back to pending if it was in-progress
            if task.status == TaskStatus::InProgress {
                task.set_status(TaskStatus::Pending);
            }
            println!(
                "{} Released: {} - {}",
                "✓".green(),
                task_id.cyan(),
                task_title
            );
            released_count += 1;
        }
    }

    if released_count == 0 {
        println!(
            "{}",
            format!("No tasks found locked by '{}'", agent_name).yellow()
        );
        return Ok(());
    }

    // Use atomic update_group which holds lock across read-modify-write
    storage.update_group(phase_tag, &phase)?;

    println!();
    println!("{} {} task(s) released", "✓".green(), released_count);

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

    if task.is_locked() {
        if let Some(ref locked_by) = task.locked_by {
            println!(
                "{:<20} {} (by {})",
                "Status:".yellow(),
                "LOCKED".red(),
                locked_by
            );
        }
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
    println!();
    println!(
        "{}",
        "Or use experimental dynamic-wave mode:".blue().dimmed()
    );
    println!(
        "  scud next --claim --name <your-name>  {}",
        "# auto-claims next task".dimmed()
    );
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

        match find_next_available(&phase, &all_tasks, false) {
            NextTaskResult::Available(task) => {
                assert_eq!(task.id, "2");
            }
            _ => panic!("Expected Available result"),
        }
    }

    #[test]
    fn test_find_next_available_exclude_locked() {
        let mut phase = create_test_phase();

        // Lock task 2
        phase.get_task_mut("2").unwrap().claim("alice").unwrap();

        let all_tasks = get_task_refs(&phase);

        // Without exclude_locked, should still find task 2
        match find_next_available(&phase, &all_tasks, false) {
            NextTaskResult::Available(task) => {
                assert_eq!(task.id, "2");
            }
            _ => panic!("Expected Available result"),
        }

        // With exclude_locked, should return AllLocked
        match find_next_available(&phase, &all_tasks, true) {
            NextTaskResult::AllLocked => {}
            _ => panic!("Expected AllLocked result"),
        }
    }

    #[test]
    fn test_find_next_no_pending() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Done".to_string(), "Desc".to_string());
        task.set_status(TaskStatus::Done);
        phase.add_task(task);

        let all_tasks = get_task_refs(&phase);

        match find_next_available(&phase, &all_tasks, false) {
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
        match find_next_available(&phase, &all_tasks, false) {
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

        match find_next_available(&phase, &all_tasks, false) {
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
        match find_next_available(&phase, &all_tasks, false) {
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
        match find_next_available(&phase, &all_tasks, false) {
            NextTaskResult::BlockedByDependencies => {}
            _ => panic!("Expected BlockedByDependencies with cross-tag dep not met"),
        }
    }
}
