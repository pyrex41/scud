//! Beads-style continuous execution mode
//!
//! Unlike wave-based execution which batches tasks and waits for all to complete,
//! beads-style execution uses continuous polling for ready tasks:
//!
//! 1. Query for all tasks where dependencies are met
//! 2. Claim task (mark in-progress)
//! 3. Spawn agent
//! 4. Immediately loop back to step 1 (no waiting for batch)
//!
//! This enables more fluid execution where downstream tasks can start
//! immediately when their dependencies complete, rather than waiting
//! for artificial wave boundaries.
//!
//! Inspired by the Beads project (https://github.com/steveyegge/beads)
//! and Gas Town's GUPP principle: "When an agent finds work on their hook,
//! they execute immediately. No confirmation. No questions. No waiting."

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use colored::Colorize;

// AgentDef is used for task prompt resolution
#[allow(unused_imports)]
use crate::agents::AgentDef;
use crate::commands::spawn::agent;
use crate::commands::spawn::terminal::{self, Harness};
use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

use super::session::{RoundState, SwarmSession};

/// Configuration for beads execution
pub struct BeadsConfig {
    /// Maximum concurrent agents
    pub max_concurrent: usize,
    /// Poll interval when no tasks are ready but some are in-progress
    pub poll_interval: Duration,
    /// Whether to run validation after each task completes
    pub validate_each: bool,
}

impl Default for BeadsConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            poll_interval: Duration::from_secs(3),
            validate_each: false,
        }
    }
}

/// Task info with tag for tracking
#[derive(Clone, Debug)]
pub struct ReadyTask {
    pub task: Task,
    pub tag: String,
}

/// Result of a beads execution run
pub struct BeadsResult {
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub total_duration: Duration,
}

/// Get all tasks that are ready to execute (dependencies met, not in-progress)
///
/// A task is ready when:
/// - Status is Pending
/// - Not expanded (or is subtask of expanded parent)
/// - All dependencies have status Done
/// - Not blocked by in-progress tasks (unlike waves, we allow execution while others run)
pub fn get_ready_tasks(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> Vec<ReadyTask> {
    let mut ready = Vec::new();

    // Collect all tasks as references for dependency checking
    let all_task_refs: Vec<&Task> = all_phases.values().flat_map(|p| &p.tasks).collect();

    // Determine which phases to check
    let phase_tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases
            .keys()
            .filter(|t| t.as_str() == phase_tag)
            .collect()
    };

    for tag in phase_tags {
        if let Some(phase) = all_phases.get(tag) {
            for task in &phase.tasks {
                if is_task_ready(task, phase, &all_task_refs) {
                    ready.push(ReadyTask {
                        task: task.clone(),
                        tag: tag.clone(),
                    });
                }
            }
        }
    }

    // Sort by priority (Critical > High > Medium > Low), then by ID
    ready.sort_by(|a, b| {
        use crate::models::task::Priority;
        let priority_ord = |p: &Priority| match p {
            Priority::Critical => 0,
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        };
        priority_ord(&a.task.priority)
            .cmp(&priority_ord(&b.task.priority))
            .then_with(|| a.task.id.cmp(&b.task.id))
    });

    ready
}

/// Check if a task is ready to execute
fn is_task_ready(task: &Task, phase: &Phase, all_tasks: &[&Task]) -> bool {
    // Must be pending
    if task.status != TaskStatus::Pending {
        return false;
    }

    // Skip expanded tasks (they have subtasks to do instead)
    if task.is_expanded() {
        return false;
    }

    // If subtask, parent must be expanded
    if let Some(ref parent_id) = task.parent_id {
        let parent_expanded = phase
            .get_task(parent_id)
            .map(|p| p.is_expanded())
            .unwrap_or(false);
        if !parent_expanded {
            return false;
        }
    }

    // All dependencies must be Done (not just "not pending")
    // This uses the effective dependencies which includes inherited parent deps
    task.has_dependencies_met_refs(all_tasks)
}

/// Count tasks currently in progress
pub fn count_in_progress(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> usize {
    let tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases
            .keys()
            .filter(|t| t.as_str() == phase_tag)
            .collect()
    };

    tags.iter()
        .filter_map(|tag| all_phases.get(*tag))
        .flat_map(|phase| &phase.tasks)
        .filter(|t| t.status == TaskStatus::InProgress)
        .count()
}

/// Count remaining tasks (pending or in-progress)
pub fn count_remaining(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> usize {
    let tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases
            .keys()
            .filter(|t| t.as_str() == phase_tag)
            .collect()
    };

    tags.iter()
        .filter_map(|tag| all_phases.get(*tag))
        .flat_map(|phase| &phase.tasks)
        .filter(|t| {
            t.status == TaskStatus::InProgress
                || (t.status == TaskStatus::Pending && !t.is_expanded())
        })
        .count()
}

/// Claim a task by marking it as in-progress
pub fn claim_task(storage: &Storage, task_id: &str, tag: &str) -> Result<bool> {
    let mut phase = storage.load_group(tag)?;

    if let Some(task) = phase.get_task_mut(task_id) {
        // Only claim if still pending (prevent race conditions)
        if task.status == TaskStatus::Pending {
            task.set_status(TaskStatus::InProgress);
            storage.update_group(tag, &phase)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Spawn an agent for a task using tmux
pub fn spawn_agent_tmux(
    ready_task: &ReadyTask,
    working_dir: &Path,
    session_name: &str,
    default_harness: Harness,
) -> Result<String> {
    // Resolve agent config (harness, model, prompt) from task's agent_type
    let config = agent::resolve_agent_config(
        &ready_task.task,
        &ready_task.tag,
        default_harness,
        None,
        working_dir,
    );

    // Spawn in tmux
    let window_index = terminal::spawn_terminal_with_harness_and_model(
        &ready_task.task.id,
        &config.prompt,
        working_dir,
        session_name,
        config.harness,
        config.model.as_deref(),
    )?;

    Ok(format!("{}:{}", session_name, window_index))
}

/// Main beads execution loop
///
/// Continuously polls for ready tasks and spawns agents immediately.
/// Does not wait for batches - new tasks can start as soon as their
/// dependencies complete.
pub fn run_beads_loop(
    storage: &Storage,
    phase_tag: &str,
    all_tags: bool,
    working_dir: &Path,
    session_name: &str,
    default_harness: Harness,
    config: &BeadsConfig,
    session: &mut SwarmSession,
) -> Result<BeadsResult> {
    let start_time = Instant::now();
    let mut tasks_completed = 0;
    let mut tasks_failed = 0;
    let mut spawned_tasks: HashSet<String> = HashSet::new();
    let mut round_state = RoundState::new(0); // Single continuous "round"

    println!();
    println!("{}", "Beads Execution Mode".cyan().bold());
    println!("{}", "═".repeat(50));
    println!(
        "  {} Continuous ready-task polling",
        "Mode:".dimmed()
    );
    println!(
        "  {} {}",
        "Max concurrent:".dimmed(),
        config.max_concurrent.to_string().cyan()
    );
    println!(
        "  {} {}ms",
        "Poll interval:".dimmed(),
        config.poll_interval.as_millis().to_string().cyan()
    );
    println!();

    loop {
        // Reload task state to see completed tasks
        let all_phases = storage.load_tasks()?;

        // Count current state
        let in_progress = count_in_progress(&all_phases, phase_tag, all_tags);
        let remaining = count_remaining(&all_phases, phase_tag, all_tags);

        // Check for completion
        if remaining == 0 {
            println!();
            println!("{}", "All tasks complete!".green().bold());
            break;
        }

        // Get ready tasks
        let ready_tasks = get_ready_tasks(&all_phases, phase_tag, all_tags);

        // Filter out tasks we've already spawned (in case status update is delayed)
        let ready_tasks: Vec<_> = ready_tasks
            .into_iter()
            .filter(|rt| !spawned_tasks.contains(&rt.task.id))
            .collect();

        if ready_tasks.is_empty() {
            if in_progress > 0 {
                // Some tasks running but none ready - wait for completion
                print!(
                    "\r  {} {} task(s) in progress, waiting...   ",
                    "⏳".dimmed(),
                    in_progress.to_string().cyan()
                );
                std::io::Write::flush(&mut std::io::stdout())?;
                thread::sleep(config.poll_interval);
                continue;
            } else {
                // No tasks ready and none in progress - might be blocked
                println!();
                println!(
                    "{}",
                    "No ready tasks and none in progress.".yellow()
                );
                println!(
                    "  {} {} remaining task(s) may be blocked.",
                    "!".yellow(),
                    remaining
                );
                println!("  Check for circular dependencies or missing dependencies.");
                break;
            }
        }

        // Clear waiting line if we were waiting
        print!("\r{}\r", " ".repeat(60));

        // Calculate how many we can spawn
        let available_slots = config.max_concurrent.saturating_sub(in_progress);
        let to_spawn = ready_tasks.into_iter().take(available_slots);

        // Spawn agents for ready tasks
        for ready_task in to_spawn {
            // Try to claim the task
            if !claim_task(storage, &ready_task.task.id, &ready_task.tag)? {
                // Task was claimed by another process or status changed
                continue;
            }

            // Mark as spawned locally
            spawned_tasks.insert(ready_task.task.id.clone());

            // Spawn agent
            match spawn_agent_tmux(&ready_task, working_dir, session_name, default_harness) {
                Ok(window_info) => {
                    println!(
                        "  {} Spawned: {} | {} [{}]",
                        "✓".green(),
                        ready_task.task.id.cyan(),
                        ready_task.task.title.dimmed(),
                        window_info.dimmed()
                    );
                    round_state.task_ids.push(ready_task.task.id.clone());
                    round_state.tags.push(ready_task.tag.clone());
                }
                Err(e) => {
                    println!(
                        "  {} Failed: {} - {}",
                        "✗".red(),
                        ready_task.task.id.red(),
                        e
                    );
                    round_state.failures.push(ready_task.task.id.clone());
                    tasks_failed += 1;

                    // Reset task status on spawn failure
                    if let Ok(mut phase) = storage.load_group(&ready_task.tag) {
                        if let Some(task) = phase.get_task_mut(&ready_task.task.id) {
                            task.set_status(TaskStatus::Failed);
                            let _ = storage.update_group(&ready_task.tag, &phase);
                        }
                    }
                }
            }
        }

        // Short sleep to avoid tight polling when at max capacity
        if in_progress >= config.max_concurrent {
            thread::sleep(config.poll_interval);
        } else {
            // Brief yield to allow other processes
            thread::sleep(Duration::from_millis(100));
        }
    }

    // Save session state
    let mut wave_state = super::session::WaveState::new(1);
    wave_state.rounds.push(round_state);
    session.waves.push(wave_state);

    // Count completed tasks (Done status that we spawned)
    let final_phases = storage.load_tasks()?;
    for task_id in &spawned_tasks {
        for phase in final_phases.values() {
            if let Some(task) = phase.get_task(task_id) {
                if task.status == TaskStatus::Done {
                    tasks_completed += 1;
                }
                break;
            }
        }
    }

    Ok(BeadsResult {
        tasks_completed,
        tasks_failed,
        total_duration: start_time.elapsed(),
    })
}

// Note: Beads extensions mode (async subprocess) is planned but not yet implemented.
// For now, beads mode uses tmux-based execution via run_beads_loop().

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::Priority;

    fn create_test_task(id: &str, status: TaskStatus, deps: Vec<&str>) -> Task {
        let mut task = Task::new(id.to_string(), format!("Task {}", id), "Description".to_string());
        task.status = status;
        task.dependencies = deps.into_iter().map(String::from).collect();
        task
    }

    #[test]
    fn test_get_ready_tasks_no_deps() {
        let mut phase = Phase::new("test".to_string());
        phase.tasks.push(create_test_task("1", TaskStatus::Pending, vec![]));
        phase.tasks.push(create_test_task("2", TaskStatus::Pending, vec![]));

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let ready = get_ready_tasks(&phases, "test", false);
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn test_get_ready_tasks_with_deps_met() {
        let mut phase = Phase::new("test".to_string());
        phase.tasks.push(create_test_task("1", TaskStatus::Done, vec![]));
        phase.tasks.push(create_test_task("2", TaskStatus::Pending, vec!["1"]));

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let ready = get_ready_tasks(&phases, "test", false);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.id, "2");
    }

    #[test]
    fn test_get_ready_tasks_with_deps_not_met() {
        let mut phase = Phase::new("test".to_string());
        phase.tasks.push(create_test_task("1", TaskStatus::InProgress, vec![]));
        phase.tasks.push(create_test_task("2", TaskStatus::Pending, vec!["1"]));

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let ready = get_ready_tasks(&phases, "test", false);
        assert_eq!(ready.len(), 0);
    }

    #[test]
    fn test_get_ready_tasks_skips_expanded() {
        let mut phase = Phase::new("test".to_string());
        let mut expanded_task = create_test_task("1", TaskStatus::Expanded, vec![]);
        expanded_task.subtasks = vec!["1.1".to_string()];
        phase.tasks.push(expanded_task);

        let mut subtask = create_test_task("1.1", TaskStatus::Pending, vec![]);
        subtask.parent_id = Some("1".to_string());
        phase.tasks.push(subtask);

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let ready = get_ready_tasks(&phases, "test", false);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].task.id, "1.1");
    }

    #[test]
    fn test_get_ready_tasks_priority_sort() {
        let mut phase = Phase::new("test".to_string());

        let mut low = create_test_task("low", TaskStatus::Pending, vec![]);
        low.priority = Priority::Low;

        let mut critical = create_test_task("critical", TaskStatus::Pending, vec![]);
        critical.priority = Priority::Critical;

        let mut high = create_test_task("high", TaskStatus::Pending, vec![]);
        high.priority = Priority::High;

        phase.tasks.push(low);
        phase.tasks.push(critical);
        phase.tasks.push(high);

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let ready = get_ready_tasks(&phases, "test", false);
        assert_eq!(ready.len(), 3);
        assert_eq!(ready[0].task.id, "critical");
        assert_eq!(ready[1].task.id, "high");
        assert_eq!(ready[2].task.id, "low");
    }

    #[test]
    fn test_count_in_progress() {
        let mut phase = Phase::new("test".to_string());
        phase.tasks.push(create_test_task("1", TaskStatus::InProgress, vec![]));
        phase.tasks.push(create_test_task("2", TaskStatus::InProgress, vec![]));
        phase.tasks.push(create_test_task("3", TaskStatus::Pending, vec![]));
        phase.tasks.push(create_test_task("4", TaskStatus::Done, vec![]));

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        assert_eq!(count_in_progress(&phases, "test", false), 2);
    }

    #[test]
    fn test_count_remaining() {
        let mut phase = Phase::new("test".to_string());
        phase.tasks.push(create_test_task("1", TaskStatus::InProgress, vec![]));
        phase.tasks.push(create_test_task("2", TaskStatus::Pending, vec![]));
        phase.tasks.push(create_test_task("3", TaskStatus::Done, vec![]));
        phase.tasks.push(create_test_task("4", TaskStatus::Failed, vec![]));

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        assert_eq!(count_remaining(&phases, "test", false), 2); // InProgress + Pending
    }
}
