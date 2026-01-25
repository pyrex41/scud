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

use crate::backpressure::ValidationResult;

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

/// State of a code review for a wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewState {
    /// Tasks that were reviewed
    pub reviewed_tasks: Vec<String>,
    /// Whether all reviewed tasks passed
    pub all_passed: bool,
    /// Tasks flagged for improvement
    pub tasks_needing_improvement: Vec<String>,
    /// When the review completed
    pub completed_at: String,
}

/// Record of a repair attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttempt {
    /// Attempt number (1-indexed)
    pub attempt_number: usize,
    /// Tasks attributed as responsible for the failure
    pub attributed_tasks: Vec<String>,
    /// Tasks cleared as not responsible
    pub cleared_tasks: Vec<String>,
    /// Attribution confidence level: "high", "medium", "low"
    pub attribution_confidence: String,
    /// Whether validation passed after this repair
    pub validation_passed: bool,
    /// When the repair attempt completed
    pub completed_at: String,
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
    /// Review result (if review was run)
    #[serde(default)]
    pub review: Option<ReviewState>,
    /// Repair attempts (if validation failed)
    #[serde(default)]
    pub repairs: Vec<RepairAttempt>,
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
            review: None,
            repairs: Vec::new(),
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

// ============================================================================
// Extension-based Agent Spawning
// ============================================================================

use std::path::Path;
use tokio::sync::mpsc;

use crate::commands::spawn::agent::generate_prompt;
use crate::commands::spawn::terminal::Harness;
use crate::extensions::runner::{load_agent_config, AgentEvent, AgentRunner, SpawnConfig};
use crate::models::task::Task;

/// Agent info for wave execution
#[derive(Debug, Clone)]
pub struct WaveAgent {
    /// Task being executed
    pub task: Task,
    /// Tag/phase the task belongs to
    pub tag: String,
}

impl WaveAgent {
    /// Create a WaveAgent from a task and tag
    pub fn new(task: Task, tag: impl Into<String>) -> Self {
        Self {
            task,
            tag: tag.into(),
        }
    }

    /// Create WaveAgents from a collection of (task, tag) pairs
    ///
    /// This is the primary conversion point from graph computation output
    /// to extension-based spawning input.
    pub fn from_task_pairs<I>(pairs: I) -> Vec<Self>
    where
        I: IntoIterator<Item = (Task, String)>,
    {
        pairs.into_iter().map(|(task, tag)| Self::new(task, tag)).collect()
    }

    /// Get the task ID
    pub fn task_id(&self) -> &str {
        &self.task.id
    }
}

/// Result from wave execution
#[derive(Debug, Clone)]
pub struct WaveExecutionResult {
    /// Round state with execution info
    pub round_state: RoundState,
    /// Results from each agent
    pub agent_results: Vec<crate::extensions::runner::AgentResult>,
}

impl WaveExecutionResult {
    /// Check if all agents completed successfully
    pub fn all_succeeded(&self) -> bool {
        self.agent_results.iter().all(|r| r.success)
    }

    /// Get task IDs that completed successfully
    pub fn successful_task_ids(&self) -> Vec<String> {
        self.agent_results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.task_id.clone())
            .collect()
    }

    /// Get task IDs that failed
    pub fn failed_task_ids(&self) -> Vec<String> {
        self.agent_results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.task_id.clone())
            .collect()
    }

    /// Get total execution duration in milliseconds
    pub fn total_duration_ms(&self) -> u64 {
        self.agent_results.iter().map(|r| r.duration_ms).max().unwrap_or(0)
    }
}

impl WaveState {
    /// Update this wave state from an execution result
    ///
    /// This integrates the extension-based execution results into
    /// the existing wave tracking structure.
    pub fn apply_execution_result(&mut self, result: WaveExecutionResult) {
        self.rounds.push(result.round_state);
    }

    /// Create a WaveState and immediately apply an execution result
    pub fn from_execution_result(wave_number: usize, result: WaveExecutionResult) -> Self {
        let mut state = Self::new(wave_number);
        state.apply_execution_result(result);
        state
    }
}

/// Execute a wave of agents using extension-based spawning (no tmux)
///
/// This function spawns agents as direct subprocesses and waits for them
/// to complete, collecting their results.
///
/// # Arguments
/// * `agents` - The agents to execute in this wave
/// * `working_dir` - Working directory for agents
/// * `round_number` - Round number for state tracking
/// * `default_harness` - Default harness if agent doesn't specify one
/// * `event_callback` - Optional callback for agent events
///
/// # Returns
/// WaveExecutionResult with round state and agent results
pub async fn execute_wave_async(
    agents: &[WaveAgent],
    working_dir: &Path,
    round_number: usize,
    default_harness: Harness,
) -> Result<WaveExecutionResult> {
    let mut round_state = RoundState::new(round_number);
    let mut runner = AgentRunner::new(agents.len() * 10);

    // Spawn all agents
    for agent in agents {
        // Resolve agent config (harness, model) from agent_type
        let (harness, model) = load_agent_config(
            agent.task.agent_type.as_deref(),
            default_harness,
            None,
            working_dir,
        );

        // Generate prompt for the agent
        let prompt = generate_prompt(&agent.task, &agent.tag);

        let config = SpawnConfig {
            task_id: agent.task.id.clone(),
            prompt,
            working_dir: working_dir.to_path_buf(),
            harness,
            model,
        };

        match runner.spawn(config).await {
            Ok(()) => {
                round_state.task_ids.push(agent.task.id.clone());
                round_state.tags.push(agent.tag.clone());
            }
            Err(e) => {
                round_state.failures.push(agent.task.id.clone());
                eprintln!("Failed to spawn agent for {}: {}", agent.task.id, e);
            }
        }
    }

    // Wait for all agents to complete
    let agent_results = runner.wait_all().await;

    round_state.mark_complete();

    Ok(WaveExecutionResult {
        round_state,
        agent_results,
    })
}

/// Execute a wave of agents with event streaming
///
/// Similar to execute_wave_async but allows receiving events during execution.
///
/// # Arguments
/// * `agents` - The agents to execute in this wave
/// * `working_dir` - Working directory for agents
/// * `round_number` - Round number for state tracking
/// * `default_harness` - Default harness if agent doesn't specify one
/// * `event_tx` - Channel to send events to
///
/// # Returns
/// WaveExecutionResult with round state and agent results
pub async fn execute_wave_with_events(
    agents: &[WaveAgent],
    working_dir: &Path,
    round_number: usize,
    default_harness: Harness,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<WaveExecutionResult> {
    use crate::extensions::runner::spawn_agent;

    let mut round_state = RoundState::new(round_number);
    let mut handles = Vec::new();

    // Spawn all agents
    for agent in agents {
        // Resolve agent config (harness, model) from agent_type
        let (harness, model) = load_agent_config(
            agent.task.agent_type.as_deref(),
            default_harness,
            None,
            working_dir,
        );

        // Generate prompt for the agent
        let prompt = generate_prompt(&agent.task, &agent.tag);

        let config = SpawnConfig {
            task_id: agent.task.id.clone(),
            prompt,
            working_dir: working_dir.to_path_buf(),
            harness,
            model,
        };

        match spawn_agent(config, event_tx.clone()).await {
            Ok(handle) => {
                handles.push(handle);
                round_state.task_ids.push(agent.task.id.clone());
                round_state.tags.push(agent.tag.clone());
            }
            Err(e) => {
                round_state.failures.push(agent.task.id.clone());
                let _ = event_tx
                    .send(AgentEvent::SpawnFailed {
                        task_id: agent.task.id.clone(),
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    }

    // Wait for all agents to complete
    let mut agent_results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            agent_results.push(result);
        }
    }

    round_state.mark_complete();

    Ok(WaveExecutionResult {
        round_state,
        agent_results,
    })
}

/// Execute a complete wave with state tracking
///
/// This is the high-level interface for executing a wave of agents using
/// extension-based spawning. It handles:
/// - Converting task info to WaveAgent format
/// - Spawning and managing agents
/// - Updating wave state with results
///
/// # Arguments
/// * `wave_number` - The wave number for state tracking
/// * `task_pairs` - Iterator of (Task, tag) pairs from graph computation
/// * `working_dir` - Working directory for agents
/// * `default_harness` - Default harness if agent doesn't specify one
///
/// # Returns
/// A tuple of (WaveState, Vec<AgentResult>) with the tracked state and raw results
pub async fn execute_wave_with_tracking<I>(
    wave_number: usize,
    task_pairs: I,
    working_dir: &Path,
    default_harness: Harness,
) -> Result<(WaveState, Vec<crate::extensions::runner::AgentResult>)>
where
    I: IntoIterator<Item = (Task, String)>,
{
    let agents = WaveAgent::from_task_pairs(task_pairs);
    let result = execute_wave_async(&agents, working_dir, 0, default_harness).await?;

    let wave_state = WaveState::from_execution_result(wave_number, result.clone());

    Ok((wave_state, result.agent_results))
}

/// Execute multiple rounds within a wave with state tracking
///
/// This function handles chunking agents into rounds based on round_size
/// and executes them sequentially, tracking state for each round.
///
/// # Arguments
/// * `wave_number` - The wave number for state tracking
/// * `task_pairs` - Iterator of (Task, tag) pairs from graph computation
/// * `working_dir` - Working directory for agents
/// * `round_size` - Maximum number of agents per round
/// * `default_harness` - Default harness if agent doesn't specify one
///
/// # Returns
/// WaveState with all rounds tracked
pub async fn execute_wave_in_rounds<I>(
    wave_number: usize,
    task_pairs: I,
    working_dir: &Path,
    round_size: usize,
    default_harness: Harness,
) -> Result<WaveState>
where
    I: IntoIterator<Item = (Task, String)>,
{
    let agents: Vec<WaveAgent> = WaveAgent::from_task_pairs(task_pairs);
    let mut wave_state = WaveState::new(wave_number);

    for (round_idx, chunk) in agents.chunks(round_size).enumerate() {
        let result = execute_wave_async(chunk, working_dir, round_idx, default_harness).await?;
        wave_state.apply_execution_result(result);
    }

    wave_state.mark_complete();
    Ok(wave_state)
}

/// Spawn a single agent using extension-based spawning (async, no tmux)
///
/// This is a convenience function for spawning a single agent.
pub async fn spawn_subagent(
    task: &Task,
    tag: &str,
    working_dir: &Path,
    default_harness: Harness,
) -> Result<crate::extensions::runner::AgentResult> {
    let agents = vec![WaveAgent {
        task: task.clone(),
        tag: tag.to_string(),
    }];

    let result = execute_wave_async(&agents, working_dir, 0, default_harness).await?;

    result
        .agent_results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No result from agent"))
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

    #[test]
    fn test_wave_agent_new() {
        let task = Task::new(
            "task:1".to_string(),
            "Test task".to_string(),
            "Description".to_string(),
        );
        let agent = WaveAgent::new(task.clone(), "test-tag");

        assert_eq!(agent.task_id(), "task:1");
        assert_eq!(agent.tag, "test-tag");
    }

    #[test]
    fn test_wave_agent_from_task_pairs() {
        let task1 = Task::new(
            "task:1".to_string(),
            "Task 1".to_string(),
            "Description".to_string(),
        );
        let task2 = Task::new(
            "task:2".to_string(),
            "Task 2".to_string(),
            "Description".to_string(),
        );

        let pairs = vec![
            (task1, "tag-a".to_string()),
            (task2, "tag-b".to_string()),
        ];

        let agents = WaveAgent::from_task_pairs(pairs);

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].task_id(), "task:1");
        assert_eq!(agents[0].tag, "tag-a");
        assert_eq!(agents[1].task_id(), "task:2");
        assert_eq!(agents[1].tag, "tag-b");
    }

    #[test]
    fn test_wave_execution_result_helpers() {
        use crate::extensions::runner::AgentResult;

        let result = WaveExecutionResult {
            round_state: RoundState::new(0),
            agent_results: vec![
                AgentResult {
                    task_id: "task:1".to_string(),
                    success: true,
                    exit_code: Some(0),
                    output: String::new(),
                    duration_ms: 1000,
                },
                AgentResult {
                    task_id: "task:2".to_string(),
                    success: false,
                    exit_code: Some(1),
                    output: String::new(),
                    duration_ms: 2000,
                },
            ],
        };

        assert!(!result.all_succeeded());
        assert_eq!(result.successful_task_ids(), vec!["task:1"]);
        assert_eq!(result.failed_task_ids(), vec!["task:2"]);
        assert_eq!(result.total_duration_ms(), 2000);
    }

    #[test]
    fn test_wave_state_from_execution_result() {
        use crate::extensions::runner::AgentResult;

        let mut round_state = RoundState::new(0);
        round_state.task_ids = vec!["task:1".to_string()];

        let result = WaveExecutionResult {
            round_state,
            agent_results: vec![AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: String::new(),
                duration_ms: 1000,
            }],
        };

        let wave_state = WaveState::from_execution_result(1, result);

        assert_eq!(wave_state.wave_number, 1);
        assert_eq!(wave_state.rounds.len(), 1);
        assert_eq!(wave_state.rounds[0].task_ids, vec!["task:1"]);
    }

    #[test]
    fn test_wave_state_apply_execution_result() {
        use crate::extensions::runner::AgentResult;

        let mut wave_state = WaveState::new(1);
        assert!(wave_state.rounds.is_empty());

        let mut round_state = RoundState::new(0);
        round_state.task_ids = vec!["task:1".to_string()];

        let result = WaveExecutionResult {
            round_state,
            agent_results: vec![AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: String::new(),
                duration_ms: 1000,
            }],
        };

        wave_state.apply_execution_result(result);

        assert_eq!(wave_state.rounds.len(), 1);
        assert_eq!(wave_state.all_task_ids(), vec!["task:1"]);
    }
}
