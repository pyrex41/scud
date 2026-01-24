//! OpenCode Server integration for agent orchestration.
//!
//! Provides HTTP client and SSE event streaming for OpenCode Server mode,
//! enabling structured communication with agents instead of CLI subprocess spawning.
//!
//! Note: This module is a stub. Full implementation depends on OpenCode server.

use anyhow::Result;
use tokio::sync::mpsc;

use crate::extensions::runner::AgentResult;
use crate::models::task::Task;

/// Event from the OpenCode server
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent spawned
    Spawned { task_id: String },
    /// Agent completed
    Completed { task_id: String, success: bool },
    /// Agent output
    Output { task_id: String, text: String },
    /// Error
    Error { task_id: String, message: String },
}

/// Orchestrator for managing multiple agents via OpenCode server
pub struct AgentOrchestrator {
    #[allow(dead_code)]
    event_tx: mpsc::Sender<AgentEvent>,
    results: Vec<AgentResult>,
}

impl AgentOrchestrator {
    /// Create a new orchestrator
    pub async fn new(event_tx: mpsc::Sender<AgentEvent>) -> Result<Self> {
        Ok(Self {
            event_tx,
            results: Vec::new(),
        })
    }

    /// Spawn an agent for a task
    pub async fn spawn_agent(
        &mut self,
        task: &Task,
        _tag: &str,
        _prompt: &str,
        _model: Option<(&str, &str)>,
    ) -> Result<()> {
        // Stub implementation - just record that we would spawn
        self.results.push(AgentResult {
            task_id: task.id.clone(),
            success: false,
            exit_code: None,
            output: "OpenCode server not available".to_string(),
            duration_ms: 0,
        });
        Ok(())
    }

    /// Wait for all agents to complete
    pub async fn wait_all(&mut self) -> Vec<AgentResult> {
        std::mem::take(&mut self.results)
    }

    /// Cleanup resources
    pub async fn cleanup(&mut self) {
        // Nothing to cleanup in stub
    }
}

/// OpenCode manager singleton (stub)
pub struct OpenCodeManager;

impl OpenCodeManager {
    /// Ensure server is running
    pub async fn ensure_running(&self) -> Result<()> {
        anyhow::bail!("OpenCode server not implemented")
    }
}

/// Get the global manager instance
pub fn global_manager() -> &'static OpenCodeManager {
    static MANAGER: OpenCodeManager = OpenCodeManager;
    &MANAGER
}
