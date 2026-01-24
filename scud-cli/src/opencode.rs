//! OpenCode Server integration stub
//!
//! Provides HTTP client and SSE event streaming for OpenCode Server mode.
//! Currently a stub implementation.

use anyhow::Result;
use tokio::sync::mpsc;

use crate::extensions::runner::AgentResult;
use crate::models::task::Task;

/// Events emitted by the orchestrator
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    AgentStarted { task_id: String },
    AgentCompleted { task_id: String, success: bool },
    AgentOutput { task_id: String, text: String },
}

/// Agent orchestrator for OpenCode Server mode (stub implementation)
pub struct AgentOrchestrator {
    _event_tx: mpsc::Sender<OrchestratorEvent>,
}

impl AgentOrchestrator {
    /// Create a new orchestrator
    pub async fn new(event_tx: mpsc::Sender<OrchestratorEvent>) -> Result<Self> {
        Ok(Self { _event_tx: event_tx })
    }

    /// Spawn an agent for a task
    pub async fn spawn_agent(
        &mut self,
        _task: &Task,
        _tag: &str,
        _prompt: &str,
        _model: Option<(&str, &str)>,
    ) -> Result<()> {
        anyhow::bail!("OpenCode Server mode is not yet implemented. Use tmux mode (--swarm-mode tmux) instead.")
    }

    /// Wait for all agents to complete
    pub async fn wait_all(&self) -> Vec<AgentResult> {
        Vec::new()
    }

    /// Cleanup resources
    pub async fn cleanup(&self) {}
}

/// Get the global manager (stub)
pub fn global_manager() -> &'static str {
    "stub"
}

/// OpenCode manager (stub)
pub struct OpenCodeManager;

impl OpenCodeManager {
    pub async fn ensure_running(&self) -> Result<()> {
        Ok(())
    }
}
