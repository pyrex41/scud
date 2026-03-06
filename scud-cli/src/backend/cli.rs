//! CLI subprocess backend.
//!
//! Wraps the existing headless CLI spawning infrastructure (Claude Code,
//! OpenCode, Cursor) behind the [`AgentBackend`] trait.

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{
    AgentBackend, AgentEvent, AgentHandle, AgentRequest, AgentResult, AgentStatus, ToolCallRecord,
};
use crate::commands::spawn::headless::events::StreamEventKind;
use crate::commands::spawn::headless::runner::create_runner;
use crate::commands::spawn::terminal::Harness;

/// Backend that spawns a CLI subprocess (Claude Code, OpenCode, Cursor).
pub struct CliBackend {
    harness: Harness,
}

impl CliBackend {
    /// Create a new CLI backend for the specified harness.
    pub fn new(harness: Harness) -> Result<Self> {
        // Validate that the binary exists early
        let _ = create_runner(harness.clone())?;
        Ok(Self { harness })
    }
}

#[async_trait]
impl AgentBackend for CliBackend {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        let runner = create_runner(self.harness.clone())?;
        let session = runner
            .start("agent", &req.prompt, &req.working_dir, req.model.as_deref())
            .await?;

        // Destructure session to own both the events receiver and the process handle
        let (mut stream_events, mut session_process) = session.into_parts();

        let (tx, rx) = mpsc::channel(1000);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        // Bridge StreamEvent -> AgentEvent
        tokio::spawn(async move {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        // Kill the subprocess on cancellation
                        let _ = session_process.kill();
                        let _ = tx.send(AgentEvent::Complete(AgentResult {
                            text: text_parts.join(""),
                            status: AgentStatus::Cancelled,
                            tool_calls,
                            usage: None,
                        })).await;
                        break;
                    }
                    event = stream_events.recv() => {
                        match event {
                            Some(stream_event) => {
                                let agent_event = match &stream_event.kind {
                                    StreamEventKind::TextDelta { text } => {
                                        text_parts.push(text.clone());
                                        AgentEvent::TextDelta(text.clone())
                                    }
                                    StreamEventKind::ToolStart { tool_name, tool_id, .. } => {
                                        tool_calls.push(ToolCallRecord {
                                            id: tool_id.clone(),
                                            name: tool_name.clone(),
                                            output: String::new(),
                                        });
                                        AgentEvent::ToolCallStart {
                                            id: tool_id.clone(),
                                            name: tool_name.clone(),
                                        }
                                    }
                                    StreamEventKind::ToolResult { tool_id, success, .. } => {
                                        if let Some(record) = tool_calls.iter_mut().find(|r| r.id == *tool_id) {
                                            record.output = if *success { "ok".into() } else { "error".into() };
                                        }
                                        AgentEvent::ToolCallEnd {
                                            id: tool_id.clone(),
                                            output: if *success { "ok".into() } else { "error".into() },
                                        }
                                    }
                                    StreamEventKind::Complete { success } => {
                                        let process_ok = session_process.wait().await.unwrap_or(false);
                                        let status = if *success && process_ok {
                                            AgentStatus::Completed
                                        } else {
                                            AgentStatus::Failed(if process_ok {
                                                "Agent reported failure".into()
                                            } else {
                                                "Agent process exited with non-zero status".into()
                                            })
                                        };
                                        let _ = tx.send(AgentEvent::Complete(AgentResult {
                                            text: text_parts.join(""),
                                            status,
                                            tool_calls: tool_calls.clone(),
                                            usage: None,
                                        })).await;
                                        break;
                                    }
                                    StreamEventKind::Error { message } => {
                                        AgentEvent::Error(message.clone())
                                    }
                                    StreamEventKind::SessionAssigned { .. } => continue,
                                };
                                if tx.send(agent_event).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                // Stream ended without explicit completion: use process exit status.
                                let process_ok = session_process.wait().await.unwrap_or(false);
                                let _ = tx.send(AgentEvent::Complete(AgentResult {
                                    text: text_parts.join(""),
                                    status: if process_ok {
                                        AgentStatus::Completed
                                    } else {
                                        AgentStatus::Failed(
                                            "Agent process exited without completion event".into(),
                                        )
                                    },
                                    tool_calls,
                                    usage: None,
                                })).await;
                                break;
                            }
                        }
                    }
                }
            }
            // Keep session_process alive until bridge completes
            drop(session_process);
        });

        Ok(AgentHandle { events: rx, cancel })
    }
}
