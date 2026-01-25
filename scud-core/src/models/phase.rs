use super::task::{Task, TaskStatus};
use serde::{Deserialize, Serialize};

/// ID format for task generation
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IdFormat {
    /// Sequential numeric IDs: "1", "2", "3", subtasks: "1.1", "1.2"
    #[default]
    Sequential,
    /// UUID v4 IDs: 32-character hex strings
    Uuid,
}

impl IdFormat {
    /// Convert to string representation for SCG format
    pub fn as_str(&self) -> &'static str {
        match self {
            IdFormat::Sequential => "sequential",
            IdFormat::Uuid => "uuid",
        }
    }

    /// Parse from string representation
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "uuid" => IdFormat::Uuid,
            _ => IdFormat::Sequential,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    pub tasks: Vec<Task>,
    /// ID format used for this phase (default: Sequential for backwards compatibility)
    #[serde(default)]
    pub id_format: IdFormat,
}

impl Phase {
    pub fn new(name: String) -> Self {
        Phase {
            name,
            tasks: Vec::new(),
            id_format: IdFormat::default(),
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == task_id)
    }

    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    pub fn remove_task(&mut self, task_id: &str) -> Option<Task> {
        self.tasks
            .iter()
            .position(|t| t.id == task_id)
            .map(|idx| self.tasks.remove(idx))
    }

    pub fn get_stats(&self) -> PhaseStats {
        let mut total = 0;
        let mut pending = 0;
        let mut in_progress = 0;
        let mut done = 0;
        let mut blocked = 0;
        let mut expanded = 0;
        let mut total_complexity = 0;

        for task in &self.tasks {
            // Don't count subtasks in total (they're part of parent)
            if task.is_subtask() {
                continue;
            }

            total += 1;

            // Only count complexity for non-expanded tasks
            // (expanded tasks have their work represented by subtasks)
            if !task.is_expanded() {
                total_complexity += task.complexity;
            }

            match task.status {
                TaskStatus::Pending => pending += 1,
                TaskStatus::InProgress => in_progress += 1,
                TaskStatus::Done => done += 1,
                TaskStatus::Blocked => blocked += 1,
                TaskStatus::Expanded => {
                    // Check if all subtasks are done - if so, count as done
                    let all_subtasks_done = task.subtasks.iter().all(|subtask_id| {
                        self.get_task(subtask_id)
                            .map(|st| st.status == TaskStatus::Done)
                            .unwrap_or(false)
                    });
                    if all_subtasks_done && !task.subtasks.is_empty() {
                        done += 1;
                    } else {
                        expanded += 1;
                    }
                }
                _ => {}
            }
        }

        PhaseStats {
            total,
            pending,
            in_progress,
            done,
            blocked,
            expanded,
            total_complexity,
        }
    }

    /// Get actionable tasks (not expanded, not subtasks of incomplete parents)
    pub fn get_actionable_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| {
                // Exclude expanded parents (work on subtasks instead)
                if t.is_expanded() {
                    return false;
                }
                // Include subtasks only if they're actionable
                if let Some(ref parent_id) = t.parent_id {
                    // Parent must be expanded
                    self.get_task(parent_id)
                        .map(|p| p.is_expanded())
                        .unwrap_or(false)
                } else {
                    // Top-level task
                    true
                }
            })
            .collect()
    }

    /// Find next task using only local phase tasks for dependency checking.
    /// For cross-tag dependency support, use find_next_task_cross_tag instead.
    pub fn find_next_task(&self) -> Option<&Task> {
        self.tasks.iter().find(|task| {
            task.status == TaskStatus::Pending && task.has_dependencies_met(&self.tasks)
        })
    }

    /// Find next task with cross-tag dependency support.
    /// Pass flattened tasks from all phases for proper dependency resolution.
    pub fn find_next_task_cross_tag<'a>(&'a self, all_tasks: &[&Task]) -> Option<&'a Task> {
        self.tasks.iter().find(|task| {
            task.status == TaskStatus::Pending && task.has_dependencies_met_refs(all_tasks)
        })
    }

    pub fn get_tasks_needing_expansion(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.needs_expansion()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub done: usize,
    pub blocked: usize,
    pub expanded: usize,
    pub total_complexity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::{Task, TaskStatus};

    #[test]
    fn test_phase_creation() {
        let phase = Phase::new("phase-1-auth".to_string());

        assert_eq!(phase.name, "phase-1-auth");
        assert!(phase.tasks.is_empty());
    }

    #[test]
    fn test_add_task() {
        let mut phase = Phase::new("phase-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );

        phase.add_task(task.clone());

        assert_eq!(phase.tasks.len(), 1);
        assert_eq!(phase.tasks[0].id, "TASK-1");
    }

    #[test]
    fn test_get_task() {
        let mut phase = Phase::new("phase-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        phase.add_task(task);

        let retrieved = phase.get_task("TASK-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "TASK-1");

        let missing = phase.get_task("TASK-99");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_task_mut() {
        let mut phase = Phase::new("phase-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        phase.add_task(task);

        {
            let task_mut = phase.get_task_mut("TASK-1").unwrap();
            task_mut.set_status(TaskStatus::InProgress);
        }

        assert_eq!(
            phase.get_task("TASK-1").unwrap().status,
            TaskStatus::InProgress
        );
    }

    #[test]
    fn test_remove_task() {
        let mut phase = Phase::new("phase-1".to_string());
        let task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        let task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        phase.add_task(task1);
        phase.add_task(task2);

        let removed = phase.remove_task("TASK-1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "TASK-1");
        assert_eq!(phase.tasks.len(), 1);
        assert_eq!(phase.tasks[0].id, "TASK-2");

        let missing = phase.remove_task("TASK-99");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_stats_empty_phase() {
        let phase = Phase::new("phase-1".to_string());
        let stats = phase.get_stats();

        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.done, 0);
        assert_eq!(stats.blocked, 0);
        assert_eq!(stats.total_complexity, 0);
    }

    #[test]
    fn test_get_stats_with_tasks() {
        let mut phase = Phase::new("phase-1".to_string());

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.complexity = 3;
        task1.set_status(TaskStatus::Done);

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        task2.complexity = 5;
        task2.set_status(TaskStatus::InProgress);

        let mut task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );
        task3.complexity = 8;
        // Pending by default

        let mut task4 = Task::new(
            "TASK-4".to_string(),
            "Task 4".to_string(),
            "Desc".to_string(),
        );
        task4.complexity = 2;
        task4.set_status(TaskStatus::Blocked);

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);
        phase.add_task(task4);

        let stats = phase.get_stats();

        assert_eq!(stats.total, 4);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.done, 1);
        assert_eq!(stats.blocked, 1);
        assert_eq!(stats.total_complexity, 18); // 3 + 5 + 8 + 2
    }

    #[test]
    fn test_find_next_task_no_dependencies() {
        let mut phase = Phase::new("phase-1".to_string());

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        // Pending, no dependencies

        phase.add_task(task1);
        phase.add_task(task2);

        let next = phase.find_next_task();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "TASK-2"); // First pending task
    }

    #[test]
    fn test_find_next_task_with_dependencies() {
        let mut phase = Phase::new("phase-1".to_string());

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        // Pending, no dependencies

        let mut task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );
        task3.dependencies = vec!["TASK-1".to_string(), "TASK-2".to_string()];
        // Pending, but depends on TASK-2 which is not done

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);

        let next = phase.find_next_task();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "TASK-2"); // TASK-3 blocked by dependencies
    }

    #[test]
    fn test_find_next_task_none_available() {
        let mut phase = Phase::new("phase-1".to_string());

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        task2.set_status(TaskStatus::InProgress);

        phase.add_task(task1);
        phase.add_task(task2);

        let next = phase.find_next_task();
        assert!(next.is_none()); // No pending tasks
    }

    #[test]
    fn test_phase_serialization() {
        let mut phase = Phase::new("phase-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        phase.add_task(task);

        let json = serde_json::to_string(&phase).unwrap();
        let deserialized: Phase = serde_json::from_str(&json).unwrap();

        assert_eq!(phase.name, deserialized.name);
        assert_eq!(phase.tasks.len(), deserialized.tasks.len());
        assert_eq!(phase.tasks[0].id, deserialized.tasks[0].id);
    }

    #[test]
    fn test_id_format_parse() {
        assert_eq!(IdFormat::parse("sequential"), IdFormat::Sequential);
        assert_eq!(IdFormat::parse("uuid"), IdFormat::Uuid);
        assert_eq!(IdFormat::parse("UUID"), IdFormat::Uuid);
        assert_eq!(IdFormat::parse("unknown"), IdFormat::Sequential); // Default
    }

    #[test]
    fn test_id_format_as_str() {
        assert_eq!(IdFormat::Sequential.as_str(), "sequential");
        assert_eq!(IdFormat::Uuid.as_str(), "uuid");
    }
}
