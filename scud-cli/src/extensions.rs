//! Dynamic extension loading and execution.
//!
//! Provides infrastructure for loading and running extensions that can
//! extend SCUD functionality with custom tools and commands.
//!
//! Note: This module is a stub. Full implementation is planned.

pub mod runner {
    //! Extension runner for subprocess-based agent execution.

    use std::path::PathBuf;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use tokio::sync::mpsc;

    use crate::commands::spawn::terminal::Harness;

    /// Result from an agent execution
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentResult {
        /// Task ID that was executed
        pub task_id: String,
        /// Whether the agent succeeded
        pub success: bool,
        /// Exit code if available
        pub exit_code: Option<i32>,
        /// Output from the agent
        pub output: String,
        /// Duration in milliseconds
        pub duration_ms: u64,
    }

    impl AgentResult {
        /// Create a new successful result
        pub fn success(task_id: &str, duration_ms: u64) -> Self {
            Self {
                task_id: task_id.to_string(),
                success: true,
                exit_code: Some(0),
                output: String::new(),
                duration_ms,
            }
        }

        /// Create a new failed result
        pub fn failure(task_id: &str, exit_code: Option<i32>, output: &str) -> Self {
            Self {
                task_id: task_id.to_string(),
                success: false,
                exit_code,
                output: output.to_string(),
                duration_ms: 0,
            }
        }
    }

    /// Configuration for spawning an agent
    #[derive(Debug, Clone)]
    pub struct SpawnConfig {
        /// Task ID
        pub task_id: String,
        /// Working directory
        pub working_dir: PathBuf,
        /// Harness to use
        pub harness: Harness,
        /// Model override
        pub model: Option<String>,
        /// Prompt to send
        pub prompt: String,
    }

    /// Events from agent execution
    #[derive(Debug, Clone)]
    pub enum AgentEvent {
        /// Agent started
        Started { task_id: String },
        /// Agent output line
        Output { task_id: String, line: String },
        /// Agent completed
        Completed { result: AgentResult },
        /// Spawn failed
        SpawnFailed { task_id: String, error: String },
    }

    /// Handle for a spawned agent that can be awaited
    pub type AgentHandle = tokio::task::JoinHandle<AgentResult>;

    /// Spawn an agent with the given configuration
    ///
    /// This is a stub implementation that immediately sends a failure event.
    /// The full implementation would spawn an async subprocess.
    pub async fn spawn_agent(
        config: SpawnConfig,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<AgentHandle> {
        let task_id = config.task_id.clone();

        // Send started event
        let _ = event_tx
            .send(AgentEvent::Started {
                task_id: task_id.clone(),
            })
            .await;

        // Spawn a task that immediately completes with failure (stub)
        let handle = tokio::spawn(async move {
            // Stub: Immediately return failure
            let result = AgentResult::failure(
                &task_id,
                None,
                "Extension runner not implemented",
            );

            let _ = event_tx
                .send(AgentEvent::Completed {
                    result: result.clone(),
                })
                .await;

            result
        });

        Ok(handle)
    }
}

/// Check if extensions are available
pub fn extensions_available() -> bool {
    false // Stub - extensions not yet implemented
}
