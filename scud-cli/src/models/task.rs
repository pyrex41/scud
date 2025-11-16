use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Done,
    Review,
    Blocked,
    Deferred,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Done => "done",
            TaskStatus::Review => "review",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Deferred => "deferred",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "in-progress" => Some(TaskStatus::InProgress),
            "done" => Some(TaskStatus::Done),
            "review" => Some(TaskStatus::Review),
            "blocked" => Some(TaskStatus::Blocked),
            "deferred" => Some(TaskStatus::Deferred),
            "cancelled" => Some(TaskStatus::Cancelled),
            _ => None,
        }
    }

    pub fn all() -> Vec<&'static str> {
        vec!["pending", "in-progress", "done", "review", "blocked", "deferred", "cancelled"]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,

    #[serde(default)]
    pub status: TaskStatus,

    #[serde(default)]
    pub complexity: u32,

    #[serde(default)]
    pub priority: Priority,

    #[serde(default)]
    pub dependencies: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_strategy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_analysis: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    // Parallel execution support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
}

impl Task {
    pub fn new(id: String, title: String, description: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Task {
            id,
            title,
            description,
            status: TaskStatus::Pending,
            complexity: 0,
            priority: Priority::Medium,
            dependencies: Vec::new(),
            details: None,
            test_strategy: None,
            complexity_analysis: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            assigned_to: None,
            locked_by: None,
            locked_at: None,
        }
    }

    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn update(&mut self) {
        self.updated_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn has_dependencies_met(&self, all_tasks: &[Task]) -> bool {
        self.dependencies.iter().all(|dep_id| {
            all_tasks
                .iter()
                .find(|t| &t.id == dep_id)
                .map(|t| t.status == TaskStatus::Done)
                .unwrap_or(false)
        })
    }

    pub fn needs_expansion(&self) -> bool {
        self.complexity > 13
    }

    // Assignment and locking methods
    pub fn assign(&mut self, assignee: &str) {
        self.assigned_to = Some(assignee.to_string());
        self.update();
    }

    pub fn claim(&mut self, assignee: &str) -> Result<(), String> {
        if let Some(ref locked_by) = self.locked_by {
            if locked_by != assignee {
                return Err(format!("Task is locked by {}", locked_by));
            }
        }

        self.assigned_to = Some(assignee.to_string());
        self.locked_by = Some(assignee.to_string());
        self.locked_at = Some(chrono::Utc::now().to_rfc3339());
        self.update();
        Ok(())
    }

    pub fn release(&mut self) {
        self.locked_by = None;
        self.locked_at = None;
        self.update();
    }

    pub fn is_locked(&self) -> bool {
        self.locked_by.is_some()
    }

    pub fn is_locked_by(&self, assignee: &str) -> bool {
        self.locked_by.as_ref().map(|s| s == assignee).unwrap_or(false)
    }

    pub fn is_assigned_to(&self, assignee: &str) -> bool {
        self.assigned_to.as_ref().map(|s| s == assignee).unwrap_or(false)
    }

    pub fn lock_age_hours(&self) -> Option<f64> {
        self.locked_at.as_ref().and_then(|locked_at| {
            chrono::DateTime::parse_from_rfc3339(locked_at)
                .ok()
                .map(|dt| {
                    let now = chrono::Utc::now();
                    let duration = now.signed_duration_since(dt);
                    duration.num_seconds() as f64 / 3600.0
                })
        })
    }

    pub fn is_stale_lock(&self, hours_threshold: f64) -> bool {
        self.lock_age_hours()
            .map(|hours| hours > hours_threshold)
            .unwrap_or(false)
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}
