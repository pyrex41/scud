//! Codergen handler — LLM task execution via AgentBackend.
//!
//! Expands `$goal` in the prompt, calls the backend, writes prompt.md + response.md.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::{Outcome, StageStatus};
use crate::attractor::run_directory::RunDirectory;
use crate::backend::{AgentBackend, AgentRequest, AgentStatus};

use super::Handler;

pub struct CodergenHandler {
    backend: Arc<dyn AgentBackend>,
}

impl CodergenHandler {
    /// Create a codergen handler with a real backend.
    pub fn new(backend: Arc<dyn AgentBackend>) -> Self {
        Self { backend }
    }

    /// Create a codergen handler with a simulated backend (for testing).
    pub fn simulated() -> Self {
        Self {
            backend: Arc::new(crate::backend::simulated::SimulatedBackend),
        }
    }
}

#[async_trait]
impl Handler for CodergenHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        context: &Context,
        graph: &PipelineGraph,
        run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Expand variables in prompt
        let prompt = expand_variables(&node.prompt, graph, context).await;

        // Write prompt to run directory
        run_dir.write_prompt(&node.id, &prompt)?;

        // Build request
        let request = AgentRequest {
            prompt,
            model: node.llm_model.clone(),
            provider: node.llm_provider.clone(),
            reasoning_effort: Some(node.reasoning_effort.clone()),
            working_dir: std::env::current_dir().unwrap_or_default(),
            timeout: node.timeout,
            ..Default::default()
        };

        // Execute via backend
        let handle = self.backend.execute(request).await?;
        let result = handle.result().await?;

        // Write response
        run_dir.write_response(&node.id, &result.text)?;

        // Write status
        let status_json = serde_json::json!({
            "node_id": node.id,
            "status": match &result.status {
                AgentStatus::Completed => "success",
                AgentStatus::Failed(_) => "failure",
                AgentStatus::Cancelled => "cancelled",
                AgentStatus::Timeout => "timeout",
            },
            "tool_calls": result.tool_calls.len(),
        });
        run_dir.write_status(&node.id, &status_json)?;

        // Build outcome
        let status = match result.status {
            AgentStatus::Completed => StageStatus::Success,
            AgentStatus::Failed(msg) => {
                return Ok(Outcome::failure(msg).with_response(result.text));
            }
            AgentStatus::Cancelled => StageStatus::Cancelled,
            AgentStatus::Timeout => StageStatus::Timeout,
        };

        Ok(Outcome {
            status,
            preferred_label: None,
            suggested_next: vec![],
            context_updates: std::collections::HashMap::new(),
            response_text: Some(result.text),
            summary: None,
        })
    }
}

/// Expand `$goal` and `$context.key` variables in a prompt string.
async fn expand_variables(
    prompt: &str,
    graph: &PipelineGraph,
    context: &Context,
) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attractor::context::Context;
    use crate::attractor::dot_parser::parse_dot;
    use crate::attractor::graph::PipelineGraph;
    use crate::attractor::run_directory::RunDirectory;

    #[tokio::test]
    async fn test_codergen_simulated() {
        let handler = CodergenHandler::simulated();
        let dir = tempfile::tempdir().unwrap();
        let run_dir = RunDirectory::create(dir.path(), "test").unwrap();

        let dot = parse_dot(r#"
            digraph test {
                graph [goal="Test goal"]
                start [shape=Mdiamond]
                task [shape=box, prompt="Do $goal"]
                finish [shape=Msquare]
                start -> task -> finish
            }
        "#).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let context = Context::new();
        let node = graph.node("task").unwrap();

        let outcome = handler.execute(node, &context, &graph, &run_dir).await.unwrap();
        assert!(outcome.status.is_success());
        assert!(outcome.response_text.is_some());

        // Check files were written
        let response = run_dir.read_response("task").unwrap();
        assert!(response.contains("Simulated"));
    }

    #[tokio::test]
    async fn test_expand_goal() {
        let dot = parse_dot(r#"
            digraph test {
                graph [goal="Build a widget"]
                start [shape=Mdiamond]
                finish [shape=Msquare]
                start -> finish
            }
        "#).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let context = Context::new();

        let result = expand_variables("Your goal is: $goal", &graph, &context).await;
        assert_eq!(result, "Your goal is: Build a widget");
    }

    #[tokio::test]
    async fn test_expand_context() {
        let dot = parse_dot(r#"
            digraph test {
                start [shape=Mdiamond]
                finish [shape=Msquare]
                start -> finish
            }
        "#).unwrap();
        let graph = PipelineGraph::from_dot(&dot).unwrap();
        let context = Context::new();
        context.set("name", serde_json::json!("Alice")).await;

        let result = expand_variables("Hello $context.name", &graph, &context).await;
        assert_eq!(result, "Hello Alice");
    }
}
