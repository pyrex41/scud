//! Direct API backend.
//!
//! Wraps the existing `llm::agent::run_agent_loop()` behind the
//! [`AgentBackend`] trait for in-process LLM execution.

use anyhow::Result;
use async_trait::async_trait;

use super::{AgentBackend, AgentHandle, AgentRequest};

#[cfg(feature = "direct-api")]
use {
    super::{AgentEvent, AgentResult, AgentStatus, ToolCallRecord},
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
};

/// Backend that calls LLM APIs directly (in-process).
///
/// Only available with the `direct-api` feature.
#[cfg(feature = "direct-api")]
pub struct DirectApiBackend {
    max_tokens: u32,
}

#[cfg(feature = "direct-api")]
impl DirectApiBackend {
    pub fn new() -> Self {
        Self { max_tokens: 16_000 }
    }
}

#[cfg(feature = "direct-api")]
#[async_trait]
impl AgentBackend for DirectApiBackend {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        use crate::commands::spawn::headless::events::{StreamEvent, StreamEventKind};
        use crate::llm::agent;
        use crate::llm::provider::AgentProvider;

        let (event_tx, rx) = mpsc::channel(1000);
        let cancel = CancellationToken::new();

        let provider = if let Some(ref p) = req.provider {
            AgentProvider::from_provider_str(p)?
        } else {
            AgentProvider::Anthropic
        };

        let model = req.model.clone();
        let max_tokens = self.max_tokens;
        let prompt = req.prompt.clone();
        let working_dir = req.working_dir.clone();
        let system_prompt = req.system_prompt.clone();

        // Bridge: run_agent_loop emits StreamEvent, we convert to AgentEvent
        let (stream_tx, mut stream_rx) = mpsc::channel::<StreamEvent>(1000);
        let cancel_clone = cancel.clone();

        // Spawn the agent loop
        let stream_tx_err = stream_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = agent::run_agent_loop(
                &prompt,
                system_prompt.as_deref(),
                &working_dir,
                model.as_deref(),
                max_tokens,
                stream_tx,
                &provider,
            )
            .await
            {
                let _ = stream_tx_err.send(StreamEvent::error(&e.to_string())).await;
                let _ = stream_tx_err.send(StreamEvent::complete(false)).await;
            }
        });

        // Bridge task: StreamEvent -> AgentEvent
        tokio::spawn(async move {
            let mut text_parts = Vec::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        let _ = event_tx.send(AgentEvent::Complete(AgentResult {
                            text: text_parts.join(""),
                            status: AgentStatus::Cancelled,
                            tool_calls,
                            usage: None,
                        })).await;
                        break;
                    }
                    event = stream_rx.recv() => {
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
                                        let status = if *success {
                                            AgentStatus::Completed
                                        } else {
                                            AgentStatus::Failed("Agent reported failure".into())
                                        };
                                        let _ = event_tx.send(AgentEvent::Complete(AgentResult {
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
                                if event_tx.send(agent_event).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                let _ = event_tx.send(AgentEvent::Complete(AgentResult {
                                    text: text_parts.join(""),
                                    status: AgentStatus::Completed,
                                    tool_calls,
                                    usage: None,
                                })).await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(AgentHandle { events: rx, cancel })
    }
}

// Stub when direct-api feature is not enabled
#[cfg(not(feature = "direct-api"))]
pub struct DirectApiBackend;

#[cfg(not(feature = "direct-api"))]
impl DirectApiBackend {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "direct-api"))]
#[async_trait]
impl AgentBackend for DirectApiBackend {
    async fn execute(&self, _req: AgentRequest) -> Result<AgentHandle> {
        anyhow::bail!("Direct API backend requires the 'direct-api' feature to be enabled")
    }
}
