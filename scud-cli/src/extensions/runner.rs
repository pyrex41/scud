//! Extension runner for spawning agents
//!
//! Provides types and functions for spawning agents in extension mode.
//! Currently a stub implementation.

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::commands::spawn::terminal::Harness;

/// Result of agent execution
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Task ID that was executed
    pub task_id: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Configuration for spawning an agent
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Task ID being executed
    pub task_id: String,
    /// Prompt to send to the agent
    pub prompt: String,
    /// Working directory for the agent
    pub working_dir: PathBuf,
    /// Harness to use
    pub harness: Harness,
    /// Model to use
    pub model: Option<String>,
}

/// Events emitted by agents during execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent started
    Started { task_id: String },
    /// Agent produced output
    Output { task_id: String, line: String },
    /// Agent completed
    Completed { result: AgentResult },
    /// Agent failed to spawn
    SpawnFailed { task_id: String, error: String },
}

/// Spawn an agent (stub implementation)
///
/// Returns a JoinHandle that resolves when the agent completes.
pub async fn spawn_agent(
    config: SpawnConfig,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<JoinHandle<()>> {
    // Send spawn failed event since this is a stub
    let task_id = config.task_id.clone();
    let _ = event_tx.send(AgentEvent::SpawnFailed {
        task_id: task_id.clone(),
        error: "Extension runner is not yet fully implemented. Use tmux mode instead.".to_string(),
    }).await;

    // Return a JoinHandle that completes immediately
    Ok(tokio::spawn(async move {
        // Agent would run here
    }))
}
