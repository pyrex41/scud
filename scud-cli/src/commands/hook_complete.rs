use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Called by Claude Code Stop hook to enforce task completion.
/// This command is internal and hidden from help.
pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    // Try to get task ID from environment or file
    let task_id = get_current_task_id(project_root.as_ref());

    match task_id {
        Some(id) => complete_task(project_root, &id)?,
        None => {
            // No task context - this is fine, just exit silently
            // (The hook fires on every session end, not just task sessions)
        }
    }

    Ok(())
}

fn complete_task(project_root: Option<PathBuf>, task_id: &str) -> Result<()> {
    let storage = Storage::new(project_root);

    // Get active tag
    let tag = match storage.get_active_group()? {
        Some(t) => t,
        None => {
            eprintln!("Hook: No active tag, skipping completion");
            return Ok(());
        }
    };

    // Load and complete the task
    let mut phase = storage.load_group(&tag)?;

    if let Some(task) = phase.get_task_mut(task_id) {
        if task.status != TaskStatus::Done {
            task.status = TaskStatus::Done;
            task.update();
            eprintln!("✓ Hook: Task {} marked complete", task_id);
        }
    } else {
        eprintln!("Hook: Task {} not found in tag {}", task_id, tag);
    }

    // Save the updated phase
    storage.update_group(&tag, &phase)?;

    // Clean up the current-task file
    let current_task_file = get_current_task_file(storage.scud_dir());
    let _ = fs::remove_file(current_task_file);

    Ok(())
}

fn get_current_task_file(scud_dir: PathBuf) -> PathBuf {
    scud_dir.join("current-task")
}

fn get_current_task_id(project_root: Option<&PathBuf>) -> Option<String> {
    // Check environment variable first (set by orchestrator)
    if let Ok(id) = env::var("SCUD_TASK_ID") {
        if !id.is_empty() {
            return Some(id);
        }
    }

    // Check .scud/current-task file (set by claim or session start)
    let storage = Storage::new(project_root.cloned());
    let current_task_file = get_current_task_file(storage.scud_dir());

    if let Ok(id) = fs::read_to_string(current_task_file) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }

    None
}
