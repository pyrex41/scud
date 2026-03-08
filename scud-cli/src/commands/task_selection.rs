use std::collections::HashMap;

use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};

/// Return phase values in the requested scope (single tag or all tags).
pub fn scoped_phases<'a>(
    all_phases: &'a HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> Vec<&'a Phase> {
    if all_tags {
        all_phases.values().collect()
    } else {
        all_phases.get(phase_tag).into_iter().collect()
    }
}

/// Shared "actionable pending task" predicate used by spawn/scheduler paths.
///
/// A task is actionable when:
/// - status is pending
/// - task itself is not expanded
/// - if it is a subtask, the parent is expanded
pub fn is_actionable_pending_task(task: &Task, phase: &Phase) -> bool {
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

/// Count in-progress tasks for the requested scope.
pub fn count_in_progress_tasks(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> usize {
    scoped_phases(all_phases, phase_tag, all_tags)
        .into_iter()
        .flat_map(|phase| &phase.tasks)
        .filter(|task| task.status == TaskStatus::InProgress)
        .count()
}
