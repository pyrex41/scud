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

/// Show recent log entries from all tasks (for discovery sharing between agents)
pub fn show_all(project_root: Option<PathBuf>, limit: usize, tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let logs_dir = storage.scud_dir().join("logs");

    if !logs_dir.exists() {
        println!("No logs directory found. Use 'scud log <task_id> <summary>' to create entries.");
        return Ok(());
    }

    // Collect all log entries with timestamps
    let mut entries: Vec<(String, String, String)> = Vec::new(); // (timestamp, task_id, content)

    // If tag is specified, get task IDs from that tag to filter
    let tag_task_ids: Option<std::collections::HashSet<String>> = if let Some(t) = tag {
        let phase = storage.load_group(t)?;
        Some(phase.tasks.iter().map(|task| task.id.clone()).collect())
    } else {
        None
    };

    // Read all .log files
    for entry in fs::read_dir(&logs_dir).context("Failed to read logs directory")? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "log") {
            let task_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Filter by tag if specified
            if let Some(ref ids) = tag_task_ids {
                if !ids.contains(&task_id) {
                    continue;
                }
            }

            let content = fs::read_to_string(&path).unwrap_or_default();

            // Parse entries from file
            let mut current_timestamp = String::new();
            let mut current_content = String::new();

            for line in content.lines() {
                if line.starts_with("--- ") && line.ends_with(" ---") {
                    // Save previous entry if exists
                    if !current_timestamp.is_empty() && !current_content.trim().is_empty() {
                        entries.push((
                            current_timestamp.clone(),
                            task_id.clone(),
                            current_content.trim().to_string(),
                        ));
                    }
                    // Start new entry
                    current_timestamp = line
                        .trim_start_matches("--- ")
                        .trim_end_matches(" ---")
                        .to_string();
                    current_content.clear();
                } else if !line.is_empty() {
                    current_content.push_str(line);
                    current_content.push('\n');
                }
            }
            // Don't forget the last entry
            if !current_timestamp.is_empty() && !current_content.trim().is_empty() {
                entries.push((current_timestamp, task_id, current_content.trim().to_string()));
            }
        }
    }

    if entries.is_empty() {
        println!("No log entries found.");
        return Ok(());
    }

    // Sort by timestamp descending (most recent first)
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    // Take only the most recent entries
    let entries: Vec<_> = entries.into_iter().take(limit).collect();

    // Print header
    println!("=== Recent Log Entries ({} shown) ===\n", entries.len());

    for (timestamp, task_id, content) in entries {
        println!("[{}] Task {}", timestamp, task_id);
        for line in content.lines() {
            println!("  {}", line);
        }
        println!();
    }

    Ok(())
}
