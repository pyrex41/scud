//! Agentic loop engine for direct Anthropic API calls.
//!
//! Sends messages to the Anthropic API with tool definitions, executes tool_use
//! blocks locally, returns results, and repeats until the model finishes.
//! Emits StreamEvents for TUI/headless consumption.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;
use tracing::debug;

use super::oauth::{ApiCredential, resolve_anthropic_credential};
use super::tools;
use crate::commands::spawn::headless::events::{StreamEvent, StreamEventKind};

const CLAUDE_CODE_VERSION: &str = "2.1.2";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const MAX_TURNS: usize = 200;

/// Anthropic Messages API request
#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<Message>,
    tools: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

#[derive(Debug, Deserialize)]
struct AssistantResponse {
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

/// Run the agentic loop.
///
/// Sends prompt to Anthropic API with tool definitions, executes tool calls
/// locally, sends results back, repeats until the model finishes or MAX_TURNS
/// is reached. Emits StreamEvents on the channel for TUI consumption.
pub async fn run_agent_loop(
    prompt: &str,
    system_prompt: Option<&str>,
    working_dir: &Path,
    model: Option<&str>,
    max_tokens: u32,
    event_tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
    let credential = resolve_anthropic_credential()?;
    let client = Client::new();
    let model = model.unwrap_or(DEFAULT_MODEL).to_string();

    let system = build_system_prompt(&credential, system_prompt);
    let tool_defs = tools::tool_definitions_json();

    let mut messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: prompt.to_string(),
        }],
    }];

    for turn in 0..MAX_TURNS {
        debug!(turn, "sending API request");

        let response = send_request(
            &client,
            &credential,
            &model,
            max_tokens,
            &system,
            &messages,
            &tool_defs,
        )
        .await?;

        // Process response content blocks
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    let _ = event_tx.send(StreamEvent::text_delta(text)).await;
                }
                ContentBlock::ToolUse { id, name, input } => {
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
        messages.push(Message {
            role: "assistant".to_string(),
            content: response.content.clone(),
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

            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: result.content,
                is_error: result.is_error,
            });
        }

        // Add tool results as user message
        messages.push(Message {
            role: "user".to_string(),
            content: tool_results,
        });
    }

    let _ = event_tx.send(StreamEvent::complete(true)).await;
    Ok(())
}

fn build_system_prompt(credential: &ApiCredential, custom: Option<&str>) -> Vec<SystemBlock> {
    let mut blocks = Vec::new();

    // Claude Code identity prefix is required for OAuth tokens
    if matches!(credential, ApiCredential::OAuth(_)) {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: "You are Claude Code, Anthropic's official CLI for Claude.".to_string(),
        });
    }

    if let Some(prompt) = custom {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: prompt.to_string(),
        });
    }

    blocks
}

/// Build and send the HTTP request to Anthropic API
async fn send_request(
    client: &Client,
    credential: &ApiCredential,
    model: &str,
    max_tokens: u32,
    system: &[SystemBlock],
    messages: &[Message],
    tools: &[serde_json::Value],
) -> Result<AssistantResponse> {
    let body = MessagesRequest {
        model: model.to_string(),
        max_tokens,
        system: system.to_vec(),
        messages: messages.to_vec(),
        tools: tools.to_vec(),
    };

    let mut req = client
        .post(ANTHROPIC_API_URL)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");

    match credential {
        ApiCredential::OAuth(token) => {
            req = req
                .bearer_auth(token)
                .header(
                    "anthropic-beta",
                    "claude-code-20250219,oauth-2025-04-20",
                )
                .header(
                    "user-agent",
                    format!("claude-cli/{} (external, cli)", CLAUDE_CODE_VERSION),
                )
                .header("x-app", "cli");
        }
        ApiCredential::ApiKey(key) => {
            req = req.header("x-api-key", key);
        }
    }

    let response = req.json(&body).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error ({}): {}", status, text);
    }

    let resp: AssistantResponse = response.json().await?;
    Ok(resp)
}
