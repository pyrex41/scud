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
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}
