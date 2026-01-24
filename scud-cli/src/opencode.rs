//! OpenCode Server integration
//!
//! Provides HTTP client and SSE event streaming for OpenCode Server mode.
//! This enables SCUD swarm to use OpenCode as an agent execution backend
//! instead of tmux windows or direct subprocesses.
//!
//! # Architecture
//!
//! ```text
//! SCUD Swarm ──HTTP──► OpenCode Server
//!                 ◄─SSE── real-time events (tool calls, output, completion)
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use scud::opencode::{AgentOrchestrator, OrchestratorEvent};
//! use tokio::sync::mpsc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let (event_tx, mut event_rx) = mpsc::channel(1000);
//!     let mut orchestrator = AgentOrchestrator::new(event_tx).await?;
//!
//!     // Spawn agents...
//!     // orchestrator.spawn_agent(...).await?;
//!
//!     // Wait for completion
//!     let results = orchestrator.wait_all().await;
//!     orchestrator.cleanup().await;
//!
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::extensions::runner::AgentResult;
use crate::models::task::Task;

// ============================================================================
// Configuration
// ============================================================================

/// Default OpenCode server port
pub const DEFAULT_PORT: u16 = 4096;

/// Default OpenCode server hostname
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// OpenCode server configuration
#[derive(Debug, Clone)]
pub struct OpenCodeConfig {
    /// Server hostname
    pub host: String,
    /// Server port
    pub port: u16,
    /// Optional authentication password
    pub password: Option<String>,
    /// Request timeout
    pub timeout: Duration,
}

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            password: std::env::var("OPENCODE_SERVER_PASSWORD").ok(),
            timeout: Duration::from_secs(300), // 5 minute timeout for long operations
        }
    }
}

impl OpenCodeConfig {
    /// Get the base URL for the OpenCode server
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

// ============================================================================
// API Types
// ============================================================================

/// Session creation request
#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    title: String,
}

/// Session response from OpenCode
#[derive(Debug, Deserialize)]
struct SessionResponse {
    id: String,
    #[serde(default)]
    title: Option<String>,
}

/// Message part for sending to OpenCode
#[derive(Debug, Serialize)]
struct MessagePart {
    #[serde(rename = "type")]
    part_type: String,
    text: String,
}

/// Model specification
#[derive(Debug, Serialize)]
struct ModelSpec {
    #[serde(rename = "providerID")]
    provider_id: String,
    #[serde(rename = "modelID")]
    model_id: String,
}

/// Message request to send to a session
#[derive(Debug, Serialize)]
struct SendMessageRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    model: ModelSpec,
    parts: Vec<MessagePart>,
}

/// Session status response
#[derive(Debug, Deserialize)]
struct SessionStatus {
    #[serde(rename = "type")]
    status_type: String,
}

/// Health check response
#[derive(Debug, Deserialize)]
struct HealthResponse {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

// ============================================================================
// Events
// ============================================================================

/// Events emitted by the orchestrator during agent execution
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Agent has started execution
    AgentStarted { task_id: String },
    /// Agent produced output text
    AgentOutput { task_id: String, text: String },
    /// Agent used a tool
    AgentToolUse { task_id: String, tool: String },
    /// Agent completed execution
    AgentCompleted { task_id: String, success: bool },
    /// Agent failed to start
    AgentSpawnFailed { task_id: String, error: String },
}

// ============================================================================
// HTTP Client
// ============================================================================

/// HTTP client for OpenCode Server API
#[derive(Clone)]
pub struct OpenCodeClient {
    client: Client,
    config: OpenCodeConfig,
}

impl OpenCodeClient {
    /// Create a new OpenCode client
    pub fn new(config: OpenCodeConfig) -> Result<Self> {
        let builder = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10));

        let client = builder.build().context("Failed to create HTTP client")?;

        Ok(Self { client, config })
    }

    /// Get basic auth credentials if configured
    fn basic_auth(&self) -> Option<(String, String)> {
        self.config.password.as_ref().map(|password| {
            let username = std::env::var("OPENCODE_SERVER_USERNAME")
                .unwrap_or_else(|_| "opencode".to_string());
            (username, password.clone())
        })
    }

    /// Check if the OpenCode server is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/global/health", self.config.base_url());

        let mut request = self.client.get(&url);
        if let Some((username, password)) = self.basic_auth() {
            request = request.basic_auth(username, Some(password));
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Create a new session
    pub async fn create_session(&self, title: &str, working_dir: &std::path::Path) -> Result<String> {
        let url = format!(
            "{}/session?directory={}",
            self.config.base_url(),
            working_dir.display()
        );

        let request = CreateSessionRequest {
            title: title.to_string(),
        };

        let mut req = self.client.post(&url).json(&request);
        if let Some((username, password)) = self.basic_auth() {
            req = req.basic_auth(username, Some(password));
        }
        let response = req.send().await.context("Failed to create session")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create session: {} - {}", status, body);
        }

        let session: SessionResponse = response
            .json()
            .await
            .context("Failed to parse session response")?;

        Ok(session.id)
    }

    /// Send a message to a session
    pub async fn send_message(
        &self,
        session_id: &str,
        prompt: &str,
        provider: &str,
        model: &str,
        working_dir: &std::path::Path,
    ) -> Result<()> {
        let url = format!("{}/session/{}/message", self.config.base_url(), session_id);

        let request = SendMessageRequest {
            agent: Some("general".to_string()),
            model: ModelSpec {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            },
            parts: vec![MessagePart {
                part_type: "text".to_string(),
                text: prompt.to_string(),
            }],
        };

        let mut req = self
            .client
            .post(&url)
            .header("x-opencode-directory", working_dir.to_string_lossy().to_string())
            .json(&request);
        if let Some((username, password)) = self.basic_auth() {
            req = req.basic_auth(username, Some(password));
        }
        let response = req.send().await.context("Failed to send message")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send message: {} - {}", status, body);
        }

        Ok(())
    }

    /// Get session status (busy or idle)
    pub async fn get_session_status(&self, session_id: &str) -> Result<String> {
        let url = format!("{}/session/{}/status", self.config.base_url(), session_id);

        let mut req = self.client.get(&url);
        if let Some((username, password)) = self.basic_auth() {
            req = req.basic_auth(username, Some(password));
        }
        let response = req.send().await.context("Failed to get session status")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Failed to get session status: {}", status);
        }

        let status: SessionStatus = response
            .json()
            .await
            .context("Failed to parse status response")?;

        Ok(status.status_type)
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let url = format!("{}/sessions/{}", self.config.base_url(), session_id);

        let mut req = self.client.delete(&url);
        if let Some((username, password)) = self.basic_auth() {
            req = req.basic_auth(username, Some(password));
        }
        let response = req.send().await.context("Failed to delete session")?;

        if !response.status().is_success() {
            // Don't fail on delete errors - session might already be gone
            eprintln!(
                "Warning: Failed to delete session {}: {}",
                session_id,
                response.status()
            );
        }

        Ok(())
    }

    /// Abort a running session
    pub async fn abort_session(&self, session_id: &str) -> Result<()> {
        let url = format!("{}/sessions/{}/abort", self.config.base_url(), session_id);

        let mut req = self.client.post(&url);
        if let Some((username, password)) = self.basic_auth() {
            req = req.basic_auth(username, Some(password));
        }
        let response = req.send().await.context("Failed to abort session")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Failed to abort session: {}", status);
        }

        Ok(())
    }
}

// ============================================================================
// Agent State
// ============================================================================

/// State of a single agent
#[derive(Debug)]
struct AgentState {
    task_id: String,
    tag: String,
    session_id: String,
    started_at: Instant,
    completed: bool,
    success: bool,
    exit_code: Option<i32>,
}

impl AgentState {
    fn new(task_id: String, tag: String, session_id: String) -> Self {
        Self {
            task_id,
            tag,
            session_id,
            started_at: Instant::now(),
            completed: false,
            success: false,
            exit_code: None,
        }
    }

    fn mark_completed(&mut self, success: bool) {
        self.completed = true;
        self.success = success;
        self.exit_code = if success { Some(0) } else { Some(1) };
    }

    fn to_result(&self) -> AgentResult {
        AgentResult {
            task_id: self.task_id.clone(),
            success: self.success,
            exit_code: self.exit_code,
            duration_ms: self.started_at.elapsed().as_millis() as u64,
        }
    }
}

// ============================================================================
// Agent Orchestrator
// ============================================================================

/// Orchestrator for managing multiple agents via OpenCode Server
///
/// The orchestrator handles:
/// - Creating OpenCode sessions for each task
/// - Sending prompts to agents
/// - Polling for completion
/// - Collecting results
pub struct AgentOrchestrator {
    client: OpenCodeClient,
    event_tx: mpsc::Sender<OrchestratorEvent>,
    agents: Arc<Mutex<HashMap<String, AgentState>>>,
    working_dir: PathBuf,
    config: OpenCodeConfig,
}

impl AgentOrchestrator {
    /// Create a new orchestrator
    ///
    /// # Arguments
    /// * `event_tx` - Channel for sending orchestrator events
    ///
    /// # Returns
    /// A new orchestrator instance connected to the OpenCode server
    pub async fn new(event_tx: mpsc::Sender<OrchestratorEvent>) -> Result<Self> {
        let config = OpenCodeConfig::default();
        let client = OpenCodeClient::new(config.clone())?;

        // Check if server is running
        if !client.health_check().await? {
            anyhow::bail!(
                "OpenCode server is not running at {}. \
                 Start it with: opencode serve --port {}",
                config.base_url(),
                config.port
            );
        }

        let working_dir = std::env::current_dir().unwrap_or_default();

        Ok(Self {
            client,
            event_tx,
            agents: Arc::new(Mutex::new(HashMap::new())),
            working_dir,
            config,
        })
    }

    /// Create orchestrator with custom configuration
    pub async fn with_config(
        event_tx: mpsc::Sender<OrchestratorEvent>,
        config: OpenCodeConfig,
        working_dir: PathBuf,
    ) -> Result<Self> {
        let client = OpenCodeClient::new(config.clone())?;

        // Check if server is running
        if !client.health_check().await? {
            anyhow::bail!(
                "OpenCode server is not running at {}. \
                 Start it with: opencode serve --port {}",
                config.base_url(),
                config.port
            );
        }

        Ok(Self {
            client,
            event_tx,
            agents: Arc::new(Mutex::new(HashMap::new())),
            working_dir,
            config,
        })
    }

    /// Spawn an agent for a task
    ///
    /// Creates an OpenCode session and sends the prompt to start execution.
    /// The agent runs asynchronously - use `wait_all()` to wait for completion.
    ///
    /// # Arguments
    /// * `task` - The task to execute
    /// * `tag` - The task's tag/phase
    /// * `prompt` - The prompt to send to the agent
    /// * `model` - Optional (provider, model) tuple, defaults to ("anthropic", "claude-sonnet-4-20250514")
    pub async fn spawn_agent(
        &mut self,
        task: &Task,
        tag: &str,
        prompt: &str,
        model: Option<(&str, &str)>,
    ) -> Result<()> {
        let (provider, model_id) = model.unwrap_or(("anthropic", "claude-sonnet-4-20250514"));
        let task_id = task.id.clone();

        // Create session for this task
        let session_title = format!("[{}] {}", task_id, task.title);
        let session_id = self
            .client
            .create_session(&session_title, &self.working_dir)
            .await
            .with_context(|| format!("Failed to create session for task {}", task_id))?;

        // Track agent state
        {
            let mut agents = self.agents.lock().await;
            agents.insert(
                task_id.clone(),
                AgentState::new(task_id.clone(), tag.to_string(), session_id.clone()),
            );
        }

        // Emit started event
        let _ = self
            .event_tx
            .send(OrchestratorEvent::AgentStarted {
                task_id: task_id.clone(),
            })
            .await;

        // Send the prompt
        self.client
            .send_message(&session_id, prompt, provider, model_id, &self.working_dir)
            .await
            .with_context(|| format!("Failed to send message for task {}", task_id))?;

        Ok(())
    }

    /// Wait for all spawned agents to complete
    ///
    /// Polls each agent's session status until all are idle.
    /// Returns results for all agents.
    pub async fn wait_all(&self) -> Vec<AgentResult> {
        let poll_interval = Duration::from_secs(2);
        let max_wait = Duration::from_secs(1800); // 30 minute timeout
        let start = Instant::now();

        loop {
            // Check if all agents are done
            let mut all_done = true;
            let mut pending_tasks = Vec::new();

            {
                let agents = self.agents.lock().await;
                for (task_id, agent) in agents.iter() {
                    if !agent.completed {
                        all_done = false;
                        pending_tasks.push((task_id.clone(), agent.session_id.clone()));
                    }
                }
            }

            if all_done {
                break;
            }

            // Check timeout
            if start.elapsed() > max_wait {
                eprintln!("Warning: Timeout waiting for agents to complete");
                // Mark remaining agents as failed
                let mut agents = self.agents.lock().await;
                for (task_id, _) in &pending_tasks {
                    if let Some(agent) = agents.get_mut(task_id) {
                        agent.mark_completed(false);
                    }
                }
                break;
            }

            // Poll status for each pending agent
            for (task_id, session_id) in &pending_tasks {
                match self.client.get_session_status(session_id).await {
                    Ok(status) => {
                        if status == "idle" {
                            // Agent completed
                            let mut agents = self.agents.lock().await;
                            if let Some(agent) = agents.get_mut(task_id) {
                                agent.mark_completed(true);
                                let _ = self
                                    .event_tx
                                    .send(OrchestratorEvent::AgentCompleted {
                                        task_id: task_id.clone(),
                                        success: true,
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to get status for task {}: {}", task_id, e);
                        // Mark as failed on repeated errors
                        let mut agents = self.agents.lock().await;
                        if let Some(agent) = agents.get_mut(task_id) {
                            agent.mark_completed(false);
                            let _ = self
                                .event_tx
                                .send(OrchestratorEvent::AgentCompleted {
                                    task_id: task_id.clone(),
                                    success: false,
                                })
                                .await;
                        }
                    }
                }
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }

        // Collect results
        let agents = self.agents.lock().await;
        agents.values().map(|a| a.to_result()).collect()
    }

    /// Cleanup resources
    ///
    /// Deletes all OpenCode sessions created by this orchestrator.
    pub async fn cleanup(&self) {
        let agents = self.agents.lock().await;
        for agent in agents.values() {
            if let Err(e) = self.client.delete_session(&agent.session_id).await {
                eprintln!(
                    "Warning: Failed to cleanup session for task {}: {}",
                    agent.task_id, e
                );
            }
        }
    }
}

// ============================================================================
// Server Manager
// ============================================================================

/// Global OpenCode server manager (lazily initialized)
static MANAGER: tokio::sync::OnceCell<RwLock<OpenCodeManager>> = tokio::sync::OnceCell::const_new();

/// Get the global OpenCode manager
pub async fn global_manager() -> Result<OpenCodeManager> {
    let manager_lock = MANAGER
        .get_or_try_init(|| async {
            let manager = OpenCodeManager::new()?;
            Ok::<_, anyhow::Error>(RwLock::new(manager))
        })
        .await?;

    let guard = manager_lock.read().await;
    Ok(guard.clone())
}

/// Manager for the OpenCode server process
#[derive(Clone)]
pub struct OpenCodeManager {
    config: OpenCodeConfig,
    client: OpenCodeClient,
}

impl OpenCodeManager {
    /// Create a new manager
    pub fn new() -> Result<Self> {
        let config = OpenCodeConfig::default();
        let client = OpenCodeClient::new(config.clone())?;

        Ok(Self { config, client })
    }

    /// Create manager with custom configuration
    pub fn with_config(config: OpenCodeConfig) -> Result<Self> {
        let client = OpenCodeClient::new(config.clone())?;
        Ok(Self { config, client })
    }

    /// Get the server base URL
    pub fn base_url(&self) -> String {
        self.config.base_url()
    }

    /// Check if the server is running
    pub async fn is_running(&self) -> bool {
        self.client.health_check().await.unwrap_or(false)
    }

    /// Ensure the server is running
    ///
    /// Checks if the server is healthy. Does not start the server automatically
    /// as that should be done by the user.
    pub async fn ensure_running(&self) -> Result<()> {
        if !self.is_running().await {
            anyhow::bail!(
                "OpenCode server is not running at {}.\n\
                 \n\
                 To start the server, run:\n\
                 \n\
                 opencode serve --port {}\n\
                 \n\
                 Or with authentication:\n\
                 \n\
                 OPENCODE_SERVER_PASSWORD=secret opencode serve --port {}",
                self.config.base_url(),
                self.config.port,
                self.config.port
            );
        }
        Ok(())
    }

    /// Get the HTTP client
    pub fn client(&self) -> &OpenCodeClient {
        &self.client
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = OpenCodeConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 4096);
        assert_eq!(config.base_url(), "http://127.0.0.1:4096");
    }

    #[test]
    fn test_config_custom_port() {
        let config = OpenCodeConfig {
            port: 8080,
            ..Default::default()
        };
        assert_eq!(config.base_url(), "http://127.0.0.1:8080");
    }

    #[test]
    fn test_agent_state() {
        let mut agent = AgentState::new(
            "task:1".to_string(),
            "test-tag".to_string(),
            "session-123".to_string(),
        );

        assert!(!agent.completed);
        assert!(!agent.success);

        agent.mark_completed(true);

        assert!(agent.completed);
        assert!(agent.success);
        assert_eq!(agent.exit_code, Some(0));

        let result = agent.to_result();
        assert_eq!(result.task_id, "task:1");
        assert!(result.success);
    }

    #[test]
    fn test_orchestrator_event_variants() {
        let events = vec![
            OrchestratorEvent::AgentStarted {
                task_id: "1".to_string(),
            },
            OrchestratorEvent::AgentOutput {
                task_id: "1".to_string(),
                text: "Hello".to_string(),
            },
            OrchestratorEvent::AgentToolUse {
                task_id: "1".to_string(),
                tool: "Read".to_string(),
            },
            OrchestratorEvent::AgentCompleted {
                task_id: "1".to_string(),
                success: true,
            },
            OrchestratorEvent::AgentSpawnFailed {
                task_id: "1".to_string(),
                error: "Failed".to_string(),
            },
        ];

        // Just verify we can create all variants
        assert_eq!(events.len(), 5);
    }
}
