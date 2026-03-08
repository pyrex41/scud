//! Agentic loop engine for direct API calls.
//!
//! Sends messages to LLM APIs with tool definitions, executes tool_use
//! blocks locally, returns results, and repeats until the model finishes.
//! Emits StreamEvents for TUI/headless consumption.

use anyhow::Result;
use reqwest::Client;
use std::path::Path;
use tokio::sync::mpsc;
use tracing::debug;

use super::provider::{AgentContentBlock, AgentMessage, AgentProvider, AgentRole};
use super::tools;
use crate::commands::spawn::headless::events::{StreamEvent, StreamEventKind};

const MAX_TURNS: usize = 200;

/// Run the agentic loop.
///
/// Sends prompt to the configured provider API with tool definitions, executes
/// tool calls locally, sends results back, repeats until the model finishes or
/// MAX_TURNS is reached. Emits StreamEvents on the channel for TUI consumption.
pub async fn run_agent_loop(
    prompt: &str,
    system_prompt: Option<&str>,
    working_dir: &Path,
    model: Option<&str>,
    max_tokens: u32,
    event_tx: mpsc::Sender<StreamEvent>,
    provider: &AgentProvider,
    allowed_tools: Option<&[String]>,
) -> Result<()> {
    let credential = provider.resolve_credential()?;
    let client = Client::new();
    let model = provider
        .normalize_model(model.unwrap_or(provider.default_model()))
        .to_string();

    let mut tool_defs_raw = tools::tool_definitions();
    if let Some(allowed) = allowed_tools {
        tool_defs_raw.retain(|td| allowed.contains(&td.name));
    }
    let tool_defs = provider.format_tool_definitions(&tool_defs_raw);

    let mut messages = vec![AgentMessage {
        role: AgentRole::User,
        content: vec![AgentContentBlock::Text {
            text: prompt.to_string(),
        }],
    }];

    for turn in 0..MAX_TURNS {
        debug!(turn, "sending API request");

        let response = provider
            .send_request(
                &client,
                &credential,
                &model,
                max_tokens,
                system_prompt,
                &messages,
                &tool_defs,
            )
            .await?;

        // Process response content blocks
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                AgentContentBlock::Text { text } => {
                    let _ = event_tx.send(StreamEvent::text_delta(text)).await;
                }
                AgentContentBlock::ToolUse { id, name, input } => {
                    let summary = tools::summarize_input(input);
                    let _ = event_tx
                        .send(StreamEvent::tool_start(name, id, &summary))
                        .await;
                    tool_calls.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        // Add assistant response to conversation
        messages.push(AgentMessage {
            role: AgentRole::Assistant,
            content: response.content,
        });

        // If no tool calls, model is done
        if tool_calls.is_empty() {
            debug!(turn, "no tool calls - agent finished");
            break;
        }

        debug!(turn, tool_count = tool_calls.len(), "executing tool calls");

        // Execute tool calls and collect results
        let mut tool_results = Vec::new();
        for (id, name, input) in &tool_calls {
            let result = tools::execute_tool(name, input, working_dir).await;

            let _ = event_tx
                .send(StreamEvent::new(StreamEventKind::ToolResult {
                    tool_name: name.clone(),
                    tool_id: id.clone(),
                    success: !result.is_error,
                }))
                .await;

            tool_results.push(AgentContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: result.content,
                is_error: result.is_error,
            });
        }

        // Add tool results as user message
        messages.push(AgentMessage {
            role: AgentRole::User,
            content: tool_results,
        });
    }

    let _ = event_tx.send(StreamEvent::complete(true)).await;
    Ok(())
}
