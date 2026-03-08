//! Unified execution backend for AI agent tasks.
//!
//! Provides the [`AgentBackend`] trait that abstracts over different execution modes:
//! - Direct API calls to LLM providers
//! - CLI subprocess spawning (Claude Code, OpenCode, Cursor)
//! - Simulated execution for testing and dry-runs
//!
//! All callers interact through the same interface regardless of how the
//! underlying LLM execution happens.

pub mod cli;
pub mod direct;
pub mod simulated;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::commands::spawn::terminal::Harness;

/// A backend that can execute an AI agent task.
///
/// Callers don't know or care whether this is a raw API call,
/// a headless CLI process, or something else.
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Execute a prompt with optional tool access.
    ///
    /// Returns a handle for streaming events and awaiting the final result.
    async fn execute(&self, request: AgentRequest) -> Result<AgentHandle>;
}

/// Request to execute an agent task.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// The prompt to send to the agent.
    pub prompt: String,
    /// Optional system prompt override.
    pub system_prompt: Option<String>,
    /// Working directory for tool execution.
    pub working_dir: PathBuf,
    /// Model to use (provider-specific format).
    pub model: Option<String>,
    /// LLM provider name.
    pub provider: Option<String>,
    /// Maximum agentic turns before stopping.
    pub max_turns: Option<usize>,
    /// Overall timeout for the execution.
    pub timeout: Option<Duration>,
    /// Reasoning effort level (e.g., "high", "medium", "low").
    pub reasoning_effort: Option<String>,
    /// If set, only these tools are available to the agent.
    pub allowed_tools: Option<Vec<String>>,
}

impl Default for AgentRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system_prompt: None,
            working_dir: std::env::current_dir().unwrap_or_default(),
            model: None,
            provider: None,
            max_turns: None,
            timeout: None,
            reasoning_effort: None,
            allowed_tools: None,
        }
    }
}

/// Handle to a running agent execution.
///
/// Provides access to streaming events and cancellation.
pub struct AgentHandle {
    /// Receiver for streaming events during execution.
    pub events: mpsc::Receiver<AgentEvent>,
    /// Token to cancel the execution.
    pub cancel: CancellationToken,
}

impl AgentHandle {
    /// Consume all events and return the final result.
    ///
    /// This drains the event stream, collecting text deltas and tool calls,
    /// until a [`AgentEvent::Complete`] event is received.
    pub async fn result(mut self) -> Result<AgentResult> {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut status = AgentStatus::Completed;
        let usage = None;

        while let Some(event) = self.events.recv().await {
            match event {
                AgentEvent::TextDelta(delta) => text_parts.push(delta),
                AgentEvent::TextComplete(text) => {
                    text_parts.clear();
                    text_parts.push(text);
                }
                AgentEvent::ToolCallStart { id, name } => {
                    tool_calls.push(ToolCallRecord {
                        id,
                        name,
                        output: String::new(),
                    });
                }
                AgentEvent::ToolCallEnd { id, output } => {
                    if let Some(record) = tool_calls.iter_mut().find(|r| r.id == id) {
                        record.output = output;
                    }
                }
                AgentEvent::Complete(result) => return Ok(result),
                AgentEvent::Error(msg) => {
                    status = AgentStatus::Failed(msg);
                    break;
                }
                AgentEvent::ThinkingDelta(_) => {}
            }
        }

        Ok(AgentResult {
            text: text_parts.join(""),
            status,
            tool_calls,
            usage,
        })
    }
}

/// Result of an agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Final response text (concatenated from all text deltas).
    pub text: String,
    /// Execution status.
    pub status: AgentStatus,
    /// Audit trail of tool calls made during execution.
    pub tool_calls: Vec<ToolCallRecord>,
    /// Token usage statistics, if available.
    pub usage: Option<TokenUsage>,
}

/// Status of an agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Completed successfully.
    Completed,
    /// Failed with an error message.
    Failed(String),
    /// Cancelled by the caller.
    Cancelled,
    /// Timed out.
    Timeout,
}

impl AgentStatus {
    /// Returns true if the execution completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, AgentStatus::Completed)
    }
}

/// Record of a single tool call during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Unique ID of the tool call.
    pub id: String,
    /// Tool name (e.g., "Read", "Bash", "Edit").
    pub name: String,
    /// Tool output/result.
    pub output: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Events emitted during agent execution.
///
/// This is a superset of the existing [`StreamEventKind`] types,
/// providing a unified event model for all backends.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Incremental text output from the agent.
    TextDelta(String),
    /// Complete text response (replaces accumulated deltas).
    TextComplete(String),
    /// A tool call has started.
    ToolCallStart { id: String, name: String },
    /// A tool call has completed.
    ToolCallEnd { id: String, output: String },
    /// Thinking/reasoning text delta.
    ThinkingDelta(String),
    /// An error occurred.
    Error(String),
    /// Execution has completed with a final result.
    Complete(AgentResult),
}

/// Create a backend from a harness specification.
///
/// This is the main factory function for creating backends.
pub fn create_backend(harness: &Harness) -> Result<Box<dyn AgentBackend>> {
    match harness {
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => Ok(Box::new(direct::DirectApiBackend::new())),
        _ => Ok(Box::new(cli::CliBackend::new(harness.clone())?)),
    }
}

/// Create a simulated backend for testing and dry-runs.
pub fn create_simulated_backend() -> Box<dyn AgentBackend> {
    Box::new(simulated::SimulatedBackend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_request_default() {
        let req = AgentRequest::default();
        assert!(req.prompt.is_empty());
        assert!(req.model.is_none());
        assert!(req.timeout.is_none());
    }

    #[tokio::test]
    async fn test_agent_status_is_success() {
        assert!(AgentStatus::Completed.is_success());
        assert!(!AgentStatus::Failed("err".into()).is_success());
        assert!(!AgentStatus::Cancelled.is_success());
        assert!(!AgentStatus::Timeout.is_success());
    }

    #[tokio::test]
    async fn test_simulated_backend() {
        let backend = create_simulated_backend();
        let req = AgentRequest {
            prompt: "Hello world".into(),
            ..Default::default()
        };
        let handle = backend.execute(req).await.unwrap();
        let result = handle.result().await.unwrap();
        assert!(result.status.is_success());
        assert!(result.text.contains("Simulated"));
    }
}
