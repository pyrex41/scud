//! Salvo worktree management for parallel tag execution.
//!
//! When `scud swarm --tag <tag>` is invoked, SCUD automatically provisions
//! a git worktree for that tag, generates a filtered task file, runs the
//! swarm in isolation, and syncs results back.
//!
//! Convention path: `../<project-name>.salvo.<tag>/`

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::db::Database;
use crate::formats::serialize_scg;
use crate::storage::Storage;

/// Resolve or create the worktree for a tag.
/// Returns the worktree path (which becomes the swarm's working directory).
pub fn ensure_worktree(
    project_root: &Path,
    tag: &str,
    custom_path: Option<&Path>,
) -> Result<PathBuf> {
    let db = Database::new(project_root);
    db.initialize()?;

    // Check if worktree already exists in database
    let existing = {
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        conn.query_row(
            "SELECT worktree_path FROM salvo_worktrees WHERE tag = ?",
            [tag],
            |row| row.get::<_, String>(0),
        )
        .ok()
    };

    if let Some(existing_path) = existing {
        let wt_path = PathBuf::from(&existing_path);
        if wt_path.exists() {
            // Refresh the filtered task file with latest state
            refresh_filtered_tasks(project_root, &wt_path, tag)?;
            // Sync agent and spawn definitions
            sync_scud_subdirs(project_root, &wt_path)?;
            println!(
                "Using existing salvo worktree at {}",
                wt_path.display()
            );
            return Ok(wt_path);
        }
        // Path recorded but directory gone - clean up stale record and recreate
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        conn.execute("DELETE FROM salvo_worktrees WHERE tag = ?", [tag])?;
    }

    // Determine worktree path
    let worktree_path = if let Some(p) = custom_path {
        p.to_path_buf()
    } else {
        default_worktree_path(project_root, tag)
    };

    create_worktree(project_root, tag, &worktree_path)?;
    Ok(worktree_path)
}

/// Convention: ../<project-name>.salvo.<tag>/
fn default_worktree_path(project_root: &Path, tag: &str) -> PathBuf {
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let parent = project_root.parent().unwrap_or(project_root);
    parent.join(format!("{}.salvo.{}", project_name, tag))
}

/// Create a new worktree for a tag
fn create_worktree(project_root: &Path, tag: &str, worktree_path: &Path) -> Result<()> {
    let storage = Storage::new(Some(project_root.to_path_buf()));

    // Verify tag exists
    let phases = storage.load_tasks()?;
    if !phases.contains_key(tag) {
        bail!(
            "Tag '{}' not found. Available tags: {:?}",
            tag,
            phases.keys().collect::<Vec<_>>()
        );
    }

    // Create git worktree with branch salvo/<tag>
    let branch_name = format!("salvo/{}", tag);
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(worktree_path)
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        // Branch may already exist, try attaching to it
        let output = Command::new("git")
            .args(["worktree", "add"])
            .arg(worktree_path)
            .arg(&branch_name)
            .current_dir(project_root)
            .output()?;

        if !output.status.success() {
            bail!(
                "Failed to create worktree: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Bootstrap .scud in worktree
    let worktree_scud = worktree_path.join(".scud");
    std::fs::create_dir_all(worktree_scud.join("tasks"))?;
    std::fs::create_dir_all(worktree_scud.join("swarm"))?;

    // Generate filtered task file
    generate_filtered_tasks(project_root, worktree_path, tag)?;

    // Set active tag
    std::fs::write(worktree_scud.join("active-tag"), tag)?;

    // Copy config
    let main_config = project_root.join(".scud").join("config.toml");
    if main_config.exists() {
        std::fs::copy(&main_config, worktree_scud.join("config.toml"))?;
    }

    // Copy guidance files if they exist
    let main_guidance = project_root.join(".scud").join("guidance");
    if main_guidance.exists() {
        let wt_guidance = worktree_scud.join("guidance");
        std::fs::create_dir_all(&wt_guidance)?;
        for entry in std::fs::read_dir(&main_guidance)? {
            let entry = entry?;
            if entry.path().is_file() {
                std::fs::copy(entry.path(), wt_guidance.join(entry.file_name()))?;
            }
        }
    }

    // Copy agent definitions if they exist
    let main_agents = project_root.join(".scud").join("agents");
    if main_agents.exists() {
        let wt_agents = worktree_scud.join("agents");
        std::fs::create_dir_all(&wt_agents)?;
        for entry in std::fs::read_dir(&main_agents)? {
            let entry = entry?;
            if entry.path().is_file() {
                std::fs::copy(entry.path(), wt_agents.join(entry.file_name()))?;
            }
        }
    }

    // Copy spawn agent definitions if they exist
    let main_spawn = project_root.join(".scud").join("spawn");
    if main_spawn.exists() {
        let wt_spawn = worktree_scud.join("spawn");
        std::fs::create_dir_all(&wt_spawn)?;
        for entry in std::fs::read_dir(&main_spawn)? {
            let entry = entry?;
            if entry.path().is_file() {
                std::fs::copy(entry.path(), wt_spawn.join(entry.file_name()))?;
            }
        }
    }

    // Record in database (use main project's database, not worktree's)
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO salvo_worktrees
         (tag, worktree_path, branch_name, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        [
            tag,
            worktree_path.to_str().unwrap_or(""),
            &branch_name,
        ],
    )?;

    println!(
        "Created salvo worktree for '{}' at {}",
        tag,
        worktree_path.display()
    );
    println!("Branch: {}", branch_name);

    Ok(())
}

/// Generate filtered task file: full detail for target tag, collapsed stubs for others
fn generate_filtered_tasks(
    project_root: &Path,
    worktree_path: &Path,
    target_tag: &str,
) -> Result<()> {
    let storage = Storage::new(Some(project_root.to_path_buf()));
    let phases = storage.load_tasks()?;

    let worktree_tasks = worktree_path
        .join(".scud")
        .join("tasks")
        .join("tasks.scg");
    let mut output = String::new();

    // Target phase gets full serialization
    if let Some(phase) = phases.get(target_tag) {
        output.push_str(&serialize_scg(phase));
    }

    // Other phases shown as collapsed stubs
    for (tag, phase) in &phases {
        if tag != target_tag {
            if !output.is_empty() {
                output.push_str("\n---\n\n");
            }
            output.push_str("# SCUD Graph v1\n");
            output.push_str(&format!("# Phase: {}\n", tag));
            output.push_str(&format!(
                "# [Collapsed - {} tasks, work in main branch]\n\n",
                phase.tasks.len()
            ));
            output.push_str(&format!("@meta {{\n  name {}\n}}\n", phase.name));
            output.push_str("\n@nodes\n");
            output.push_str("# Tasks hidden. Run `scud salvo sync` to merge changes.\n");
        }
    }

    std::fs::write(&worktree_tasks, output)?;
    Ok(())
}

/// Sync .scud subdirectories (agents, spawn) from main to worktree
fn sync_scud_subdirs(project_root: &Path, worktree_path: &Path) -> Result<()> {
    for dir_name in &["agents", "spawn"] {
        let main_dir = project_root.join(".scud").join(dir_name);
        if main_dir.exists() {
            let wt_dir = worktree_path.join(".scud").join(dir_name);
            std::fs::create_dir_all(&wt_dir)?;
            for entry in std::fs::read_dir(&main_dir)? {
                let entry = entry?;
                if entry.path().is_file() {
                    std::fs::copy(entry.path(), wt_dir.join(entry.file_name()))?;
                }
            }
        }
    }
    Ok(())
}

/// Refresh filtered tasks (update worktree with latest from main)
fn refresh_filtered_tasks(
    project_root: &Path,
    worktree_path: &Path,
    tag: &str,
) -> Result<()> {
    let worktree_storage = Storage::new(Some(worktree_path.to_path_buf()));
    let worktree_phases = worktree_storage.load_tasks().ok();

    let main_storage = Storage::new(Some(project_root.to_path_buf()));
    let main_phases = main_storage.load_tasks()?;

    let worktree_tasks = worktree_path
        .join(".scud")
        .join("tasks")
        .join("tasks.scg");
    let mut output = String::new();

    // For target tag: prefer worktree version (has in-progress status changes)
    // Fall back to main if worktree doesn't have it yet
    if let Some(phase) = worktree_phases
        .as_ref()
        .and_then(|p| p.get(tag))
        .or_else(|| main_phases.get(tag))
    {
        output.push_str(&serialize_scg(phase));
    }

    // Collapsed stubs for other tags (always from main)
    for (other_tag, phase) in &main_phases {
        if other_tag != tag {
            if !output.is_empty() {
                output.push_str("\n---\n\n");
            }
            output.push_str("# SCUD Graph v1\n");
            output.push_str(&format!("# Phase: {}\n", other_tag));
            output.push_str(&format!(
                "# [Collapsed - {} tasks]\n\n",
                phase.tasks.len()
            ));
            output.push_str(&format!("@meta {{\n  name {}\n}}\n", phase.name));
            output.push_str("\n@nodes\n");
            output.push_str("# Tasks hidden. Run `scud salvo sync` to merge changes.\n");
        }
    }

    std::fs::write(&worktree_tasks, output)?;
    Ok(())
}

/// Sync task status changes from worktree back to main branch's tasks.scg
pub fn sync_to_main(project_root: &Path, worktree_path: &Path, tag: &str) -> Result<()> {
    let worktree_storage = Storage::new(Some(worktree_path.to_path_buf()));
    let worktree_phases = worktree_storage.load_tasks()?;

    let worktree_phase = worktree_phases
        .get(tag)
        .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found in worktree", tag))?;

    let main_storage = Storage::new(Some(project_root.to_path_buf()));

    // Use update_group for atomic read-modify-write on main
    main_storage.update_group(tag, worktree_phase)?;

    // Record sync time
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "UPDATE salvo_worktrees SET last_sync_at = datetime('now') WHERE tag = ?",
        [tag],
    )?;

    println!("Synced salvo '{}' back to main", tag);
    Ok(())
}

/// List all salvo worktrees
pub fn list_worktrees(project_root: &Path) -> Result<()> {
    let db = Database::new(project_root);
    db.initialize()?;
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();

    let mut stmt = conn.prepare(
        "SELECT tag, worktree_path, branch_name, created_at, last_sync_at
         FROM salvo_worktrees ORDER BY created_at DESC",
    )?;

    let worktrees: Vec<(String, String, String, String, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if worktrees.is_empty() {
        println!("No salvo worktrees found.");
        println!("Create one by running: scud swarm --tag <tag>");
        return Ok(());
    }

    println!("Salvo Worktrees:");
    println!(
        "{:<15} {:<40} {:<20} {}",
        "Tag", "Path", "Branch", "Last Sync"
    );
    println!("{}", "-".repeat(90));

    for (tag, path, branch, _created, synced) in &worktrees {
        let sync_display = synced.as_deref().unwrap_or("never");
        let exists = Path::new(path).exists();
        let status = if exists { "" } else { " (missing)" };
        println!(
            "{:<15} {:<40} {:<20} {}{}",
            tag, path, branch, sync_display, status
        );
    }

    Ok(())
}

/// Remove a salvo worktree and its git branch
pub fn remove_worktree(project_root: &Path, tag: &str) -> Result<()> {
    let db = Database::new(project_root);
    db.initialize()?;
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();

    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT worktree_path, branch_name FROM salvo_worktrees WHERE tag = ?",
            [tag],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if let Some((path, _branch)) = row {
        // Remove git worktree (--force needed because .scud/ files are untracked)
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force", &path])
            .current_dir(project_root)
            .output();
    }

    conn.execute("DELETE FROM salvo_worktrees WHERE tag = ?", [tag])?;
    println!("Removed salvo worktree for '{}'", tag);
    Ok(())
}
