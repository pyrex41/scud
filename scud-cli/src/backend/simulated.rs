//! Simulated backend for testing and dry-runs.
//!
//! Returns a canned response without making any LLM API calls.
//! Useful for Attractor pipeline testing and `--simulated` mode.

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{AgentBackend, AgentEvent, AgentHandle, AgentRequest, AgentResult, AgentStatus};

/// A backend that returns simulated responses without calling any LLM.
pub struct SimulatedBackend;

#[async_trait]
impl AgentBackend for SimulatedBackend {
    async fn execute(&self, req: AgentRequest) -> Result<AgentHandle> {
        let (tx, rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();

        let prompt_preview = if req.prompt.len() > 100 {
            format!("{}...", &req.prompt[..97])
        } else {
            req.prompt.clone()
        };

        let response_text = format!("[Simulated] Response for: {}", prompt_preview);

        tokio::spawn(async move {
            let _ = tx
                .send(AgentEvent::TextDelta(response_text.clone()))
                .await;
            let _ = tx
                .send(AgentEvent::Complete(AgentResult {
                    text: response_text,
                    status: AgentStatus::Completed,
                    tool_calls: vec![],
                    usage: None,
                }))
                .await;
        });

        Ok(AgentHandle { events: rx, cancel })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simulated_response_contains_prompt() {
        let backend = SimulatedBackend;
        let req = AgentRequest {
            prompt: "Write a test".into(),
            ..Default::default()
        };
        let handle = backend.execute(req).await.unwrap();
        let result = handle.result().await.unwrap();
        assert!(result.text.contains("Write a test"));
        assert!(result.text.starts_with("[Simulated]"));
    }

    #[tokio::test]
    async fn test_simulated_truncates_long_prompt() {
        let backend = SimulatedBackend;
        let long_prompt = "x".repeat(200);
        let req = AgentRequest {
            prompt: long_prompt,
            ..Default::default()
        };
        let handle = backend.execute(req).await.unwrap();
        let result = handle.result().await.unwrap();
        assert!(result.text.contains("..."));
        assert!(result.text.len() < 200);
    }
}
