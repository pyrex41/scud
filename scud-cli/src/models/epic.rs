use serde::{Deserialize, Serialize};
use super::task::Task;

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
