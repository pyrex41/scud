//! DirectApiRunner - HeadlessRunner implementation using direct API calls.
//!
//! Runs an agentic loop via the Anthropic Messages API or OpenAI-compatible
//! Chat Completions APIs instead of spawning an external CLI process.

use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;

use super::events::StreamEvent;
use super::runner::{BoxFuture, HeadlessRunner, SessionHandle};
use crate::commands::spawn::terminal::Harness;
use crate::llm::agent;
use crate::llm::provider::AgentProvider;

/// HeadlessRunner that calls LLM APIs directly.
pub struct DirectApiRunner {
    model: Option<String>,
    max_tokens: u32,
    provider: AgentProvider,
}

impl DirectApiRunner {
    pub fn new() -> Self {
        Self {
            model: None,
            max_tokens: 16_000,
            provider: AgentProvider::Anthropic,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_provider(mut self, provider: AgentProvider) -> Self {
        self.provider = provider;
        self
    }
}

impl HeadlessRunner for DirectApiRunner {
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>> {
        Box::pin(async move {
            let (tx, rx) = mpsc::channel(1000);

            let model = model.or(self.model.as_deref()).map(String::from);
            let max_tokens = self.max_tokens;
            let prompt = prompt.to_string();
            let working_dir = working_dir.to_path_buf();
            let task_id_owned = task_id.to_string();
            let provider = self.provider.clone();

            let tx_clone = tx.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = agent::run_agent_loop(
                    &prompt,
                    None,
                    &working_dir,
                    model.as_deref(),
                    max_tokens,
                    tx_clone,
                    &provider,
                )
                .await
                {
                    let _ = tx.send(StreamEvent::error(&e.to_string())).await;
                    let _ = tx.send(StreamEvent::complete(false)).await;
                }
            });

            Ok(SessionHandle::from_task(task_id_owned, rx, handle))
        })
    }

    fn interactive_command(&self, _session_id: &str) -> Vec<String> {
        vec![
            "echo".to_string(),
            "Direct API sessions cannot be resumed".to_string(),
        ]
    }

    fn harness(&self) -> Harness {
        Harness::DirectApi
    }
}
