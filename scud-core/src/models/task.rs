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
    Expanded, // Task has been broken into subtasks
    Failed,   // Task failed validation (build/test/lint)
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
            TaskStatus::Expanded => "expanded",
            TaskStatus::Failed => "failed",
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
            "expanded" => Some(TaskStatus::Expanded),
            "failed" => Some(TaskStatus::Failed),
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
            "expanded",
            "failed",
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    // Parent-child relationship for expanded tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_strategy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    // Assignment tracking (informational only, no locking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,

    /// Agent type for model routing (e.g., "builder", "reviewer", "planner")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

impl Task {
    // Validation constants
    const MAX_TITLE_LENGTH: usize = 200;
    const MAX_DESCRIPTION_LENGTH: usize = 5000;
    const VALID_FIBONACCI_NUMBERS: &'static [u32] = &[0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89];

    /// ID separator for namespaced IDs (epic:local_id)
    pub const ID_SEPARATOR: char = ':';

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
            parent_id: None,
            subtasks: Vec::new(),
            details: None,
            test_strategy: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            assigned_to: None,
            agent_type: None,
        }
    }

    /// Parse a task ID into (epic_tag, local_id) parts
    /// e.g., "phase1:10.1" -> Some(("phase1", "10.1"))
    /// e.g., "10.1" -> None (legacy format)
    pub fn parse_id(id: &str) -> Option<(&str, &str)> {
        id.split_once(Self::ID_SEPARATOR)
    }

    /// Create a namespaced task ID
    pub fn make_id(epic_tag: &str, local_id: &str) -> String {
        format!("{}{}{}", epic_tag, Self::ID_SEPARATOR, local_id)
    }

    /// Get the local ID part (without epic prefix)
    pub fn local_id(&self) -> &str {
        Self::parse_id(&self.id)
            .map(|(_, local)| local)
            .unwrap_or(&self.id)
    }

    /// Get the epic tag from a namespaced ID
    pub fn epic_tag(&self) -> Option<&str> {
        Self::parse_id(&self.id).map(|(tag, _)| tag)
    }

    /// Check if this is a subtask (has parent)
    pub fn is_subtask(&self) -> bool {
        self.parent_id.is_some()
    }

    /// Check if this task has been expanded into subtasks
    pub fn is_expanded(&self) -> bool {
        self.status == TaskStatus::Expanded || !self.subtasks.is_empty()
    }

    /// Validate task ID - must contain only alphanumeric characters, hyphens, underscores,
    /// colons (for namespacing), and dots (for subtask IDs)
    pub fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("Task ID cannot be empty".to_string());
        }

        if id.len() > 100 {
            return Err("Task ID too long (max 100 characters)".to_string());
        }

        // Allow alphanumeric, hyphen, underscore, colon (namespacing), and dot (subtask IDs)
        let valid_chars = id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.');

        if !valid_chars {
            return Err(
                "Task ID can only contain alphanumeric characters, hyphens, underscores, colons, and dots"
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

    /// Get effective dependencies including inherited parent dependencies
    /// Subtasks inherit their parent's dependencies (including cross-tag deps)
    pub fn get_effective_dependencies(&self, all_tasks: &[&Task]) -> Vec<String> {
        let mut deps = self.dependencies.clone();

        // If this is a subtask, inherit parent's dependencies
        if let Some(ref parent_id) = self.parent_id {
            if let Some(parent) = all_tasks.iter().find(|t| &t.id == parent_id) {
                // Recursively get parent's effective dependencies
                let parent_deps = parent.get_effective_dependencies(all_tasks);
                deps.extend(parent_deps);
            }
        }

        // Deduplicate
        deps.sort();
        deps.dedup();
        deps
    }

    /// Check if all dependencies are met, searching across provided task references
    /// Supports cross-tag dependencies when passed tasks from all phases
    /// Subtasks inherit parent dependencies via get_effective_dependencies
    pub fn has_dependencies_met_refs(&self, all_tasks: &[&Task]) -> bool {
        self.get_effective_dependencies(all_tasks)
            .iter()
            .all(|dep_id| {
                all_tasks
                    .iter()
                    .find(|t| &t.id == dep_id)
                    .map(|t| t.status == TaskStatus::Done)
                    .unwrap_or(false)
            })
    }

    /// Returns whether this task should be expanded into subtasks
    /// Only tasks with complexity >= 5 benefit from expansion
    /// Subtasks and already-expanded tasks don't need expansion
    pub fn needs_expansion(&self) -> bool {
        self.complexity >= 5 && !self.is_expanded() && !self.is_subtask()
    }

    /// Returns the recommended number of subtasks based on complexity
    /// Complexity 0-3: 0 subtasks (trivial/simple, no expansion needed)
    /// Complexity 5-8: 2 broad, multi-step subtasks
    /// Complexity 13+: 3 broad, multi-step subtasks
    pub fn recommended_subtasks(&self) -> usize {
        Self::recommended_subtasks_for_complexity(self.complexity)
    }

    /// Static version for use when we only have complexity value
    pub fn recommended_subtasks_for_complexity(complexity: u32) -> usize {
        match complexity {
            0..=3 => 0, // Trivial/simple tasks: no expansion needed
            5 => 2,     // Moderate tasks: 2 broad subtasks
            8 => 2,     // Complex tasks: 2 broad subtasks
            13 => 3,    // Very complex: 3 broad subtasks
            _ => 3,     // Extremely complex (21+): 3 broad subtasks max
        }
    }

    // Assignment methods (informational only, no locking)
    pub fn assign(&mut self, assignee: &str) {
        self.assigned_to = Some(assignee.to_string());
        self.update();
    }

    pub fn is_assigned_to(&self, assignee: &str) -> bool {
        self.assigned_to
            .as_ref()
            .map(|s| s == assignee)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTask {
    pub id: String,
    pub subject: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
    pub status: ClaudeTaskStatus,
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaudeTaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl From<TaskStatus> for ClaudeTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Pending => ClaudeTaskStatus::Pending,
            TaskStatus::InProgress => ClaudeTaskStatus::InProgress,
            TaskStatus::Done | TaskStatus::Review => ClaudeTaskStatus::Completed,
            _ => ClaudeTaskStatus::Pending, // Map others conservatively
        }
    }
}

impl Task {
    /// Convert SCUD DAG to flat Claude Tasks list
    /// Flatten subtasks recursively, deps → blocked_by
    pub fn to_claude_tasks(&self, all_tasks: &[Task]) -> Vec<ClaudeTask> {
        let mut tasks = Vec::new();
        self.flatten_to_claude(&mut tasks, all_tasks);
        tasks
    }

    fn flatten_to_claude(&self, tasks: &mut Vec<ClaudeTask>, all_tasks: &[Task]) {
        let claude_status = ClaudeTaskStatus::from(self.status.clone());
        let claude_task = ClaudeTask {
            id: self.local_id().to_string(),
            subject: self.title.clone(),
            description: self.description.clone(),
            active_form: if self.status == TaskStatus::InProgress {
                Some(self.title.clone()) // Or details?
            } else {
                None
            },
            status: claude_status,
            blocks: self.subtasks.clone(),
            blocked_by: self.dependencies.clone(),
        };
        tasks.push(claude_task);

        // Recurse subtasks
        for sub_id in &self.subtasks {
            if let Some(sub) = all_tasks.iter().find(|t| &t.id == sub_id) {
                sub.flatten_to_claude(tasks, all_tasks);
            }
        }
    }

    /// Build SCUD Task from ClaudeTask (single, no DAG restore)
    pub fn from_claude_task(ct: &ClaudeTask) -> Self {
        let status = match ct.status {
            ClaudeTaskStatus::Pending => TaskStatus::Pending,
            ClaudeTaskStatus::InProgress => TaskStatus::InProgress,
            ClaudeTaskStatus::Completed => TaskStatus::Done,
        };
        Task {
            id: ct.id.clone(),
            title: ct.subject.clone(),
            description: ct.description.clone(),
            status,
            dependencies: ct.blocked_by.clone(),
            subtasks: ct.blocks.clone(),
            ..Default::default()
        }
    }

    /// Import full list to Vec<Task> (topological order?)
    pub fn from_claude_tasks(cts: &[ClaudeTask]) -> Vec<Task> {
        cts.iter().map(Task::from_claude_task).collect()
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
    fn test_needs_expansion() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());

        // Complexity < 5 should not need expansion
        task.complexity = 1;
        assert!(!task.needs_expansion());

        task.complexity = 3;
        assert!(!task.needs_expansion());

        // Complexity >= 5 should need expansion
        task.complexity = 5;
        assert!(task.needs_expansion());

        task.complexity = 8;
        assert!(task.needs_expansion());

        // Already expanded tasks should not need expansion
        task.status = TaskStatus::Expanded;
        assert!(!task.needs_expansion());

        // Reset status and test subtask case
        task.status = TaskStatus::Pending;
        task.parent_id = Some("parent:1".to_string());
        assert!(!task.needs_expansion()); // Subtasks don't need expansion
    }

    #[test]
    fn test_validate_id_success() {
        assert!(Task::validate_id("TASK-123").is_ok());
        assert!(Task::validate_id("task_456").is_ok());
        assert!(Task::validate_id("phase1:10").is_ok());
        assert!(Task::validate_id("phase1:10.1").is_ok());
    }

    #[test]
    fn test_validate_id_empty() {
        let result = Task::validate_id("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Task ID cannot be empty");
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
    }

    #[test]
    fn test_validate_complexity_invalid() {
        assert!(Task::validate_complexity(4).is_err());
        assert!(Task::validate_complexity(6).is_err());
        assert!(Task::validate_complexity(7).is_err());
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
    fn test_priority_default() {
        let default_priority = Priority::default();
        assert_eq!(default_priority, Priority::Medium);
    }

    #[test]
    fn test_status_all() {
        let all_statuses = TaskStatus::all();
        assert_eq!(all_statuses.len(), 9);
        assert!(all_statuses.contains(&"pending"));
        assert!(all_statuses.contains(&"in-progress"));
        assert!(all_statuses.contains(&"done"));
    }
}
