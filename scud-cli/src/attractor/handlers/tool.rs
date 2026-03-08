//! Tool handler — executes shell commands.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct ToolHandler;

#[async_trait]
impl Handler for ToolHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        _context: &Context,
        _graph: &PipelineGraph,
        run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Get the tool command from node attributes
        let command = node
            .extra_attrs
            .get("tool_command")
            .map(|v| v.as_str())
            .or_else(|| node.extra_attrs.get("command").map(|v| v.as_str()))
            .unwrap_or_default();

        if command.is_empty() {
            return Ok(Outcome::failure("No tool_command attribute specified"));
        }

        // Execute the command
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Write output to run directory
        let combined = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);
        run_dir.write_response(&node.id, &combined)?;

        let mut updates = HashMap::new();
        updates.insert(format!("{}.stdout", node.id), serde_json::json!(stdout));
        updates.insert(
            format!("{}.exit_code", node.id),
            serde_json::json!(output.status.code()),
        );

        if output.status.success() {
            Ok(Outcome::success()
                .with_response(stdout)
                .with_context(updates))
        } else {
            Ok(Outcome::failure(format!(
                "Command failed with exit code {:?}: {}",
                output.status.code(),
                stderr.lines().take(5).collect::<Vec<_>>().join("\n")
            ))
            .with_context(updates))
        }
    }
}
