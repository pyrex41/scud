//! Sync task changes from Claude Tasks back to SCUD
//!
//! This command is called by the PostToolUse hook when agents use
//! TaskUpdate or TaskCreate tools. It reads `~/.claude/tasks/scud-*.json`
//! files and syncs any status changes back to SCUD's task storage.
//!
//! This ensures SCUD remains the source of truth while allowing agents
//! to make task updates via Claude Code's native task tools.

use anyhow::Result;
use std::path::PathBuf;

use crate::models::task::TaskStatus;
use crate::storage::Storage;
use crate::sync::claude_tasks::{claude_tasks_dir, ClaudeTaskList};

/// Sync task status changes from Claude Tasks back to SCUD
///
/// Called by the PostToolUse hook after TaskUpdate/TaskCreate tool calls.
/// Silently exits if not in a SCUD project or if no task files exist.
pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let project_root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
    let storage = Storage::new(Some(project_root.clone()));

    if !storage.is_initialized() {
        // Silently exit if not a SCUD project
        return Ok(());
    }

    let tasks_dir = claude_tasks_dir();
    if !tasks_dir.exists() {
        return Ok(());
    }

    // Find all scud-* task files
    let entries = match std::fs::read_dir(&tasks_dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process scud-*.json files
        let file_name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(name) if name.starts_with("scud-") => name,
            _ => continue,
        };

        let tag = match file_name.strip_prefix("scud-") {
            Some(t) => t,
            None => continue,
        };

        // Load Claude task list
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let claude_list: ClaudeTaskList = match serde_json::from_str(&content) {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Load SCUD phase
        let mut phase = match storage.load_group(tag) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let mut changed = false;

        // Sync status changes
        for claude_task in &claude_list.tasks {
            // Parse task ID (format: "tag:id")
            let task_id = claude_task
                .id
                .strip_prefix(&format!("{}:", tag))
                .unwrap_or(&claude_task.id);

            if let Some(scud_task) = phase.get_task_mut(task_id) {
                let new_status = match claude_task.status.as_str() {
                    "pending" => TaskStatus::Pending,
                    "in_progress" => TaskStatus::InProgress,
                    "completed" => TaskStatus::Done,
                    _ => continue,
                };

                // Only update if status actually changed
                if scud_task.status != new_status {
                    scud_task.set_status(new_status);
                    changed = true;
                }
            }
        }

        if changed {
            // Save the updated phase
            if let Err(e) = storage.update_group(tag, &phase) {
                eprintln!("Warning: Failed to sync changes for tag '{}': {}", tag, e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use crate::models::task::Task;
    use crate::sync::claude_tasks::sync_phase;
    use tempfile::TempDir;

    #[test]
    fn test_sync_status_change() {
        // Use a unique tag to avoid interference with other tests
        let tag = format!("sync-test-change-{}", std::process::id());
        let tmp = TempDir::new().unwrap();

        // Initialize SCUD storage
        let storage = Storage::new(Some(tmp.path().to_path_buf()));
        storage.initialize().unwrap();

        // Create a phase with a task
        let mut phase = Phase::new(tag.clone());
        let task = Task::new(
            "1".to_string(),
            "Test task".to_string(),
            "Description".to_string(),
        );
        phase.add_task(task);
        storage.update_group(&tag, &phase).unwrap();
        storage.set_active_group(&tag).unwrap();

        // Sync to Claude format
        sync_phase(&phase, &tag).unwrap();

        // Modify the Claude task file to mark task as completed
        let tasks_dir = claude_tasks_dir();
        let task_file = tasks_dir.join(format!("scud-{}.json", tag));

        let mut claude_list: ClaudeTaskList =
            serde_json::from_str(&std::fs::read_to_string(&task_file).unwrap()).unwrap();
        claude_list.tasks[0].status = "completed".to_string();
        std::fs::write(&task_file, serde_json::to_string(&claude_list).unwrap()).unwrap();

        // Run sync
        run(Some(tmp.path().to_path_buf())).unwrap();

        // Verify SCUD task was updated
        let updated_phase = storage.load_group(&tag).unwrap();
        let updated_task = updated_phase.get_task("1").unwrap();
        assert_eq!(updated_task.status, TaskStatus::Done);

        // Clean up the Claude task file
        let _ = std::fs::remove_file(&task_file);
    }

    #[test]
    fn test_sync_ignores_unchanged_status() {
        // Use a unique tag to avoid interference with other tests
        let tag = format!("sync-test-unchanged-{}", std::process::id());
        let tmp = TempDir::new().unwrap();

        // Initialize SCUD storage
        let storage = Storage::new(Some(tmp.path().to_path_buf()));
        storage.initialize().unwrap();

        // Create a phase with a task already marked done
        let mut phase = Phase::new(tag.clone());
        let mut task = Task::new(
            "1".to_string(),
            "Test task".to_string(),
            "Description".to_string(),
        );
        task.set_status(TaskStatus::Done);
        phase.add_task(task);
        storage.update_group(&tag, &phase).unwrap();
        storage.set_active_group(&tag).unwrap();

        // Verify the task was saved correctly
        let saved_phase = storage.load_group(&tag).unwrap();
        let saved_task = saved_phase.get_task("1").unwrap();
        assert_eq!(saved_task.status, TaskStatus::Done, "Task should be saved as Done");

        // Sync to Claude format
        sync_phase(&phase, &tag).unwrap();

        // Run sync (should not error even though status matches)
        run(Some(tmp.path().to_path_buf())).unwrap();

        // Verify task is still done
        let updated_phase = storage.load_group(&tag).unwrap();
        let updated_task = updated_phase.get_task("1").unwrap();
        assert_eq!(updated_task.status, TaskStatus::Done);

        // Clean up the Claude task file
        let tasks_dir = claude_tasks_dir();
        let task_file = tasks_dir.join(format!("scud-{}.json", tag));
        let _ = std::fs::remove_file(&task_file);
    }

    #[test]
    fn test_sync_silently_exits_non_scud_project() {
        let tmp = TempDir::new().unwrap();

        // Don't initialize SCUD - should exit silently
        let result = run(Some(tmp.path().to_path_buf()));
        assert!(result.is_ok());
    }
}
