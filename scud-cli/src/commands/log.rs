use anyhow::{Context, Result};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::storage::Storage;

/// Write a summary log entry for a task.
/// Logs are stored in .scud/logs/<task-id>.log
/// Each entry is timestamped and appended to the log file.
pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    summary: &str,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Get active tag if not provided
    let active_tag = match tag {
        Some(t) => t.to_string(),
        None => storage
            .get_active_group()?
            .ok_or_else(|| anyhow::anyhow!("No active tag. Use --tag or run: scud tags <tag>"))?,
    };

    // Verify task exists
    let phase = storage.load_group(&active_tag)?;
    if phase.get_task(task_id).is_none() {
        anyhow::bail!("Task '{}' not found in tag '{}'", task_id, active_tag);
    }

    // Create logs directory if it doesn't exist
    let logs_dir = storage.scud_dir().join("logs");
    fs::create_dir_all(&logs_dir).context("Failed to create logs directory")?;

    // Append to log file
    let log_file = logs_dir.join(format!("{}.log", task_id));
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .context("Failed to open log file")?;

    writeln!(file, "--- {} ---", timestamp)?;
    writeln!(file, "{}", summary.trim())?;
    writeln!(file)?;

    println!("✓ Log entry added to {}", log_file.display());
    Ok(())
}

/// Read the log file for a task
pub fn show(project_root: Option<PathBuf>, task_id: &str) -> Result<()> {
    let storage = Storage::new(project_root);

    let logs_dir = storage.scud_dir().join("logs");
    let log_file = logs_dir.join(format!("{}.log", task_id));

    if !log_file.exists() {
        println!("No log entries for task '{}'", task_id);
        return Ok(());
    }

    let content = fs::read_to_string(&log_file).context("Failed to read log file")?;
    print!("{}", content);
    Ok(())
}
