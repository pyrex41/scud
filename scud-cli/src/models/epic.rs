use super::task::Task;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epic {
    pub name: String,
    pub tasks: Vec<Task>,
}

impl Epic {
    pub fn new(name: String) -> Self {
        Epic {
            name,
            tasks: Vec::new(),
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

    pub fn get_stats(&self) -> EpicStats {
        let total = self.tasks.len();
        let mut pending = 0;
        let mut in_progress = 0;
        let mut done = 0;
        let mut blocked = 0;
        let mut total_complexity = 0;

        for task in &self.tasks {
            total_complexity += task.complexity;
            match task.status {
                super::task::TaskStatus::Pending => pending += 1,
                super::task::TaskStatus::InProgress => in_progress += 1,
                super::task::TaskStatus::Done => done += 1,
                super::task::TaskStatus::Blocked => blocked += 1,
                _ => {}
            }
        }

        EpicStats {
            total,
            pending,
            in_progress,
            done,
            blocked,
            total_complexity,
        }
    }

    pub fn find_next_task(&self) -> Option<&Task> {
        self.tasks.iter().find(|task| {
            task.status == super::task::TaskStatus::Pending
                && task.has_dependencies_met(&self.tasks)
        })
    }

    pub fn get_tasks_needing_expansion(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.needs_expansion()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub done: usize,
    pub blocked: usize,
    pub total_complexity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::{Task, TaskStatus};

    #[test]
    fn test_epic_creation() {
        let epic = Epic::new("epic-1-auth".to_string());

        assert_eq!(epic.name, "epic-1-auth");
        assert!(epic.tasks.is_empty());
    }

    #[test]
    fn test_add_task() {
        let mut epic = Epic::new("epic-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );

        epic.add_task(task.clone());

        assert_eq!(epic.tasks.len(), 1);
        assert_eq!(epic.tasks[0].id, "TASK-1");
    }

    #[test]
    fn test_get_task() {
        let mut epic = Epic::new("epic-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        epic.add_task(task);

        let retrieved = epic.get_task("TASK-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "TASK-1");

        let missing = epic.get_task("TASK-99");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_task_mut() {
        let mut epic = Epic::new("epic-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        epic.add_task(task);

        {
            let task_mut = epic.get_task_mut("TASK-1").unwrap();
            task_mut.set_status(TaskStatus::InProgress);
        }

        assert_eq!(
            epic.get_task("TASK-1").unwrap().status,
            TaskStatus::InProgress
        );
    }

    #[test]
    fn test_remove_task() {
        let mut epic = Epic::new("epic-1".to_string());
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
        epic.add_task(task1);
        epic.add_task(task2);

        let removed = epic.remove_task("TASK-1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "TASK-1");
        assert_eq!(epic.tasks.len(), 1);
        assert_eq!(epic.tasks[0].id, "TASK-2");

        let missing = epic.remove_task("TASK-99");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_stats_empty_epic() {
        let epic = Epic::new("epic-1".to_string());
        let stats = epic.get_stats();

        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.done, 0);
        assert_eq!(stats.blocked, 0);
        assert_eq!(stats.total_complexity, 0);
    }

    #[test]
    fn test_get_stats_with_tasks() {
        let mut epic = Epic::new("epic-1".to_string());

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

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);
        epic.add_task(task4);

        let stats = epic.get_stats();

        assert_eq!(stats.total, 4);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.done, 1);
        assert_eq!(stats.blocked, 1);
        assert_eq!(stats.total_complexity, 18); // 3 + 5 + 8 + 2
    }

    #[test]
    fn test_find_next_task_no_dependencies() {
        let mut epic = Epic::new("epic-1".to_string());

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

        let task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );
        // Pending, no dependencies

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);

        let next = epic.find_next_task();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "TASK-2"); // First pending task
    }

    #[test]
    fn test_find_next_task_with_dependencies() {
        let mut epic = Epic::new("epic-1".to_string());

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

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);

        let next = epic.find_next_task();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "TASK-2"); // TASK-3 blocked by dependencies
    }

    #[test]
    fn test_find_next_task_dependencies_met() {
        let mut epic = Epic::new("epic-1".to_string());

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
        task2.set_status(TaskStatus::Done);

        let mut task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );
        task3.dependencies = vec!["TASK-1".to_string(), "TASK-2".to_string()];
        // Pending, dependencies met

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);

        let next = epic.find_next_task();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, "TASK-3"); // Dependencies met
    }

    #[test]
    fn test_find_next_task_none_available() {
        let mut epic = Epic::new("epic-1".to_string());

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

        epic.add_task(task1);
        epic.add_task(task2);

        let next = epic.find_next_task();
        assert!(next.is_none()); // No pending tasks
    }

    #[test]
    fn test_get_tasks_needing_expansion() {
        let mut epic = Epic::new("epic-1".to_string());

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Small Task".to_string(),
            "Desc".to_string(),
        );
        task1.complexity = 5;

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Medium Task".to_string(),
            "Desc".to_string(),
        );
        task2.complexity = 13;

        let mut task3 = Task::new(
            "TASK-3".to_string(),
            "Large Task".to_string(),
            "Desc".to_string(),
        );
        task3.complexity = 21;

        let mut task4 = Task::new(
            "TASK-4".to_string(),
            "Huge Task".to_string(),
            "Desc".to_string(),
        );
        task4.complexity = 34;

        epic.add_task(task1);
        epic.add_task(task2);
        epic.add_task(task3);
        epic.add_task(task4);

        let needing_expansion = epic.get_tasks_needing_expansion();

        assert_eq!(needing_expansion.len(), 4); // All tasks with complexity >= 3
        assert!(needing_expansion.iter().any(|t| t.id == "TASK-1"));
        assert!(needing_expansion.iter().any(|t| t.id == "TASK-2"));
        assert!(needing_expansion.iter().any(|t| t.id == "TASK-3"));
        assert!(needing_expansion.iter().any(|t| t.id == "TASK-4"));
    }

    #[test]
    fn test_epic_serialization() {
        let mut epic = Epic::new("epic-1".to_string());
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        epic.add_task(task);

        let json = serde_json::to_string(&epic).unwrap();
        let deserialized: Epic = serde_json::from_str(&json).unwrap();

        assert_eq!(epic.name, deserialized.name);
        assert_eq!(epic.tasks.len(), deserialized.tasks.len());
        assert_eq!(epic.tasks[0].id, deserialized.tasks[0].id);
    }
}
