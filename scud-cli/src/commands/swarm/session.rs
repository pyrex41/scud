//! Swarm session state tracking
//!
//! Tracks the state of a swarm execution session, including:
//! - Waves executed
//! - Rounds within waves
//! - Task completion status
//! - Validation results

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::backpressure::ValidationResult;

/// Get the current git commit SHA
pub fn get_current_commit() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Brief summary of what was done in a wave
/// This is NOT accumulated context - just a simple summary for the next wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveSummary {
    /// Wave number
    pub wave_number: usize,
    /// Tasks that were completed
    pub tasks_completed: Vec<String>,
    /// Files that were changed
    pub files_changed: Vec<String>,
}

impl WaveSummary {
    /// Generate a brief text summary
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Wave {} completed {} task(s):",
            self.wave_number,
            self.tasks_completed.len()
        ));

        for task_id in &self.tasks_completed {
            lines.push(format!("  - {}", task_id));
        }

        if !self.files_changed.is_empty() {
            let file_summary = if self.files_changed.len() <= 5 {
                self.files_changed.join(", ")
            } else {
                format!(
                    "{} and {} more",
                    self.files_changed[..5].join(", "),
                    self.files_changed.len() - 5
                )
            };
            lines.push(format!("Files changed: {}", file_summary));
        }

        lines.join("\n")
    }
}

/// State of a single round within a wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundState {
    /// Round number (0-indexed)
    pub round_number: usize,
    /// Task IDs executed in this round
    pub task_ids: Vec<String>,
    /// Tags for each task
    pub tags: Vec<String>,
    /// Tasks that failed to spawn
    pub failures: Vec<String>,
    /// Start time
    pub started_at: String,
    /// End time (set when complete)
    pub completed_at: Option<String>,
}

impl RoundState {
    pub fn new(round_number: usize) -> Self {
        Self {
            round_number,
            task_ids: Vec::new(),
            tags: Vec::new(),
            failures: Vec::new(),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    pub fn mark_complete(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// State of a single wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveState {
    /// Wave number (1-indexed)
    pub wave_number: usize,
    /// Rounds executed in this wave
    pub rounds: Vec<RoundState>,
    /// Validation result (if validation was run)
    pub validation: Option<ValidationResult>,
    /// Summary of what was done
    pub summary: Option<WaveSummary>,
    /// Git commit SHA at wave start (for tracking changes)
    #[serde(default)]
    pub start_commit: Option<String>,
    /// Start time
    pub started_at: String,
    /// End time (set when complete)
    pub completed_at: Option<String>,
}

impl WaveState {
    pub fn new(wave_number: usize) -> Self {
        Self {
            wave_number,
            rounds: Vec::new(),
            validation: None,
            summary: None,
            start_commit: get_current_commit(),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    pub fn mark_complete(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Get all task IDs from all rounds
    pub fn all_task_ids(&self) -> Vec<String> {
        self.rounds
            .iter()
            .flat_map(|r| r.task_ids.clone())
            .collect()
    }

    /// Get task ID to tag mapping
    pub fn task_tags(&self) -> Vec<(String, String)> {
        self.rounds
            .iter()
            .flat_map(|r| {
                r.task_ids
                    .iter()
                    .zip(r.tags.iter())
                    .map(|(id, tag)| (id.clone(), tag.clone()))
            })
            .collect()
    }
}

/// Full swarm session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSession {
    /// Session name
    pub session_name: String,
    /// Tag being executed
    pub tag: String,
    /// Terminal type
    pub terminal: String,
    /// Working directory
    pub working_dir: String,
    /// Round size (max tasks per round)
    pub round_size: usize,
    /// Waves executed
    pub waves: Vec<WaveState>,
    /// Session start time
    pub started_at: String,
    /// Session end time
    pub completed_at: Option<String>,
}

impl SwarmSession {
    pub fn new(
        session_name: &str,
        tag: &str,
        terminal: &str,
        working_dir: &str,
        round_size: usize,
    ) -> Self {
        Self {
            session_name: session_name.to_string(),
            tag: tag.to_string(),
            terminal: terminal.to_string(),
            working_dir: working_dir.to_string(),
            round_size,
            waves: Vec::new(),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    pub fn mark_complete(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Get total tasks executed
    pub fn total_tasks(&self) -> usize {
        self.waves
            .iter()
            .flat_map(|w| &w.rounds)
            .map(|r| r.task_ids.len())
            .sum()
    }

    /// Get total failures
    pub fn total_failures(&self) -> usize {
        self.waves
            .iter()
            .flat_map(|w| &w.rounds)
            .map(|r| r.failures.len())
            .sum()
    }

    /// Get brief summary of the previous wave (if any)
    /// This is just "what was done", not accumulated context
    pub fn get_previous_summary(&self) -> Option<String> {
        self.waves
            .last()
            .and_then(|w| w.summary.as_ref().map(|s| s.to_text()))
    }
}

/// Get the swarm session directory
pub fn swarm_dir(project_root: Option<&PathBuf>) -> PathBuf {
    let root = project_root
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    root.join(".scud").join("swarm")
}

/// Get the path to the session lock file for a given tag
pub fn lock_file_path(project_root: Option<&PathBuf>, tag: &str) -> PathBuf {
    swarm_dir(project_root).join(format!("{}.lock", tag))
}

/// A session lock that prevents concurrent swarm sessions on the same tag.
/// The lock is automatically released when this struct is dropped.
pub struct SessionLock {
    _file: fs::File,
    path: PathBuf,
}

impl SessionLock {
    /// Get the path to the lock file
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        // Lock is released automatically when file is dropped
        // Optionally remove the lock file
        let _ = fs::remove_file(&self.path);
    }
}

/// Acquire an exclusive session lock for a tag.
/// Returns a SessionLock that will be released when dropped.
/// Returns an error if another session already holds the lock.
pub fn acquire_session_lock(project_root: Option<&PathBuf>, tag: &str) -> Result<SessionLock> {
    use fs2::FileExt;

    let dir = swarm_dir(project_root);
    fs::create_dir_all(&dir)?;

    let lock_path = lock_file_path(project_root, tag);
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)?;

    // Try to acquire exclusive lock (non-blocking)
    file.try_lock_exclusive().map_err(|_| {
        anyhow::anyhow!(
            "Another swarm session is already running for tag '{}'. \
             If this is incorrect, remove the lock file: {}",
            tag,
            lock_path.display()
        )
    })?;

    // Write PID and timestamp to lock file for debugging
    use std::io::Write;
    let mut file = file;
    writeln!(
        file,
        "pid={}\nstarted={}",
        std::process::id(),
        chrono::Utc::now().to_rfc3339()
    )?;

    Ok(SessionLock {
        _file: file,
        path: lock_path,
    })
}

/// Get the path to a session's state file
pub fn session_file(project_root: Option<&PathBuf>, session_name: &str) -> PathBuf {
    swarm_dir(project_root).join(format!("{}.json", session_name))
}

/// Save swarm session state
pub fn save_session(project_root: Option<&PathBuf>, session: &SwarmSession) -> Result<()> {
    let dir = swarm_dir(project_root);
    fs::create_dir_all(&dir)?;

    let file = session_file(project_root, &session.session_name);
    let json = serde_json::to_string_pretty(session)?;
    fs::write(file, json)?;

    Ok(())
}

/// Load swarm session state
pub fn load_session(project_root: Option<&PathBuf>, session_name: &str) -> Result<SwarmSession> {
    let file = session_file(project_root, session_name);
    let json = fs::read_to_string(&file)?;
    let session: SwarmSession = serde_json::from_str(&json)?;
    Ok(session)
}

/// List all swarm sessions
pub fn list_sessions(project_root: Option<&PathBuf>) -> Result<Vec<String>> {
    let dir = swarm_dir(project_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Some(stem) = path.file_stem() {
                sessions.push(stem.to_string_lossy().to_string());
            }
        }
    }

    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_state_new() {
        let round = RoundState::new(0);
        assert_eq!(round.round_number, 0);
        assert!(round.task_ids.is_empty());
        assert!(round.completed_at.is_none());
    }

    #[test]
    fn test_wave_state_all_task_ids() {
        let mut wave = WaveState::new(1);

        let mut round1 = RoundState::new(0);
        round1.task_ids = vec!["task:1".to_string(), "task:2".to_string()];

        let mut round2 = RoundState::new(1);
        round2.task_ids = vec!["task:3".to_string()];

        wave.rounds.push(round1);
        wave.rounds.push(round2);

        let all_ids = wave.all_task_ids();
        assert_eq!(all_ids.len(), 3);
        assert!(all_ids.contains(&"task:1".to_string()));
        assert!(all_ids.contains(&"task:2".to_string()));
        assert!(all_ids.contains(&"task:3".to_string()));
    }

    #[test]
    fn test_swarm_session_total_tasks() {
        let mut session = SwarmSession::new("test-session", "test-tag", "tmux", "/test/path", 5);

        let mut wave = WaveState::new(1);
        let mut round = RoundState::new(0);
        round.task_ids = vec!["task:1".to_string(), "task:2".to_string()];
        wave.rounds.push(round);
        session.waves.push(wave);

        assert_eq!(session.total_tasks(), 2);
    }

    #[test]
    fn test_wave_summary_to_text() {
        let summary = WaveSummary {
            wave_number: 1,
            tasks_completed: vec!["task:1".to_string(), "task:2".to_string()],
            files_changed: vec!["src/main.rs".to_string()],
        };

        let text = summary.to_text();
        assert!(text.contains("Wave 1"));
        assert!(text.contains("task:1"));
        assert!(text.contains("src/main.rs"));
    }

    #[test]
    fn test_get_previous_summary() {
        let mut session = SwarmSession::new("test", "tag", "tmux", "/path", 5);

        // No waves yet
        assert!(session.get_previous_summary().is_none());

        // Add wave with summary
        let mut wave = WaveState::new(1);
        wave.summary = Some(WaveSummary {
            wave_number: 1,
            tasks_completed: vec!["task:1".to_string()],
            files_changed: vec![],
        });
        session.waves.push(wave);

        let summary = session.get_previous_summary();
        assert!(summary.is_some());
        assert!(summary.unwrap().contains("task:1"));
    }

    #[test]
    fn test_session_lock_contention() {
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Acquire first lock
        let _lock1 = acquire_session_lock(Some(&project_root), "test-tag")
            .expect("First lock should succeed");

        // Try to acquire second lock for same tag while first is held
        let result = acquire_session_lock(Some(&project_root), "test-tag");

        // Verify the second attempt fails and error message mentions "already running"
        match result {
            Ok(_) => panic!("Second lock should fail"),
            Err(e) => {
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("already running"),
                    "Error message should mention 'already running', got: {}",
                    error_msg
                );
            }
        }
    }

    #[test]
    fn test_get_current_commit() {
        let result = get_current_commit();

        // Should return Some(sha) since we're in a git repo
        assert!(result.is_some(), "Expected Some(sha) in a git repository");

        let sha = result.unwrap();

        // Verify the SHA is 40 characters long (full SHA)
        assert_eq!(
            sha.len(),
            40,
            "Expected SHA to be 40 characters long, got {}",
            sha.len()
        );

        // Verify the SHA contains only hex characters (0-9, a-f)
        assert!(
            sha.chars().all(|c| c.is_ascii_hexdigit()),
            "Expected SHA to contain only hex characters, got: {}",
            sha
        );
    }
}
