//! Claude Code Tasks format conversion and sync
//!
//! Converts SCUD tasks to Claude Code's native Tasks JSON format,
//! enabling agents to see tasks via the `TaskList` tool.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};

/// Claude Code task format
///
/// This matches the JSON structure that Claude Code's Task tools expect.
/// See: `~/.claude/tasks/<list-id>.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTask {
    /// Task identifier (format: "tag:id" for SCUD tasks)
    pub id: String,

    /// Task title (maps to SCUD's `title`)
    pub subject: String,

    /// Task description
    #[serde(default)]
    pub description: String,

    /// Task status: "pending", "in_progress", or "completed"
    pub status: String,

    /// Task IDs that must complete before this one can start
    #[serde(default, rename = "blockedBy")]
    pub blocked_by: Vec<String>,

    /// Task IDs that are waiting for this task to complete
    #[serde(default)]
    pub blocks: Vec<String>,

    /// Agent/session currently working on this task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Additional metadata (SCUD-specific fields)
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Claude Code task list format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTaskList {
    /// List of tasks
    pub tasks: Vec<ClaudeTask>,
}

impl ClaudeTask {
    /// Convert a SCUD task to Claude Code's task format
    ///
    /// # Arguments
    /// * `task` - The SCUD task to convert
    /// * `tag` - The phase tag (used to namespace task IDs)
    ///
    /// # Returns
    /// A `ClaudeTask` with SCUD fields mapped to Claude format
    pub fn from_scud_task(task: &Task, tag: &str) -> Self {
        let status = match task.status {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Done => "completed",
            // Map other statuses with metadata to track original
            TaskStatus::Blocked | TaskStatus::Deferred => "pending",
            TaskStatus::Failed | TaskStatus::Cancelled => "completed",
            TaskStatus::Review => "in_progress",
            TaskStatus::Expanded => "completed",
        };

        ClaudeTask {
            id: format!("{}:{}", tag, task.id),
            subject: task.title.clone(),
            description: task.description.clone(),
            status: status.to_string(),
            blocked_by: task
                .dependencies
                .iter()
                .map(|d: &String| {
                    // Handle cross-tag dependencies (already namespaced)
                    if d.contains(':') {
                        d.clone()
                    } else {
                        format!("{}:{}", tag, d)
                    }
                })
                .collect(),
            blocks: vec![], // Filled in by sync_phase()
            owner: task.assigned_to.clone(),
            metadata: serde_json::json!({
                "scud_tag": tag,
                "scud_status": format!("{:?}", task.status),
                "complexity": task.complexity,
                "priority": format!("{:?}", task.priority),
                "agent_type": task.agent_type,
            }),
        }
    }
}

/// Get the Claude Code tasks directory
///
/// Returns `~/.claude/tasks/`
pub fn claude_tasks_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("tasks")
}

/// Generate a task list ID for a SCUD tag
///
/// The task list ID is used for the `CLAUDE_CODE_TASK_LIST_ID` environment
/// variable and as the filename for the tasks JSON file.
///
/// # Arguments
/// * `tag` - The SCUD phase tag
///
/// # Returns
/// A task list ID in the format "scud-{tag}"
pub fn task_list_id(tag: &str) -> String {
    format!("scud-{}", tag)
}

/// Sync a SCUD phase to Claude Code's Tasks format
///
/// Creates or updates `~/.claude/tasks/scud-{tag}.json` with tasks
/// from the given phase.
///
/// # Arguments
/// * `phase` - The SCUD phase containing tasks to sync
/// * `tag` - The phase tag
///
/// # Returns
/// The path to the created/updated task file
///
/// # Example
///
/// ```no_run
/// use scud::sync::claude_tasks;
/// use scud::models::phase::Phase;
///
/// let phase = Phase::new("auth".to_string());
/// let task_file = claude_tasks::sync_phase(&phase, "auth").unwrap();
/// // Creates ~/.claude/tasks/scud-auth.json
/// ```
pub fn sync_phase(phase: &Phase, tag: &str) -> Result<PathBuf> {
    let tasks_dir = claude_tasks_dir();
    std::fs::create_dir_all(&tasks_dir)?;

    let list_id = task_list_id(tag);
    let task_file = tasks_dir.join(format!("{}.json", list_id));

    // Build dependency reverse map for "blocks" field
    let mut blocks_map: HashMap<String, Vec<String>> = HashMap::new();

    for task in phase.tasks.iter() {
        let task_full_id = format!("{}:{}", tag, task.id);
        for dep in task.dependencies.iter() {
            // Handle cross-tag dependencies
            let dep_full_id: String = if dep.contains(':') {
                dep.clone()
            } else {
                format!("{}:{}", tag, dep)
            };
            blocks_map
                .entry(dep_full_id)
                .or_default()
                .push(task_full_id.clone());
        }
    }

    // Convert tasks
    let claude_tasks: Vec<ClaudeTask> = phase
        .tasks
        .iter()
        .filter(|t: &&Task| !t.is_expanded()) // Skip expanded parent tasks
        .map(|t: &Task| {
            let mut ct = ClaudeTask::from_scud_task(t, tag);
            let full_id = format!("{}:{}", tag, t.id);
            ct.blocks = blocks_map.get(&full_id).cloned().unwrap_or_default();
            ct
        })
        .collect();

    let task_list = ClaudeTaskList { tasks: claude_tasks };
    let json = serde_json::to_string_pretty(&task_list)?;
    std::fs::write(&task_file, json)?;

    Ok(task_file)
}

/// Sync multiple phases (for --all-tags mode)
///
/// # Arguments
/// * `phases` - Map of tag names to phases
///
/// # Returns
/// A vector of paths to created/updated task files
pub fn sync_phases(phases: &HashMap<String, Phase>) -> Result<Vec<PathBuf>> {
    phases
        .iter()
        .map(|(tag, phase)| sync_phase(phase, tag))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::Priority;

    #[test]
    fn test_task_list_id() {
        assert_eq!(task_list_id("auth"), "scud-auth");
        assert_eq!(task_list_id("my-feature"), "scud-my-feature");
    }

    #[test]
    fn test_claude_task_from_scud_task() {
        let mut task = Task::new(
            "1".to_string(),
            "Implement login".to_string(),
            "Add login functionality".to_string(),
        );
        task.complexity = 5;
        task.priority = Priority::High;
        task.dependencies = vec!["setup".to_string()];

        let claude_task = ClaudeTask::from_scud_task(&task, "auth");

        assert_eq!(claude_task.id, "auth:1");
        assert_eq!(claude_task.subject, "Implement login");
        assert_eq!(claude_task.status, "pending");
        assert_eq!(claude_task.blocked_by, vec!["auth:setup"]);
    }

    #[test]
    fn test_status_mapping() {
        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());

        // Pending -> pending
        task.status = TaskStatus::Pending;
        assert_eq!(
            ClaudeTask::from_scud_task(&task, "t").status,
            "pending"
        );

        // InProgress -> in_progress
        task.status = TaskStatus::InProgress;
        assert_eq!(
            ClaudeTask::from_scud_task(&task, "t").status,
            "in_progress"
        );

        // Done -> completed
        task.status = TaskStatus::Done;
        assert_eq!(
            ClaudeTask::from_scud_task(&task, "t").status,
            "completed"
        );

        // Review -> in_progress
        task.status = TaskStatus::Review;
        assert_eq!(
            ClaudeTask::from_scud_task(&task, "t").status,
            "in_progress"
        );

        // Failed -> completed (with metadata flag)
        task.status = TaskStatus::Failed;
        assert_eq!(
            ClaudeTask::from_scud_task(&task, "t").status,
            "completed"
        );
    }

    #[test]
    fn test_cross_tag_dependencies() {
        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.dependencies = vec!["other:setup".to_string(), "local".to_string()];

        let claude_task = ClaudeTask::from_scud_task(&task, "auth");

        // Cross-tag deps keep their prefix, local deps get the tag added
        assert!(claude_task.blocked_by.contains(&"other:setup".to_string()));
        assert!(claude_task.blocked_by.contains(&"auth:local".to_string()));
    }

    #[test]
    fn test_sync_phase() {
        use tempfile::TempDir;

        // Create a temp dir to use as home
        let tmp = TempDir::new().unwrap();
        let original_home = std::env::var("HOME").ok();

        // This test would need HOME override which is tricky
        // For now, just test the conversion logic
        let mut phase = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "First".to_string(), "First task".to_string());
        let mut task2 = Task::new(
            "2".to_string(),
            "Second".to_string(),
            "Second task".to_string(),
        );
        task2.dependencies = vec!["1".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);

        // Test the conversion without actually writing
        let claude_tasks: Vec<ClaudeTask> = phase
            .tasks
            .iter()
            .map(|t| ClaudeTask::from_scud_task(t, "test"))
            .collect();

        assert_eq!(claude_tasks.len(), 2);
        assert_eq!(claude_tasks[0].id, "test:1");
        assert_eq!(claude_tasks[1].id, "test:2");
        assert_eq!(claude_tasks[1].blocked_by, vec!["test:1"]);
    }
}
