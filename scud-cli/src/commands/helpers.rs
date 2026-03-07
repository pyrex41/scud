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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use crate::models::task::Task;

    // ── is_task_spawnable ───────────────────────────────────────────────

    #[test]
    fn spawnable_pending_task() {
        let mut phase = Phase::new("t".into());
        phase.add_task(Task::new("1".into(), "A".into(), "d".into()));
        assert!(is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn not_spawnable_in_progress() {
        let mut phase = Phase::new("t".into());
        let mut task = Task::new("1".into(), "A".into(), "d".into());
        task.set_status(TaskStatus::InProgress);
        phase.add_task(task);
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn not_spawnable_done() {
        let mut phase = Phase::new("t".into());
        let mut task = Task::new("1".into(), "A".into(), "d".into());
        task.set_status(TaskStatus::Done);
        phase.add_task(task);
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn not_spawnable_failed() {
        let mut phase = Phase::new("t".into());
        let mut task = Task::new("1".into(), "A".into(), "d".into());
        task.set_status(TaskStatus::Failed);
        phase.add_task(task);
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn not_spawnable_expanded_status() {
        let mut phase = Phase::new("t".into());
        let mut task = Task::new("1".into(), "A".into(), "d".into());
        task.set_status(TaskStatus::Expanded);
        phase.add_task(task);
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn not_spawnable_has_subtasks() {
        let mut phase = Phase::new("t".into());
        let mut parent = Task::new("1".into(), "Parent".into(), "d".into());
        parent.subtasks = vec!["1.1".into()];
        phase.add_task(parent);
        // Parent with subtasks is considered expanded regardless of status
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn spawnable_subtask_of_expanded_parent() {
        let mut phase = Phase::new("t".into());

        let mut parent = Task::new("1".into(), "Parent".into(), "d".into());
        parent.subtasks = vec!["1.1".into()];
        phase.add_task(parent);

        let mut child = Task::new("1.1".into(), "Child".into(), "d".into());
        child.parent_id = Some("1".into());
        phase.add_task(child);

        // Child of expanded parent is spawnable
        assert!(is_task_spawnable(&phase.tasks[1], &phase));
    }

    #[test]
    fn not_spawnable_subtask_of_unexpanded_parent() {
        let mut phase = Phase::new("t".into());

        // Parent with NO subtasks list (not expanded)
        let parent = Task::new("1".into(), "Parent".into(), "d".into());
        phase.add_task(parent);

        let mut child = Task::new("1.1".into(), "Child".into(), "d".into());
        child.parent_id = Some("1".into());
        phase.add_task(child);

        // Child of unexpanded parent is NOT spawnable
        assert!(!is_task_spawnable(&phase.tasks[1], &phase));
    }

    #[test]
    fn spawnable_subtask_with_missing_parent() {
        // Edge case: parent_id points to a task not in the phase
        let mut phase = Phase::new("t".into());
        let mut orphan = Task::new("1.1".into(), "Orphan".into(), "d".into());
        orphan.parent_id = Some("nonexistent".into());
        phase.add_task(orphan);

        // parent not found → parent_expanded defaults to false → not spawnable
        assert!(!is_task_spawnable(&phase.tasks[0], &phase));
    }

    #[test]
    fn spawnable_task_no_parent() {
        // Task with parent_id = None (top-level) is spawnable if pending
        let mut phase = Phase::new("t".into());
        let task = Task::new("1".into(), "Top".into(), "d".into());
        phase.add_task(task);
        assert!(is_task_spawnable(&phase.tasks[0], &phase));
    }

    // ── is_task_ready ───────────────────────────────────────────────────

    #[test]
    fn ready_no_deps() {
        let mut phase = Phase::new("t".into());
        phase.add_task(Task::new("1".into(), "A".into(), "d".into()));
        let all: Vec<&Task> = phase.tasks.iter().collect();
        assert!(is_task_ready(&phase.tasks[0], &phase, &all));
    }

    #[test]
    fn not_ready_dep_pending() {
        let mut phase = Phase::new("t".into());
        phase.add_task(Task::new("1".into(), "First".into(), "d".into()));
        let mut t2 = Task::new("2".into(), "Second".into(), "d".into());
        t2.dependencies = vec!["1".into()];
        phase.add_task(t2);
        let all: Vec<&Task> = phase.tasks.iter().collect();

        assert!(is_task_ready(&phase.tasks[0], &phase, &all));
        assert!(!is_task_ready(&phase.tasks[1], &phase, &all));
    }

    #[test]
    fn ready_dep_done() {
        let mut phase = Phase::new("t".into());
        let mut t1 = Task::new("1".into(), "First".into(), "d".into());
        t1.set_status(TaskStatus::Done);
        phase.add_task(t1);

        let mut t2 = Task::new("2".into(), "Second".into(), "d".into());
        t2.dependencies = vec!["1".into()];
        phase.add_task(t2);
        let all: Vec<&Task> = phase.tasks.iter().collect();

        // Task 1 is done so not ready (not Pending)
        assert!(!is_task_ready(&phase.tasks[0], &phase, &all));
        // Task 2 is pending, dep is done → ready
        assert!(is_task_ready(&phase.tasks[1], &phase, &all));
    }

    #[test]
    fn not_ready_expanded_even_with_deps_met() {
        let mut phase = Phase::new("t".into());
        let mut t1 = Task::new("1".into(), "First".into(), "d".into());
        t1.set_status(TaskStatus::Done);
        phase.add_task(t1);

        let mut t2 = Task::new("2".into(), "Second".into(), "d".into());
        t2.dependencies = vec!["1".into()];
        t2.subtasks = vec!["2.1".into()]; // expanded
        phase.add_task(t2);
        let all: Vec<&Task> = phase.tasks.iter().collect();

        // Deps met but task is expanded → not ready
        assert!(!is_task_ready(&phase.tasks[1], &phase, &all));
    }

    #[test]
    fn not_ready_subtask_parent_unexpanded_deps_met() {
        let mut phase = Phase::new("t".into());
        // Parent is pending with no subtasks (unexpanded)
        phase.add_task(Task::new("1".into(), "Parent".into(), "d".into()));

        let mut child = Task::new("1.1".into(), "Child".into(), "d".into());
        child.parent_id = Some("1".into());
        // No dependencies → deps trivially met
        phase.add_task(child);
        let all: Vec<&Task> = phase.tasks.iter().collect();

        // Parent is unexpanded → child not ready
        assert!(!is_task_ready(&phase.tasks[1], &phase, &all));
    }

    #[test]
    fn ready_chain_of_three() {
        let mut phase = Phase::new("t".into());
        let mut t1 = Task::new("1".into(), "A".into(), "d".into());
        t1.set_status(TaskStatus::Done);
        phase.add_task(t1);

        let mut t2 = Task::new("2".into(), "B".into(), "d".into());
        t2.dependencies = vec!["1".into()];
        t2.set_status(TaskStatus::Done);
        phase.add_task(t2);

        let mut t3 = Task::new("3".into(), "C".into(), "d".into());
        t3.dependencies = vec!["2".into()];
        phase.add_task(t3);

        let all: Vec<&Task> = phase.tasks.iter().collect();

        assert!(!is_task_ready(&phase.tasks[0], &phase, &all)); // done
        assert!(!is_task_ready(&phase.tasks[1], &phase, &all)); // done
        assert!(is_task_ready(&phase.tasks[2], &phase, &all));  // pending, dep chain met
    }

    #[test]
    fn not_ready_multiple_deps_one_unmet() {
        let mut phase = Phase::new("t".into());
        let mut t1 = Task::new("1".into(), "A".into(), "d".into());
        t1.set_status(TaskStatus::Done);
        phase.add_task(t1);

        phase.add_task(Task::new("2".into(), "B".into(), "d".into())); // still pending

        let mut t3 = Task::new("3".into(), "C".into(), "d".into());
        t3.dependencies = vec!["1".into(), "2".into()];
        phase.add_task(t3);

        let all: Vec<&Task> = phase.tasks.iter().collect();

        // Task 3 depends on both 1 (done) and 2 (pending) → not ready
        assert!(!is_task_ready(&phase.tasks[2], &phase, &all));
    }

    #[test]
    fn ready_multiple_deps_all_met() {
        let mut phase = Phase::new("t".into());
        let mut t1 = Task::new("1".into(), "A".into(), "d".into());
        t1.set_status(TaskStatus::Done);
        phase.add_task(t1);

        let mut t2 = Task::new("2".into(), "B".into(), "d".into());
        t2.set_status(TaskStatus::Done);
        phase.add_task(t2);

        let mut t3 = Task::new("3".into(), "C".into(), "d".into());
        t3.dependencies = vec!["1".into(), "2".into()];
        phase.add_task(t3);

        let all: Vec<&Task> = phase.tasks.iter().collect();
        assert!(is_task_ready(&phase.tasks[2], &phase, &all));
    }

    // ── flatten_all_tasks ───────────────────────────────────────────────

    #[test]
    fn flatten_empty() {
        let phases: HashMap<String, Phase> = HashMap::new();
        assert!(flatten_all_tasks(&phases).is_empty());
    }

    #[test]
    fn flatten_multiple_phases() {
        let mut phases = HashMap::new();

        let mut p1 = Phase::new("a".into());
        p1.add_task(Task::new("1".into(), "X".into(), "d".into()));
        phases.insert("a".into(), p1);

        let mut p2 = Phase::new("b".into());
        p2.add_task(Task::new("2".into(), "Y".into(), "d".into()));
        p2.add_task(Task::new("3".into(), "Z".into(), "d".into()));
        phases.insert("b".into(), p2);

        let flat = flatten_all_tasks(&phases);
        assert_eq!(flat.len(), 3);
    }
}
