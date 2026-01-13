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

use super::backpressure::ValidationResult;

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
}
