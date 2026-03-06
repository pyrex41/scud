//! Attach command - Continue a headless session interactively
//!
//! This module provides functionality to resume a headless agent session
//! by reading stored session metadata and launching the appropriate
//! harness with session continuation flags.

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::commands::spawn::terminal::{find_harness_binary, Harness};
use crate::storage::Storage;

/// Metadata for a headless session, stored for later continuation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Task ID this session is for
    pub task_id: String,
    /// Harness session ID (for --resume)
    pub session_id: String,
    /// Associated tag/phase
    pub tag: String,
    /// Process ID (if still running)
    #[serde(default)]
    pub pid: Option<u32>,
    /// Harness used (rho, claude, opencode, cursor)
    #[serde(default = "default_harness")]
    pub harness: String,
}

fn default_harness() -> String {
    "rho".to_string()
}

impl SessionMetadata {
    /// Create new session metadata
    pub fn new(task_id: &str, session_id: &str, tag: &str, harness: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            session_id: session_id.to_string(),
            tag: tag.to_string(),
            pid: None,
            harness: harness.to_string(),
        }
    }

    /// Set the process ID
    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }
}

/// Get the directory for headless session metadata
pub fn headless_metadata_dir(project_root: &Path) -> PathBuf {
    project_root.join(".scud").join("headless")
}

/// Get the path to a session metadata file
pub fn session_metadata_path(project_root: &Path, task_id: &str) -> PathBuf {
    headless_metadata_dir(project_root).join(format!("{}.json", task_id))
}

/// Save session metadata for later continuation
pub fn save_session_metadata(project_root: &Path, metadata: &SessionMetadata) -> Result<()> {
    let metadata_dir = headless_metadata_dir(project_root);
    std::fs::create_dir_all(&metadata_dir)
        .context("Failed to create headless metadata directory")?;

    let metadata_file = session_metadata_path(project_root, &metadata.task_id);
    let content =
        serde_json::to_string_pretty(metadata).context("Failed to serialize session metadata")?;

    std::fs::write(&metadata_file, content).context("Failed to write session metadata")?;

    Ok(())
}

/// Load session metadata for a task
pub fn load_session_metadata(project_root: &Path, task_id: &str) -> Result<SessionMetadata> {
    let metadata_file = session_metadata_path(project_root, task_id);

    if !metadata_file.exists() {
        anyhow::bail!(
            "No session metadata found for task '{}'. Was it run in headless mode?",
            task_id
        );
    }

    let content =
        std::fs::read_to_string(&metadata_file).context("Failed to read session metadata")?;

    let metadata: SessionMetadata =
        serde_json::from_str(&content).context("Failed to parse session metadata")?;

    Ok(metadata)
}

/// List all available headless sessions
pub fn list_sessions(project_root: &Path) -> Result<Vec<SessionMetadata>> {
    let metadata_dir = headless_metadata_dir(project_root);

    if !metadata_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for entry in std::fs::read_dir(&metadata_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(metadata) = serde_json::from_str::<SessionMetadata>(&content) {
                    sessions.push(metadata);
                }
            }
        }
    }

    Ok(sessions)
}

/// Delete session metadata after successful continuation
pub fn delete_session_metadata(project_root: &Path, task_id: &str) -> Result<()> {
    let metadata_file = session_metadata_path(project_root, task_id);

    if metadata_file.exists() {
        std::fs::remove_file(&metadata_file).context("Failed to delete session metadata")?;
    }

    Ok(())
}

/// Build the command arguments for interactive session continuation
pub fn interactive_command(harness: Harness, session_id: &str) -> Result<Vec<String>> {
    let binary_path = find_harness_binary(harness)?.to_string();

    match harness {
        Harness::Claude => Ok(vec![
            binary_path,
            "--resume".to_string(),
            session_id.to_string(),
        ]),
        Harness::OpenCode => {
            // OpenCode uses attach command with server URL and session
            Ok(vec![
                binary_path,
                "attach".to_string(),
                "http://localhost:4096".to_string(),
                "--session".to_string(),
                session_id.to_string(),
            ])
        }
        Harness::Cursor => Ok(vec![
            binary_path,
            "--resume".to_string(),
            session_id.to_string(),
        ]),
        Harness::Rho => anyhow::bail!("Rho sessions cannot be resumed interactively yet"),
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => anyhow::bail!("Direct API sessions cannot be resumed interactively"),
    }
}

/// Main entry point for the attach command
pub fn run(project_root: Option<PathBuf>, task_id: &str, harness_arg: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root.clone());
    let root = storage.project_root().to_path_buf();

    // Load session metadata
    let metadata = load_session_metadata(&root, task_id)?;

    // Determine harness - prefer stored harness, fall back to argument
    let harness_str = harness_arg.unwrap_or(&metadata.harness);
    let harness = Harness::parse(harness_str)?;

    // Build interactive command
    let cmd_args = interactive_command(harness, &metadata.session_id)?;

    // Display info
    println!("{}", "SCUD Attach".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<15} {}", "Task:".dimmed(), task_id.cyan());
    println!(
        "{:<15} {}",
        "Session:".dimmed(),
        metadata.session_id.dimmed()
    );
    println!("{:<15} {}", "Tag:".dimmed(), metadata.tag);
    println!("{:<15} {}", "Harness:".dimmed(), harness.name().green());
    if let Some(pid) = metadata.pid {
        println!("{:<15} {}", "PID:".dimmed(), pid);
    }
    println!();
    println!("{}", "Attaching to session...".cyan());
    println!();

    // Use exec to replace current process with interactive session
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        let err = std::process::Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .exec();

        // exec() only returns on error
        anyhow::bail!("Failed to exec '{}': {}", cmd_args[0], err);
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, spawn a child process and wait
        let status = std::process::Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .status()
            .context("Failed to spawn interactive session")?;

        if !status.success() {
            anyhow::bail!("Interactive session exited with error");
        }

        Ok(())
    }
}

/// List available headless sessions
pub fn run_list(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);
    let root = storage.project_root().to_path_buf();

    let sessions = list_sessions(&root)?;

    if sessions.is_empty() {
        println!("{}", "No headless sessions found.".dimmed());
        println!(
            "Run {} to start a headless session.",
            "scud spawn --headless".cyan()
        );
        return Ok(());
    }

    println!("{}", "Headless Sessions:".cyan().bold());
    println!();

    for session in &sessions {
        let pid_info = session
            .pid
            .map(|p| format!(" (pid: {})", p))
            .unwrap_or_default();

        println!(
            "  {} {} [{}]{}",
            session.task_id.cyan(),
            session.tag.dimmed(),
            session.harness.green(),
            pid_info.dimmed()
        );
    }

    println!();
    println!(
        "{}",
        "Use 'scud attach <task_id>' to resume a session.".dimmed()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_metadata_serialization() {
        let metadata =
            SessionMetadata::new("1.1", "sess-abc123", "alpha", "claude").with_pid(12345);

        let json = serde_json::to_string(&metadata).unwrap();
        let parsed: SessionMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.task_id, "1.1");
        assert_eq!(parsed.session_id, "sess-abc123");
        assert_eq!(parsed.tag, "alpha");
        assert_eq!(parsed.harness, "claude");
        assert_eq!(parsed.pid, Some(12345));
    }

    #[test]
    fn test_save_and_load_session_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let metadata = SessionMetadata::new("2.1", "sess-xyz789", "beta", "opencode");

        // Save metadata
        save_session_metadata(project_root, &metadata).unwrap();

        // Verify file exists
        let metadata_path = session_metadata_path(project_root, "2.1");
        assert!(metadata_path.exists());

        // Load metadata
        let loaded = load_session_metadata(project_root, "2.1").unwrap();
        assert_eq!(loaded.task_id, "2.1");
        assert_eq!(loaded.session_id, "sess-xyz789");
        assert_eq!(loaded.tag, "beta");
        assert_eq!(loaded.harness, "opencode");
    }

    #[test]
    fn test_load_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let result = load_session_metadata(project_root, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No session metadata found"));
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create multiple sessions
        save_session_metadata(
            project_root,
            &SessionMetadata::new("1.1", "sess-1", "alpha", "claude"),
        )
        .unwrap();
        save_session_metadata(
            project_root,
            &SessionMetadata::new("2.1", "sess-2", "beta", "opencode"),
        )
        .unwrap();

        let sessions = list_sessions(project_root).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_delete_session_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let metadata = SessionMetadata::new("3.1", "sess-delete", "gamma", "claude");
        save_session_metadata(project_root, &metadata).unwrap();

        let metadata_path = session_metadata_path(project_root, "3.1");
        assert!(metadata_path.exists());

        delete_session_metadata(project_root, "3.1").unwrap();
        assert!(!metadata_path.exists());
    }

    #[test]
    fn test_default_harness() {
        // Test that old metadata without harness field deserializes correctly
        let json = r#"{"task_id": "1.1", "session_id": "sess-123", "tag": "test"}"#;
        let metadata: SessionMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.harness, "rho");
    }

    #[test]
    fn test_interactive_command_claude() {
        // This test will only pass if claude binary is found
        // Skip if not available
        if find_harness_binary(Harness::Claude).is_err() {
            return;
        }

        let cmd = interactive_command(Harness::Claude, "sess-123").unwrap();
        assert!(cmd.len() >= 3);
        assert!(cmd[0].contains("claude"));
        assert_eq!(cmd[1], "--resume");
        assert_eq!(cmd[2], "sess-123");
    }
}
