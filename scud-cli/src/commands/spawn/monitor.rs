//! Monitor data structures for TUI integration
//!
//! Provides data structures and functions for tracking spawn session state,
//! to be consumed by a TUI control window.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// State of a spawned agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Information about a spawned agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub task_id: String,
    pub task_title: String,
    pub window_name: String,
    pub status: AgentStatus,
    pub started_at: String,
    pub tag: String,
}

/// Spawn session state for TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnSession {
    pub session_name: String,
    pub tag: String,
    pub terminal: String,
    pub created_at: String,
    pub working_dir: String,
    pub agents: Vec<AgentState>,
}

impl SpawnSession {
    /// Create a new spawn session
    pub fn new(session_name: &str, tag: &str, terminal: &str, working_dir: &str) -> Self {
        Self {
            session_name: session_name.to_string(),
            tag: tag.to_string(),
            terminal: terminal.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            working_dir: working_dir.to_string(),
            agents: Vec::new(),
        }
    }

    /// Add an agent to the session
    pub fn add_agent(&mut self, task_id: &str, task_title: &str, tag: &str) {
        let window_name = format!("task-{}", task_id);
        self.agents.push(AgentState {
            task_id: task_id.to_string(),
            task_title: task_title.to_string(),
            window_name,
            status: AgentStatus::Starting,
            started_at: chrono::Utc::now().to_rfc3339(),
            tag: tag.to_string(),
        });
    }

    /// Update agent status
    pub fn update_agent_status(&mut self, task_id: &str, status: AgentStatus) {
        if let Some(agent) = self.agents.iter_mut().find(|a| a.task_id == task_id) {
            agent.status = status;
        }
    }

    /// Count agents by status
    pub fn count_by_status(&self, status: AgentStatus) -> usize {
        self.agents.iter().filter(|a| a.status == status).count()
    }
}

/// Statistics for a spawn session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnStats {
    pub session_name: String,
    pub tag: String,
    pub total_agents: usize,
    pub starting: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub created_at: String,
}

impl From<&SpawnSession> for SpawnStats {
    fn from(session: &SpawnSession) -> Self {
        Self {
            session_name: session.session_name.clone(),
            tag: session.tag.clone(),
            total_agents: session.agents.len(),
            starting: session.count_by_status(AgentStatus::Starting),
            running: session.count_by_status(AgentStatus::Running),
            completed: session.count_by_status(AgentStatus::Completed),
            failed: session.count_by_status(AgentStatus::Failed),
            created_at: session.created_at.clone(),
        }
    }
}

/// Get the spawn metadata directory
pub fn spawn_dir(project_root: Option<&PathBuf>) -> PathBuf {
    let root = project_root
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    root.join(".scud").join("spawn")
}

/// Get the path to a session's metadata file
pub fn session_file(project_root: Option<&PathBuf>, session_name: &str) -> PathBuf {
    spawn_dir(project_root).join(format!("{}.json", session_name))
}

/// Save spawn session metadata
pub fn save_session(project_root: Option<&PathBuf>, session: &SpawnSession) -> Result<()> {
    let dir = spawn_dir(project_root);
    fs::create_dir_all(&dir)?;

    let file = session_file(project_root, &session.session_name);
    let json = serde_json::to_string_pretty(session)?;
    fs::write(file, json)?;

    Ok(())
}

/// Load spawn session metadata
pub fn load_session(project_root: Option<&PathBuf>, session_name: &str) -> Result<SpawnSession> {
    let file = session_file(project_root, session_name);
    let json = fs::read_to_string(&file)?;
    let session: SpawnSession = serde_json::from_str(&json)?;
    Ok(session)
}

/// List all spawn sessions
pub fn list_sessions(project_root: Option<&PathBuf>) -> Result<Vec<String>> {
    let dir = spawn_dir(project_root);
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

/// Delete a spawn session metadata file
pub fn delete_session(project_root: Option<&PathBuf>, session_name: &str) -> Result<()> {
    let file = session_file(project_root, session_name);
    if file.exists() {
        fs::remove_file(file)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_session_new() {
        let session = SpawnSession::new("scud-test", "test-tag", "tmux", "/path/to/project");

        assert_eq!(session.session_name, "scud-test");
        assert_eq!(session.tag, "test-tag");
        assert_eq!(session.terminal, "tmux");
        assert!(session.agents.is_empty());
    }

    #[test]
    fn test_add_agent() {
        let mut session = SpawnSession::new("scud-test", "test-tag", "tmux", "/path/to/project");
        session.add_agent("auth:1", "Implement auth", "auth");

        assert_eq!(session.agents.len(), 1);
        assert_eq!(session.agents[0].task_id, "auth:1");
        assert_eq!(session.agents[0].window_name, "task-auth:1");
        assert_eq!(session.agents[0].status, AgentStatus::Starting);
    }

    #[test]
    fn test_update_agent_status() {
        let mut session = SpawnSession::new("scud-test", "test-tag", "tmux", "/path/to/project");
        session.add_agent("auth:1", "Implement auth", "auth");
        session.update_agent_status("auth:1", AgentStatus::Running);

        assert_eq!(session.agents[0].status, AgentStatus::Running);
    }

    #[test]
    fn test_spawn_stats() {
        let mut session = SpawnSession::new("scud-test", "test-tag", "tmux", "/path/to/project");
        session.add_agent("auth:1", "Task 1", "auth");
        session.add_agent("auth:2", "Task 2", "auth");
        session.add_agent("auth:3", "Task 3", "auth");

        session.update_agent_status("auth:1", AgentStatus::Running);
        session.update_agent_status("auth:2", AgentStatus::Completed);

        let stats = SpawnStats::from(&session);

        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.starting, 1); // auth:3
        assert_eq!(stats.running, 1); // auth:1
        assert_eq!(stats.completed, 1); // auth:2
        assert_eq!(stats.failed, 0);
    }
}
