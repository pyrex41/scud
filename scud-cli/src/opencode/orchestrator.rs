//! Agent orchestration via OpenCode Server
//!
//! Manages multiple concurrent agent sessions through the OpenCode Server,
//! translating server events to the existing AgentEvent system.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::extensions::runner::{AgentEvent, AgentResult};
use crate::models::Task;

use super::events::OpenCodeEvent;
use super::manager::{global_manager, OpenCodeManager};

/// Handle to a running agent session
#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub task_id: String,
    pub session_id: String,
    pub tag: String,
}

/// Orchestrator for running agents via OpenCode Server
pub struct AgentOrchestrator {
    manager: Arc<OpenCodeManager>,
    /// Map from session_id to AgentHandle
    sessions: HashMap<String, AgentHandle>,
    /// Map from task_id to session_id
    task_sessions: HashMap<String, String>,
    /// Event channel for broadcasting to consumers
    event_tx: mpsc::Sender<AgentEvent>,
    /// Start times for duration tracking
    start_times: HashMap<String, std::time::Instant>,
}

impl AgentOrchestrator {
    /// Create a new orchestrator
    pub async fn new(event_tx: mpsc::Sender<AgentEvent>) -> Result<Self> {
        let manager = global_manager();
        manager.ensure_running().await?;

        Ok(Self {
            manager,
            sessions: HashMap::new(),
            task_sessions: HashMap::new(),
            event_tx,
            start_times: HashMap::new(),
        })
    }

    /// Create with a custom manager (for testing)
    pub fn with_manager(manager: Arc<OpenCodeManager>, event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            manager,
            sessions: HashMap::new(),
            task_sessions: HashMap::new(),
            event_tx,
            start_times: HashMap::new(),
        }
    }

    /// Spawn an agent for a task
    pub async fn spawn_agent(
        &mut self,
        task: &Task,
        tag: &str,
        prompt: &str,
        model: Option<(&str, &str)>,
    ) -> Result<AgentHandle> {
        let client = self.manager.client();

        // Create session with task title
        let session = client
            .create_session(&format!("[{}] {}", task.id, task.title))
            .await?;

        // Send the prompt
        client.send_message(&session.id, prompt, model).await?;

        // Create handle
        let handle = AgentHandle {
            task_id: task.id.clone(),
            session_id: session.id.clone(),
            tag: tag.to_string(),
        };

        // Track session
        self.sessions.insert(session.id.clone(), handle.clone());
        self.task_sessions
            .insert(task.id.clone(), session.id.clone());
        self.start_times
            .insert(task.id.clone(), std::time::Instant::now());

        // Emit started event
        let _ = self
            .event_tx
            .send(AgentEvent::Started {
                task_id: task.id.clone(),
            })
            .await;

        Ok(handle)
    }

    /// Cancel a running agent
    pub async fn cancel_agent(&mut self, task_id: &str) -> Result<()> {
        if let Some(session_id) = self.task_sessions.get(task_id) {
            self.manager.client().abort_session(session_id).await?;
        }
        Ok(())
    }

    /// Get all active task IDs
    pub fn active_tasks(&self) -> Vec<String> {
        self.task_sessions.keys().cloned().collect()
    }

    /// Check if a task has an active session
    pub fn is_task_active(&self, task_id: &str) -> bool {
        self.task_sessions.contains_key(task_id)
    }

    /// Get the number of active sessions
    pub fn active_count(&self) -> usize {
        self.task_sessions.len()
    }

    /// Process an OpenCode event and emit corresponding AgentEvent
    pub async fn process_event(&mut self, event: OpenCodeEvent) -> Option<AgentEvent> {
        let session_id = event.session_id()?;
        let handle = self.sessions.get(session_id)?;
        let task_id = handle.task_id.clone();

        match event {
            OpenCodeEvent::TextDelta { text, .. } => {
                let agent_event = AgentEvent::Output {
                    task_id: task_id.clone(),
                    line: text,
                };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            OpenCodeEvent::ToolStart {
                tool_name, input, ..
            } => {
                // Format tool call as output line
                let input_summary = summarize_tool_input(&input);
                let line = format!(">> {} {}", tool_name, input_summary);
                let agent_event = AgentEvent::Output {
                    task_id: task_id.clone(),
                    line,
                };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            OpenCodeEvent::ToolResult {
                tool_name, success, ..
            } => {
                let status = if success { "ok" } else { "failed" };
                let line = format!("<< {} {}", tool_name, status);
                let agent_event = AgentEvent::Output {
                    task_id: task_id.clone(),
                    line,
                };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            OpenCodeEvent::MessageComplete { success, .. } => {
                // Calculate duration
                let duration_ms = self
                    .start_times
                    .get(&task_id)
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                let result = AgentResult {
                    task_id: task_id.clone(),
                    success,
                    exit_code: if success { Some(0) } else { Some(1) },
                    output: String::new(), // Output was streamed
                    duration_ms,
                };

                // Clean up tracking
                if let Some(session_id) = self.task_sessions.remove(&task_id) {
                    self.sessions.remove(&session_id);
                }
                self.start_times.remove(&task_id);

                let agent_event = AgentEvent::Completed { result };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            OpenCodeEvent::SessionError { error, .. } => {
                let duration_ms = self
                    .start_times
                    .get(&task_id)
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                let result = AgentResult {
                    task_id: task_id.clone(),
                    success: false,
                    exit_code: Some(1),
                    output: error,
                    duration_ms,
                };

                // Clean up tracking
                if let Some(session_id) = self.task_sessions.remove(&task_id) {
                    self.sessions.remove(&session_id);
                }
                self.start_times.remove(&task_id);

                let agent_event = AgentEvent::Completed { result };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            _ => None,
        }
    }

    /// Wait for all agents to complete
    pub async fn wait_all(&mut self) -> Vec<AgentResult> {
        let mut results = Vec::new();
        let mut event_stream = match self.manager.event_stream().await {
            Ok(s) => s,
            Err(_) => return results,
        };

        while !self.task_sessions.is_empty() {
            if let Some(event) = event_stream.recv().await {
                if let Some(AgentEvent::Completed { result }) = self.process_event(event).await {
                    results.push(result);
                }
            }
        }

        results
    }

    /// Clean up all sessions
    pub async fn cleanup(&mut self) {
        let client = self.manager.client();
        for session_id in self.sessions.keys() {
            let _ = client.delete_session(session_id).await;
        }
        self.sessions.clear();
        self.task_sessions.clear();
        self.start_times.clear();
    }
}

/// Summarize tool input for display (avoid massive JSON dumps)
fn summarize_tool_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(obj) => {
            // Show first few keys
            let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).take(3).collect();
            if keys.is_empty() {
                "{}".to_string()
            } else if keys.len() < obj.len() {
                format!("{{{},...}}", keys.join(", "))
            } else {
                format!("{{{}}}", keys.join(", "))
            }
        }
        serde_json::Value::String(s) => {
            if s.len() > 50 {
                format!("\"{}...\"", &s[..47])
            } else {
                format!("\"{}\"", s)
            }
        }
        serde_json::Value::Null => "".to_string(),
        other => {
            let s = other.to_string();
            if s.len() > 50 {
                format!("{}...", &s[..47])
            } else {
                s
            }
        }
    }
}

/// Execute a wave of agents via OpenCode Server
pub async fn execute_wave_server(
    tasks: &[(Task, String)], // (task, tag) pairs
    working_dir: &Path,
    model: Option<(&str, &str)>,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<Vec<AgentResult>> {
    let mut orchestrator = AgentOrchestrator::new(event_tx).await?;

    // Spawn all agents
    for (task, tag) in tasks {
        let prompt = generate_prompt(task, tag, working_dir);
        if let Err(e) = orchestrator.spawn_agent(task, tag, &prompt, model).await {
            eprintln!("Failed to spawn agent for {}: {}", task.id, e);
        }
    }

    // Wait for all to complete
    let results = orchestrator.wait_all().await;

    // Cleanup
    orchestrator.cleanup().await;

    Ok(results)
}

/// Generate prompt for a task
fn generate_prompt(task: &Task, tag: &str, working_dir: &Path) -> String {
    let details = task
        .details
        .as_ref()
        .map(|d| format!("\n\n## Details\n\n{}", d))
        .unwrap_or_default();

    let test_strategy = task
        .test_strategy
        .as_ref()
        .map(|t| format!("\n\n## Test Strategy\n\n{}", t))
        .unwrap_or_default();

    format!(
        r#"You are working on task [{id}] in phase "{tag}".

## Task: {title}

{description}{details}{test_strategy}

## Instructions

1. Implement the task requirements
2. Test your changes
3. When complete, run: `scud set-status {id} done --tag {tag}`

Working directory: {working_dir}
"#,
        id = task.id,
        tag = tag,
        title = task.title,
        description = task.description,
        details = details,
        test_strategy = test_strategy,
        working_dir = working_dir.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_prompt() {
        let task = Task::new(
            "1".to_string(),
            "Test task".to_string(),
            "Do something".to_string(),
        );
        let prompt = generate_prompt(&task, "feature", Path::new("/tmp"));

        assert!(prompt.contains("[1]"));
        assert!(prompt.contains("Test task"));
        assert!(prompt.contains("Do something"));
        assert!(prompt.contains("feature"));
        assert!(prompt.contains("/tmp"));
    }

    #[test]
    fn test_generate_prompt_with_details() {
        let mut task = Task::new(
            "2".to_string(),
            "Task with details".to_string(),
            "Main description".to_string(),
        );
        task.details = Some("Extra details here".to_string());
        task.test_strategy = Some("Run unit tests".to_string());

        let prompt = generate_prompt(&task, "api", Path::new("/project"));

        assert!(prompt.contains("Extra details here"));
        assert!(prompt.contains("Run unit tests"));
        assert!(prompt.contains("## Details"));
        assert!(prompt.contains("## Test Strategy"));
    }

    #[test]
    fn test_summarize_tool_input_object() {
        let input = serde_json::json!({"path": "/foo", "content": "bar"});
        let summary = summarize_tool_input(&input);
        assert!(summary.contains("path"));
        assert!(summary.contains("content"));
    }

    #[test]
    fn test_summarize_tool_input_string() {
        let input = serde_json::json!("short string");
        let summary = summarize_tool_input(&input);
        assert_eq!(summary, "\"short string\"");

        let long_string = "a".repeat(100);
        let input = serde_json::json!(long_string);
        let summary = summarize_tool_input(&input);
        assert!(summary.len() < 60);
        assert!(summary.ends_with("...\""));
    }

    #[test]
    fn test_summarize_tool_input_null() {
        let input = serde_json::Value::Null;
        let summary = summarize_tool_input(&input);
        assert_eq!(summary, "");
    }

    #[test]
    fn test_agent_handle_debug() {
        let handle = AgentHandle {
            task_id: "auth:1".to_string(),
            session_id: "sess-123".to_string(),
            tag: "auth".to_string(),
        };
        let debug = format!("{:?}", handle);
        assert!(debug.contains("auth:1"));
        assert!(debug.contains("sess-123"));
    }
}
