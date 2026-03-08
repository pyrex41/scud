//! Rho handler — executes tasks via rho-cli subprocess.
//!
//! Spawns `rho-cli --output-format stream-json` with the node prompt,
//! parses the streaming JSON events, and returns an Outcome with the
//! collected response text.

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, trace, warn};

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::{Outcome, StageStatus};
use crate::attractor::run_directory::RunDirectory;
use crate::commands::spawn::terminal::{find_harness_binary, Harness};

use super::Handler;

pub struct RhoHandler;

#[async_trait]
impl Handler for RhoHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        context: &Context,
        graph: &PipelineGraph,
        run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Resolve the rho-cli binary
        let binary_path = find_harness_binary(Harness::Rho)
            .context("Could not find rho-cli binary")?;

        // Expand variables in the prompt
        let prompt = expand_prompt(&node.prompt, graph, context).await;

        if prompt.is_empty() {
            return Ok(Outcome::failure("No prompt specified for rho handler"));
        }

        // Write prompt to run directory
        run_dir.write_prompt(&node.id, &prompt)?;

        // Determine working directory
        let working_dir = std::env::current_dir().unwrap_or_default();

        // Build the rho-cli command
        let mut cmd = Command::new(binary_path);
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("-p").arg(&prompt);
        cmd.arg("-C").arg(&working_dir);

        // Optional model override from node attributes
        let model = node
            .extra_attrs
            .get("rho_model")
            .map(|v| v.as_str())
            .or_else(|| node.llm_model.clone());
        if let Some(ref m) = model {
            cmd.arg("--model").arg(m);
        }

        cmd.current_dir(&working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        debug!(node_id = %node.id, "spawning rho-cli");

        let mut child = cmd.spawn().context("Failed to spawn rho-cli")?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to capture rho-cli stdout")?;

        // Parse streaming JSON output
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut response_text = String::new();
        let mut success = true;
        let mut error_message: Option<String> = None;
        let mut tool_count: u32 = 0;

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            let json: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    trace!(node_id = %node.id, "rho: unparsed line: {}", if line.len() > 200 { &line[..200] } else { &line });
                    continue;
                }
            };

            let event_type = match json.get("type").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => continue,
            };

            match event_type {
                "text_delta" => {
                    if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                        response_text.push_str(text);
                    }
                }
                "tool_start" => {
                    tool_count += 1;
                    let tool_name = json
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    trace!(node_id = %node.id, tool = tool_name, "rho tool start");
                }
                "tool_result" => {
                    let tool_success = json
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    if !tool_success {
                        debug!(node_id = %node.id, "rho tool reported failure");
                    }
                }
                "complete" => {
                    success = json
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                }
                "error" => {
                    let msg = json
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown rho error")
                        .to_string();
                    warn!(node_id = %node.id, error = %msg, "rho error event");
                    error_message = Some(msg);
                    success = false;
                }
                "session" => {
                    if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                        trace!(node_id = %node.id, session_id = sid, "rho session assigned");
                    }
                }
                _ => {
                    trace!(node_id = %node.id, event_type, "rho: unknown event type");
                }
            }
        }

        // Wait for the child process to exit
        let exit_status = child.wait().await.context("Failed to wait for rho-cli")?;

        // If process exited with non-zero and we haven't already flagged failure
        if !exit_status.success() && success {
            success = false;
            if error_message.is_none() {
                error_message = Some(format!(
                    "rho-cli exited with code {:?}",
                    exit_status.code()
                ));
            }
        }

        // Write response to run directory
        run_dir.write_response(&node.id, &response_text)?;

        // Write status
        let status_json = serde_json::json!({
            "node_id": node.id,
            "status": if success { "success" } else { "failure" },
            "tool_calls": tool_count,
        });
        run_dir.write_status(&node.id, &status_json)?;

        // Build context updates
        let mut updates = HashMap::new();
        updates.insert(
            format!("{}.response", node.id),
            serde_json::json!(response_text),
        );
        updates.insert(
            format!("{}.tool_calls", node.id),
            serde_json::json!(tool_count),
        );

        if success {
            Ok(Outcome {
                status: StageStatus::Success,
                preferred_label: None,
                suggested_next: vec![],
                context_updates: updates,
                response_text: Some(response_text),
                summary: None,
            })
        } else {
            let msg = error_message.unwrap_or_else(|| "rho-cli execution failed".into());
            Ok(Outcome::failure(&msg)
                .with_response(response_text)
                .with_context(updates))
        }
    }
}

/// Expand `$goal` and `$context.key` variables in a prompt string.
///
/// Follows the same variable expansion pattern as the codergen handler.
async fn expand_prompt(prompt: &str, graph: &PipelineGraph, context: &Context) -> String {
    let mut result = prompt.to_string();

    // Replace $goal with graph-level goal
    if let Some(ref goal) = graph.graph_attrs.goal {
        result = result.replace("$goal", goal);
    }

    // Replace $context.key patterns
    let snapshot = context.snapshot().await;
    for (key, value) in &snapshot {
        let pattern = format!("$context.{}", key);
        let replacement = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&pattern, &replacement);
    }

    result
}
