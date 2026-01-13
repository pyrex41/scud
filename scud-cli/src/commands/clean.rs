use anyhow::{Context, Result};
use chrono::Local;
use colored::Colorize;
use dialoguer::Confirm;
use std::fs;
use std::path::PathBuf;

use crate::formats::{parse_scg, serialize_scg};
use crate::storage::Storage;

/// Archive directory path within .scud
fn archive_dir(storage: &Storage) -> PathBuf {
    storage.scud_dir().join("archive")
}

/// List all archived phases
fn list_archives(storage: &Storage) -> Result<Vec<(String, String)>> {
    let archive_path = archive_dir(storage);
    if !archive_path.exists() {
        return Ok(vec![]);
    }

    let mut archives = vec![];
    for entry in fs::read_dir(&archive_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "scg").unwrap_or(false) {
            let filename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            // Extract tag name from filename (format: tag_YYYYMMDD_HHMMSS)
            let parts: Vec<&str> = filename.rsplitn(3, '_').collect();
            let tag_name = if parts.len() >= 3 {
                parts[2].to_string()
            } else {
                filename.clone()
            };
            archives.push((tag_name, filename));
        }
    }

    // Sort by filename (which includes timestamp)
    archives.sort_by(|a, b| b.1.cmp(&a.1)); // Most recent first
    Ok(archives)
}

/// Archive a phase to .scud/archive/
fn archive_phase(storage: &Storage, tag: &str) -> Result<String> {
    let all_tasks = storage.load_tasks()?;
    let phase = all_tasks
        .get(tag)
        .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found", tag))?;

    let archive_path = archive_dir(storage);
    fs::create_dir_all(&archive_path).context("Failed to create archive directory")?;

    // Generate archive filename with timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let archive_filename = format!("{}_{}.scg", tag, timestamp);
    let archive_file = archive_path.join(&archive_filename);

    // Serialize and write the phase
    let content = serialize_scg(phase);
    fs::write(&archive_file, content).context("Failed to write archive file")?;

    Ok(archive_filename)
}

/// Restore a phase from archive
///
/// # Arguments
/// * `storage` - Storage instance
/// * `archive_name` - Name of the archive file (with or without .scg extension)
/// * `force` - If true, replace existing tags; if false, error on conflict
///
/// # Returns
/// The tag name that was restored
fn restore_phase(storage: &Storage, archive_name: &str, force: bool) -> Result<String> {
    let archive_path = archive_dir(storage);

    // Find the archive file
    let archive_file = if archive_name.ends_with(".scg") {
        archive_path.join(archive_name)
    } else {
        archive_path.join(format!("{}.scg", archive_name))
    };

    if !archive_file.exists() {
        anyhow::bail!("Archive '{}' not found", archive_name);
    }

    // Read and parse the archived phase
    let content = fs::read_to_string(&archive_file).context("Failed to read archive file")?;
    let phase = parse_scg(&content).context("Failed to parse archive file")?;
    let tag_name = phase.name.clone();

    // Load current tasks and check for conflicts
    let mut all_tasks = storage.load_tasks()?;
    if all_tasks.contains_key(&tag_name) {
        if force {
            // With --force, replace the existing tag
            println!(
                "{} Replacing existing tag '{}'",
                "⚠".yellow(),
                tag_name.cyan()
            );
        } else {
            anyhow::bail!(
                "Tag '{}' already exists. Use --force to replace it.",
                tag_name
            );
        }
    }

    // Add the restored phase (replaces if exists)
    all_tasks.insert(tag_name.clone(), phase);
    storage.save_tasks(&all_tasks)?;

    // Remove the archive file after successful restore
    fs::remove_file(&archive_file).context("Failed to remove archive file after restore")?;

    Ok(tag_name)
}

pub fn run(
    project_root: Option<PathBuf>,
    force: bool,
    tag: Option<&str>,
    keep: &[String],
    delete: bool,
    list: bool,
    restore: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Handle --list flag
    if list {
        let archives = list_archives(&storage)?;
        if archives.is_empty() {
            println!("{}", "No archived phases found.".yellow());
        } else {
            println!("{}", "Archived phases:".cyan().bold());
            println!();
            for (tag_name, filename) in archives {
                println!(
                    "  {} {}",
                    tag_name.green(),
                    format!("({})", filename).dimmed()
                );
            }
            println!();
            println!(
                "{}",
                "Use 'scud clean --restore <filename>' to restore".dimmed()
            );
        }
        return Ok(());
    }

    // Handle --restore flag
    if let Some(archive_name) = restore {
        let restored_tag = restore_phase(&storage, archive_name, force)?;
        println!();
        println!(
            "{} Restored tag '{}' from archive",
            "✓".green(),
            restored_tag.cyan()
        );
        println!();
        return Ok(());
    }

    let mut all_tasks = storage.load_tasks()?;

    if all_tasks.is_empty() {
        println!("{}", "No tasks to clean.".yellow());
        return Ok(());
    }

    // Determine which tags to clean
    let tags_to_clean: Vec<String> = if let Some(tag_name) = tag {
        if !all_tasks.contains_key(tag_name) {
            anyhow::bail!("Tag '{}' not found", tag_name);
        }
        if keep.contains(&tag_name.to_string()) {
            anyhow::bail!("Cannot clean tag '{}' - it's in the keep list", tag_name);
        }
        vec![tag_name.to_string()]
    } else {
        // Clean all tags except those in --keep
        all_tasks
            .keys()
            .filter(|t| !keep.contains(t))
            .cloned()
            .collect()
    };

    if tags_to_clean.is_empty() {
        println!("{}", "No tags to clean (all kept).".yellow());
        return Ok(());
    }

    // Calculate totals for confirmation message
    let total_tasks: usize = tags_to_clean
        .iter()
        .filter_map(|t| all_tasks.get(t))
        .map(|p| p.tasks.len())
        .sum();

    // Build confirmation message based on action type
    let action_word = if delete { "Delete" } else { "Archive" };
    let (confirm_msg, warning_msg) = if tags_to_clean.len() == 1 {
        let tag_name = &tags_to_clean[0];
        (
            format!(
                "{} {} tasks from tag '{}'?",
                action_word,
                total_tasks.to_string().cyan(),
                tag_name.cyan()
            ),
            if delete {
                "This action cannot be undone!".to_string()
            } else {
                "Tasks will be archived to .scud/archive/".to_string()
            },
        )
    } else {
        let kept_msg = if !keep.is_empty() {
            format!(" (keeping: {})", keep.join(", ").green())
        } else {
            String::new()
        };
        (
            format!(
                "{} {} tasks across {} tags{}?",
                action_word,
                total_tasks.to_string().cyan(),
                tags_to_clean.len().to_string().cyan(),
                kept_msg
            ),
            if delete {
                "This action cannot be undone!".to_string()
            } else {
                "Tasks will be archived to .scud/archive/".to_string()
            },
        )
    };

    // Confirm unless --force
    if !force {
        println!();
        if delete {
            println!("{}", format!("⚠ WARNING: {}", warning_msg).red().bold());
        } else {
            println!("{}", format!("ℹ {}", warning_msg).blue());
        }
        println!();

        let confirmed = Confirm::new()
            .with_prompt(confirm_msg)
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{}", "Cancelled.".yellow());
            return Ok(());
        }
    }

    // Perform the clean
    let mut archived_files = vec![];
    let mut cleaned_tags = vec![];

    for tag_name in &tags_to_clean {
        // Archive first (unless --delete)
        if !delete {
            match archive_phase(&storage, tag_name) {
                Ok(filename) => archived_files.push((tag_name.clone(), filename)),
                Err(e) => {
                    eprintln!("{} Failed to archive '{}': {}", "✗".red(), tag_name, e);
                    continue;
                }
            }
        }

        // Remove the tag from active tasks
        all_tasks.remove(tag_name);
        cleaned_tags.push(tag_name.clone());

        // Clear active tag if it was the one we cleaned
        if let Ok(Some(active)) = storage.get_active_group() {
            if &active == tag_name {
                let _ = storage.clear_active_group();
            }
        }
    }

    storage.save_tasks(&all_tasks)?;

    // Print results
    println!();
    if delete {
        println!(
            "{} Deleted {} tag(s): {}",
            "✓".green(),
            cleaned_tags.len(),
            cleaned_tags.join(", ").cyan()
        );
    } else {
        println!("{} Archived {} tag(s):", "✓".green(), archived_files.len());
        for (tag_name, filename) in &archived_files {
            println!("  {} → {}", tag_name.cyan(), filename.dimmed());
        }
        println!();
        println!(
            "{}",
            "Use 'scud clean --list' to see archives, '--restore <name>' to restore".dimmed()
        );
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Phase, Task};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn setup_test_storage() -> (TempDir, Storage) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
        storage.initialize().unwrap();
        (temp_dir, storage)
    }

    #[test]
    fn test_archive_single_tag() {
        let (_temp, storage) = setup_test_storage();

        // Create test tasks with one tag
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Test Task".to_string(),
            "Test description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive single tag
        let path = storage.archive_phase("v1", &phases).unwrap();

        // Verify archive was created
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("v1"));
        assert!(path.extension().unwrap() == "scg");

        // Verify archive contains the correct data
        let loaded = storage.load_archive(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("v1"));
        assert_eq!(loaded.get("v1").unwrap().tasks.len(), 1);
    }

    #[test]
    fn test_archive_all() {
        let (_temp, storage) = setup_test_storage();

        // Create multiple tags
        let mut phases = HashMap::new();
        let mut phase1 = Phase::new("v1".to_string());
        phase1.add_task(Task::new(
            "task-1".to_string(),
            "Task 1".to_string(),
            "Desc 1".to_string(),
        ));
        phases.insert("v1".to_string(), phase1);

        let mut phase2 = Phase::new("v2".to_string());
        phase2.add_task(Task::new(
            "task-2".to_string(),
            "Task 2".to_string(),
            "Desc 2".to_string(),
        ));
        phases.insert("v2".to_string(), phase2);
        storage.save_tasks(&phases).unwrap();

        // Archive all
        let path = storage.archive_all(&phases).unwrap();

        // Verify archive was created with "all" in filename
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("all"));

        // Verify archive contains all phases
        let loaded = storage.load_archive(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains_key("v1"));
        assert!(loaded.contains_key("v2"));
    }

    #[test]
    fn test_list_archives() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive a phase
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Test Task".to_string(),
            "Test description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it
        storage.archive_phase("v1", &phases).unwrap();

        // List archives
        let archives = storage.list_archives().unwrap();

        // Verify archive appears in list
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].tag, Some("v1".to_string()));
        assert_eq!(archives[0].task_count, 1);
        assert!(archives[0].filename.contains("v1"));
    }

    #[test]
    fn test_restore_archive() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive a phase
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Test Task".to_string(),
            "Test description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive
        let archive_path = storage.archive_phase("v1", &phases).unwrap();
        let archive_name = archive_path.file_name().unwrap().to_str().unwrap();

        // Clear current tasks
        storage.save_tasks(&HashMap::new()).unwrap();
        let empty_check = storage.load_tasks().unwrap();
        assert!(empty_check.is_empty());

        // Restore
        let restored = storage.restore_archive(archive_name, false).unwrap();
        assert_eq!(restored, vec!["v1".to_string()]);

        // Verify restored data
        let current = storage.load_tasks().unwrap();
        assert!(current.contains_key("v1"));
        assert_eq!(current.get("v1").unwrap().tasks.len(), 1);
    }

    // Edge case tests

    #[test]
    fn test_archive_duplicate_filename_uses_counter() {
        let (_temp, storage) = setup_test_storage();

        // Create test phase
        let mut phases = HashMap::new();
        let phase = Phase::new("v1".to_string());
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive the same tag multiple times on the same day
        let path1 = storage.archive_phase("v1", &phases).unwrap();
        let path2 = storage.archive_phase("v1", &phases).unwrap();
        let path3 = storage.archive_phase("v1", &phases).unwrap();

        // All paths should be unique
        assert!(path1.exists());
        assert!(path2.exists());
        assert!(path3.exists());
        assert_ne!(path1, path2);
        assert_ne!(path2, path3);

        // Second and third should have counter suffix
        let filename2 = path2.file_name().unwrap().to_string_lossy();
        let filename3 = path3.file_name().unwrap().to_string_lossy();
        assert!(filename2.contains("_1.scg") || filename2.contains("_v1_1.scg"));
        assert!(filename3.contains("_2.scg") || filename3.contains("_v1_2.scg"));
    }

    #[test]
    fn test_archive_nonexistent_tag_fails() {
        let (_temp, storage) = setup_test_storage();

        // Empty phases
        let phases = HashMap::new();

        // Try to archive a non-existent tag
        let result = storage.archive_phase("nonexistent", &phases);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_restore_nonexistent_archive_fails() {
        let (_temp, storage) = setup_test_storage();

        // Try to restore an archive that doesn't exist
        let result = storage.restore_archive("nonexistent", false);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_list_archives_empty() {
        let (_temp, storage) = setup_test_storage();

        // No archives created yet
        let archives = storage.list_archives().unwrap();
        assert!(archives.is_empty());
    }

    #[test]
    fn test_restore_phase_without_force_errors_on_conflict() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive a phase
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Original Task".to_string(),
            "Original description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it using the local archive_phase function
        let archive_filename = archive_phase(&storage, "v1").unwrap();

        // Try to restore without force - should error because v1 still exists
        let result = restore_phase(&storage, &archive_filename, false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("already exists"));
        assert!(err.contains("--force"));
    }

    #[test]
    fn test_restore_phase_with_force_replaces_existing() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive a phase with 1 task
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Original Task".to_string(),
            "Original description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it
        let archive_filename = archive_phase(&storage, "v1").unwrap();

        // Modify current tasks - add a second task
        let mut current = storage.load_tasks().unwrap();
        current.get_mut("v1").unwrap().add_task(Task::new(
            "task-2".to_string(),
            "New Task".to_string(),
            "New description".to_string(),
        ));
        storage.save_tasks(&current).unwrap();

        // Verify we have 2 tasks now
        let check = storage.load_tasks().unwrap();
        assert_eq!(check.get("v1").unwrap().tasks.len(), 2);

        // Restore with force - should replace
        let result = restore_phase(&storage, &archive_filename, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "v1");

        // Verify we're back to 1 task (the archived version)
        let final_tasks = storage.load_tasks().unwrap();
        assert_eq!(final_tasks.get("v1").unwrap().tasks.len(), 1);
        assert_eq!(
            final_tasks.get("v1").unwrap().tasks[0].title,
            "Original Task"
        );
    }

    #[test]
    fn test_restore_phase_to_empty_tasks() {
        let (_temp, storage) = setup_test_storage();

        // Create and archive a phase
        let mut phases = HashMap::new();
        let mut phase = Phase::new("v1".to_string());
        phase.add_task(Task::new(
            "task-1".to_string(),
            "Test Task".to_string(),
            "Test description".to_string(),
        ));
        phases.insert("v1".to_string(), phase);
        storage.save_tasks(&phases).unwrap();

        // Archive it
        let archive_filename = archive_phase(&storage, "v1").unwrap();

        // Clear current tasks
        storage.save_tasks(&HashMap::new()).unwrap();

        // Restore (no force needed since tag doesn't exist)
        let result = restore_phase(&storage, &archive_filename, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "v1");

        // Verify restored
        let current = storage.load_tasks().unwrap();
        assert!(current.contains_key("v1"));
        assert_eq!(current.get("v1").unwrap().tasks.len(), 1);
    }
}
