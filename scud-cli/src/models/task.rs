use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    #[default]
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

    #[allow(clippy::should_implement_trait)]
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
        vec![
            "pending",
            "in-progress",
            "done",
            "review",
            "blocked",
            "deferred",
            "cancelled",
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    #[default]
    Medium,
    Low,
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
    // Validation constants
    const MAX_TITLE_LENGTH: usize = 200;
    const MAX_DESCRIPTION_LENGTH: usize = 5000;
    const VALID_FIBONACCI_NUMBERS: &'static [u32] = &[0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];

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

    /// Validate task ID - must contain only alphanumeric characters and hyphens
    pub fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("Task ID cannot be empty".to_string());
        }

        if id.len() > 100 {
            return Err("Task ID too long (max 100 characters)".to_string());
        }

        let valid_chars = id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

        if !valid_chars {
            return Err(
                "Task ID can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
            );
        }

        Ok(())
    }

    /// Validate title - must not be empty and within length limit
    pub fn validate_title(title: &str) -> Result<(), String> {
        if title.trim().is_empty() {
            return Err("Task title cannot be empty".to_string());
        }

        if title.len() > Self::MAX_TITLE_LENGTH {
            return Err(format!(
                "Task title too long (max {} characters)",
                Self::MAX_TITLE_LENGTH
            ));
        }

        Ok(())
    }

    /// Validate description - within length limit
    pub fn validate_description(description: &str) -> Result<(), String> {
        if description.len() > Self::MAX_DESCRIPTION_LENGTH {
            return Err(format!(
                "Task description too long (max {} characters)",
                Self::MAX_DESCRIPTION_LENGTH
            ));
        }

        Ok(())
    }

    /// Validate complexity - must be a Fibonacci number
    pub fn validate_complexity(complexity: u32) -> Result<(), String> {
        if !Self::VALID_FIBONACCI_NUMBERS.contains(&complexity) {
            return Err(format!(
                "Complexity must be a Fibonacci number: {:?}",
                Self::VALID_FIBONACCI_NUMBERS
            ));
        }

        Ok(())
    }

    /// Sanitize text by removing potentially dangerous HTML/script tags
    pub fn sanitize_text(text: &str) -> String {
        text.replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    /// Comprehensive validation of all task fields
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Err(e) = Self::validate_id(&self.id) {
            errors.push(e);
        }

        if let Err(e) = Self::validate_title(&self.title) {
            errors.push(e);
        }

        if let Err(e) = Self::validate_description(&self.description) {
            errors.push(e);
        }

        if self.complexity > 0 {
            if let Err(e) = Self::validate_complexity(self.complexity) {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
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
        self.locked_by
            .as_ref()
            .map(|s| s == assignee)
            .unwrap_or(false)
    }

    pub fn is_assigned_to(&self, assignee: &str) -> bool {
        self.assigned_to
            .as_ref()
            .map(|s| s == assignee)
            .unwrap_or(false)
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

    /// Check if adding a dependency would create a circular reference
    /// Returns Err with the cycle path if circular dependency detected
    pub fn would_create_cycle(&self, new_dep_id: &str, all_tasks: &[Task]) -> Result<(), String> {
        if self.id == new_dep_id {
            return Err(format!("Self-reference: {} -> {}", self.id, new_dep_id));
        }

        let mut visited = std::collections::HashSet::new();
        let mut path = Vec::new();

        Self::detect_cycle_recursive(new_dep_id, &self.id, all_tasks, &mut visited, &mut path)
    }

    fn detect_cycle_recursive(
        current_id: &str,
        target_id: &str,
        all_tasks: &[Task],
        visited: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
    ) -> Result<(), String> {
        if current_id == target_id {
            path.push(current_id.to_string());
            return Err(format!("Circular dependency: {}", path.join(" -> ")));
        }

        if visited.contains(current_id) {
            return Ok(());
        }

        visited.insert(current_id.to_string());
        path.push(current_id.to_string());

        if let Some(task) = all_tasks.iter().find(|t| t.id == current_id) {
            for dep_id in &task.dependencies {
                Self::detect_cycle_recursive(dep_id, target_id, all_tasks, visited, path)?;
            }
        }

        path.pop();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );

        assert_eq!(task.id, "TASK-1");
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, "Description");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.complexity, 0);
        assert_eq!(task.priority, Priority::Medium);
        assert!(task.dependencies.is_empty());
        assert!(task.created_at.is_some());
        assert!(task.updated_at.is_some());
        assert!(task.assigned_to.is_none());
        assert!(task.locked_by.is_none());
        assert!(task.locked_at.is_none());
    }

    #[test]
    fn test_status_conversion() {
        assert_eq!(TaskStatus::Pending.as_str(), "pending");
        assert_eq!(TaskStatus::InProgress.as_str(), "in-progress");
        assert_eq!(TaskStatus::Done.as_str(), "done");
        assert_eq!(TaskStatus::Review.as_str(), "review");
        assert_eq!(TaskStatus::Blocked.as_str(), "blocked");
        assert_eq!(TaskStatus::Deferred.as_str(), "deferred");
        assert_eq!(TaskStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_status_from_string() {
        assert_eq!(TaskStatus::from_str("pending"), Some(TaskStatus::Pending));
        assert_eq!(
            TaskStatus::from_str("in-progress"),
            Some(TaskStatus::InProgress)
        );
        assert_eq!(TaskStatus::from_str("done"), Some(TaskStatus::Done));
        assert_eq!(TaskStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_set_status_updates_timestamp() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());
        let initial_updated = task.updated_at.clone();

        std::thread::sleep(std::time::Duration::from_millis(10));
        task.set_status(TaskStatus::InProgress);

        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.updated_at > initial_updated);
    }

    #[test]
    fn test_task_assignment() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.assign("alice");
        assert_eq!(task.assigned_to, Some("alice".to_string()));
        assert!(task.is_assigned_to("alice"));
        assert!(!task.is_assigned_to("bob"));
    }

    #[test]
    fn test_task_claim_success() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        let result = task.claim("alice");
        assert!(result.is_ok());
        assert_eq!(task.assigned_to, Some("alice".to_string()));
        assert_eq!(task.locked_by, Some("alice".to_string()));
        assert!(task.locked_at.is_some());
        assert!(task.is_locked());
        assert!(task.is_locked_by("alice"));
    }

    #[test]
    fn test_task_claim_already_locked_by_same_user() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.claim("alice").unwrap();
        let result = task.claim("alice");
        assert!(result.is_ok()); // Same user can re-claim
    }

    #[test]
    fn test_task_claim_already_locked_by_different_user() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.claim("alice").unwrap();
        let result = task.claim("bob");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task is locked by alice");
    }

    #[test]
    fn test_task_release() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.claim("alice").unwrap();
        assert!(task.is_locked());

        task.release();
        assert!(!task.is_locked());
        assert_eq!(task.locked_by, None);
        assert_eq!(task.locked_at, None);
        assert_eq!(task.assigned_to, Some("alice".to_string())); // Assignment persists
    }

    #[test]
    fn test_lock_age_calculation() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.claim("alice").unwrap();

        let age = task.lock_age_hours();
        assert!(age.is_some());
        assert!(age.unwrap() < 0.001); // Should be very recent (< 1 minute)
    }

    #[test]
    fn test_stale_lock_detection() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.claim("alice").unwrap();

        // Not stale immediately
        assert!(!task.is_stale_lock(24.0));

        // Simulate old lock by setting locked_at to 48 hours ago
        let two_days_ago = chrono::Utc::now() - chrono::Duration::hours(48);
        task.locked_at = Some(two_days_ago.to_rfc3339());

        assert!(task.is_stale_lock(24.0));
        assert!(!task.is_stale_lock(72.0));
    }

    #[test]
    fn test_has_dependencies_met_all_done() {
        let mut task = Task::new("TASK-3".to_string(), "Test".to_string(), "Desc".to_string());
        task.dependencies = vec!["TASK-1".to_string(), "TASK-2".to_string()];

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Dep 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Dep 2".to_string(),
            "Desc".to_string(),
        );
        task2.set_status(TaskStatus::Done);

        let all_tasks = vec![task1, task2];
        assert!(task.has_dependencies_met(&all_tasks));
    }

    #[test]
    fn test_has_dependencies_met_some_pending() {
        let mut task = Task::new("TASK-3".to_string(), "Test".to_string(), "Desc".to_string());
        task.dependencies = vec!["TASK-1".to_string(), "TASK-2".to_string()];

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Dep 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let task2 = Task::new(
            "TASK-2".to_string(),
            "Dep 2".to_string(),
            "Desc".to_string(),
        );
        // task2 is pending

        let all_tasks = vec![task1, task2];
        assert!(!task.has_dependencies_met(&all_tasks));
    }

    #[test]
    fn test_has_dependencies_met_missing_dependency() {
        let mut task = Task::new("TASK-3".to_string(), "Test".to_string(), "Desc".to_string());
        task.dependencies = vec!["TASK-1".to_string(), "TASK-MISSING".to_string()];

        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Dep 1".to_string(),
            "Desc".to_string(),
        );
        task1.set_status(TaskStatus::Done);

        let all_tasks = vec![task1];
        assert!(!task.has_dependencies_met(&all_tasks));
    }

    #[test]
    fn test_needs_expansion() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        task.complexity = 8;
        assert!(!task.needs_expansion());

        task.complexity = 13;
        assert!(!task.needs_expansion());

        task.complexity = 21;
        assert!(task.needs_expansion());
    }

    #[test]
    fn test_task_serialization() {
        let task = Task::new(
            "TASK-1".to_string(),
            "Test Task".to_string(),
            "Description".to_string(),
        );

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(task.id, deserialized.id);
        assert_eq!(task.title, deserialized.title);
        assert_eq!(task.description, deserialized.description);
    }

    #[test]
    fn test_task_serialization_with_optional_fields() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());
        task.details = Some("Detailed info".to_string());
        task.test_strategy = Some("Test plan".to_string());
        task.claim("alice").unwrap();

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(task.details, deserialized.details);
        assert_eq!(task.test_strategy, deserialized.test_strategy);
        assert_eq!(task.locked_by, deserialized.locked_by);
        assert_eq!(task.locked_at, deserialized.locked_at);
    }

    #[test]
    fn test_priority_default() {
        let default_priority = Priority::default();
        assert_eq!(default_priority, Priority::Medium);
    }

    #[test]
    fn test_status_all() {
        let all_statuses = TaskStatus::all();
        assert_eq!(all_statuses.len(), 7);
        assert!(all_statuses.contains(&"pending"));
        assert!(all_statuses.contains(&"in-progress"));
        assert!(all_statuses.contains(&"done"));
        assert!(all_statuses.contains(&"review"));
        assert!(all_statuses.contains(&"blocked"));
        assert!(all_statuses.contains(&"deferred"));
        assert!(all_statuses.contains(&"cancelled"));
    }

    #[test]
    fn test_circular_dependency_self_reference() {
        let task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());
        let all_tasks = vec![task.clone()];

        let result = task.would_create_cycle("TASK-1", &all_tasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Self-reference"));
    }

    #[test]
    fn test_circular_dependency_direct_cycle() {
        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.dependencies = vec!["TASK-2".to_string()];

        let task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );

        let all_tasks = vec![task1.clone(), task2.clone()];

        // Trying to add TASK-1 as dependency of TASK-2 would create cycle: TASK-2 -> TASK-1 -> TASK-2
        let result = task2.would_create_cycle("TASK-1", &all_tasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    #[test]
    fn test_circular_dependency_indirect_cycle() {
        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.dependencies = vec!["TASK-2".to_string()];

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        task2.dependencies = vec!["TASK-3".to_string()];

        let task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );

        let all_tasks = vec![task1.clone(), task2, task3.clone()];

        // Trying to add TASK-1 as dependency of TASK-3 would create cycle:
        // TASK-3 -> TASK-1 -> TASK-2 -> TASK-3
        let result = task3.would_create_cycle("TASK-1", &all_tasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    #[test]
    fn test_circular_dependency_no_cycle() {
        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.dependencies = vec!["TASK-3".to_string()];

        let task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );

        let task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );

        let all_tasks = vec![task1.clone(), task2.clone(), task3];

        // Adding TASK-2 as dependency of TASK-1 is fine (no cycle)
        let result = task1.would_create_cycle("TASK-2", &all_tasks);
        assert!(result.is_ok());
    }

    #[test]
    fn test_circular_dependency_complex_graph() {
        let mut task1 = Task::new(
            "TASK-1".to_string(),
            "Task 1".to_string(),
            "Desc".to_string(),
        );
        task1.dependencies = vec!["TASK-2".to_string(), "TASK-3".to_string()];

        let mut task2 = Task::new(
            "TASK-2".to_string(),
            "Task 2".to_string(),
            "Desc".to_string(),
        );
        task2.dependencies = vec!["TASK-4".to_string()];

        let mut task3 = Task::new(
            "TASK-3".to_string(),
            "Task 3".to_string(),
            "Desc".to_string(),
        );
        task3.dependencies = vec!["TASK-4".to_string()];

        let task4 = Task::new(
            "TASK-4".to_string(),
            "Task 4".to_string(),
            "Desc".to_string(),
        );

        let all_tasks = vec![task1.clone(), task2, task3, task4.clone()];

        // Adding TASK-1 as dependency of TASK-4 would create a cycle
        let result = task4.would_create_cycle("TASK-1", &all_tasks);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    // Validation tests
    #[test]
    fn test_validate_id_success() {
        assert!(Task::validate_id("TASK-123").is_ok());
        assert!(Task::validate_id("task_456").is_ok());
        assert!(Task::validate_id("Feature-789").is_ok());
    }

    #[test]
    fn test_validate_id_empty() {
        let result = Task::validate_id("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task ID cannot be empty");
    }

    #[test]
    fn test_validate_id_too_long() {
        let long_id = "A".repeat(101);
        let result = Task::validate_id(&long_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));
    }

    #[test]
    fn test_validate_id_invalid_characters() {
        assert!(Task::validate_id("TASK@123").is_err());
        assert!(Task::validate_id("TASK 123").is_err());
        assert!(Task::validate_id("TASK#123").is_err());
        assert!(Task::validate_id("TASK.123").is_err());
    }

    #[test]
    fn test_validate_title_success() {
        assert!(Task::validate_title("Valid title").is_ok());
        assert!(Task::validate_title("A").is_ok());
    }

    #[test]
    fn test_validate_title_empty() {
        let result = Task::validate_title("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task title cannot be empty");

        let result = Task::validate_title("   ");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task title cannot be empty");
    }

    #[test]
    fn test_validate_title_too_long() {
        let long_title = "A".repeat(201);
        let result = Task::validate_title(&long_title);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));
    }

    #[test]
    fn test_validate_description_success() {
        assert!(Task::validate_description("Valid description").is_ok());
        assert!(Task::validate_description("").is_ok());
    }

    #[test]
    fn test_validate_description_too_long() {
        let long_desc = "A".repeat(5001);
        let result = Task::validate_description(&long_desc);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));
    }

    #[test]
    fn test_validate_complexity_success() {
        assert!(Task::validate_complexity(0).is_ok());
        assert!(Task::validate_complexity(1).is_ok());
        assert!(Task::validate_complexity(2).is_ok());
        assert!(Task::validate_complexity(3).is_ok());
        assert!(Task::validate_complexity(5).is_ok());
        assert!(Task::validate_complexity(8).is_ok());
        assert!(Task::validate_complexity(13).is_ok());
        assert!(Task::validate_complexity(21).is_ok());
    }

    #[test]
    fn test_validate_complexity_invalid() {
        assert!(Task::validate_complexity(4).is_err());
        assert!(Task::validate_complexity(6).is_err());
        assert!(Task::validate_complexity(7).is_err());
        assert!(Task::validate_complexity(100).is_err());
    }

    #[test]
    fn test_sanitize_text() {
        assert_eq!(
            Task::sanitize_text("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
        assert_eq!(
            Task::sanitize_text("Normal text"),
            "Normal text"
        );
        assert_eq!(
            Task::sanitize_text("<div>Content</div>"),
            "&lt;div&gt;Content&lt;/div&gt;"
        );
    }

    #[test]
    fn test_validate_success() {
        let task = Task::new(
            "TASK-1".to_string(),
            "Valid title".to_string(),
            "Valid description".to_string(),
        );
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_validate_multiple_errors() {
        let mut task = Task::new(
            "TASK@INVALID".to_string(),
            "".to_string(),
            "A".repeat(5001),
        );
        task.complexity = 100; // Invalid Fibonacci number

        let result = task.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 4);
        assert!(errors.iter().any(|e| e.contains("ID")));
        assert!(errors.iter().any(|e| e.contains("title")));
        assert!(errors.iter().any(|e| e.contains("description")));
        assert!(errors.iter().any(|e| e.contains("Complexity")));
    }
}
