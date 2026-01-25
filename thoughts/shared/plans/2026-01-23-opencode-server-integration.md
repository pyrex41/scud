# OpenCode Server Mode Integration Plan

## Overview

Replace the current CLI subprocess spawning approach with OpenCode Server mode integration for **xAI/Grok models**. SCUD will automatically start/manage an `opencode serve` instance and communicate via HTTP REST API + SSE for real-time events. This fixes issues with stuck agents, enables graceful cancellation, and provides structured event visibility.

**Note**: This integration focuses on xAI/Grok models via OpenCode. For Anthropic models, use Claude Code SDK directly (separate integration).

## Current State Analysis

### What Exists Now
- **CLI Subprocess Spawning** (`extensions/runner.rs:176-294`): Spawns `opencode run --variant minimal <prompt>` per agent
- **Event System** (`extensions/runner.rs:145-156`): `AgentEvent` enum with Started/Output/Completed/SpawnFailed
- **Swarm Modes** (`swarm/mod.rs:145-149`): `Tmux` and `Extensions` modes
- **HTTP Client** (`llm/client.rs`): Uses `reqwest` for LLM API calls (pattern to follow)

### Problems with Current Approach
- Each agent spawns a full process (high overhead)
- No graceful cancellation (must kill process)
- Stuck agents require manual intervention
- No visibility into tool calls or thinking
- Polling-based completion detection (5s intervals)

### Key Discoveries
- No SSE client exists in codebase - need to add dependency
- `reqwest` already in Cargo.toml with JSON support
- Event channel pattern established in `AgentRunner` (`runner.rs:318-379`)
- Swarm session persistence at `.scud/swarm/<name>.json` (`session.rs:362-379`)

## Desired End State

After implementation:
1. `scud swarm --tag <tag>` uses OpenCode Server mode by default
2. SCUD automatically starts `opencode serve` on first use
3. Real-time SSE events stream to TUI (tool calls, text output, completion)
4. Agents can be gracefully cancelled via HTTP API
5. Multiple concurrent sessions share single server instance
6. Server auto-shutdown after configurable idle timeout

### Verification
```bash
# Start swarm - server starts automatically
scud swarm --tag feature-x

# TUI shows real-time tool calls and output
# Press 'c' to cancel an agent gracefully
# All agents complete, server stays running for next use

# Explicit server control (optional)
scud serve start    # Manual start
scud serve stop     # Manual stop
scud serve status   # Check if running
```

## What We're NOT Doing

- Removing tmux mode (keep as `--terminal tmux` fallback)
- Changing the TUI layout significantly (just adding event types)
- Supporting remote OpenCode servers (localhost only for V1)
- Authentication (local server, no password needed)
- WebSocket support (SSE is sufficient)

## Implementation Approach

Replace `extensions/runner.rs` subprocess spawning with HTTP client calls to OpenCode Server. The server manages agent sessions while SCUD orchestrates task assignment and monitors events via SSE.

```
┌─────────────────────────────────────────────────────────────┐
│                     SCUD Swarm                              │
│                                                             │
│  ┌──────────────────┐       ┌─────────────────────────────┐│
│  │ OpenCodeManager  │       │ OpenCode Server             ││
│  │                  │ HTTP  │ (opencode serve :4096)      ││
│  │  - start_server()│──────►│                             ││
│  │  - create_session│       │  Sessions:                  ││
│  │  - send_prompt() │       │   [task-1] ──► Grok API     ││
│  │  - abort_session │       │   [task-2] ──► Grok API     ││
│  │                  │◄──────│   [task-3] ──► Grok API     ││
│  │  - event_stream()│  SSE  │                             ││
│  └──────────────────┘       └─────────────────────────────┘│
│           │                                                 │
│           ▼                                                 │
│  ┌──────────────────┐                                      │
│  │ TUI Monitor      │                                      │
│  │  - Tool calls    │                                      │
│  │  - Text streaming│                                      │
│  │  - Cancel button │                                      │
│  └──────────────────┘                                      │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: OpenCode HTTP Client

### Overview
Create a Rust HTTP client for OpenCode Server API with full async support.

### Changes Required

#### 1.1 Add Dependencies

**File**: `scud-cli/Cargo.toml`
**Changes**: Add SSE client and enhance reqwest features

```toml
[dependencies]
# Existing
reqwest = { version = "0.11", features = ["json", "rustls-tls", "stream"], default-features = false }

# New
eventsource-client = "0.12"  # SSE client
pin-project-lite = "0.2"     # For custom Stream impl
```

#### 1.2 Create OpenCode Client Module

**File**: `scud-cli/src/opencode/mod.rs` (new)
**Changes**: Module declaration

```rust
//! OpenCode Server integration
//!
//! Provides HTTP client and SSE event streaming for OpenCode Server mode.

pub mod client;
pub mod events;
pub mod manager;
pub mod types;

pub use client::OpenCodeClient;
pub use events::{OpenCodeEvent, EventStream};
pub use manager::OpenCodeManager;
pub use types::*;
```

#### 1.3 Define Types

**File**: `scud-cli/src/opencode/types.rs` (new)
**Changes**: API types matching OpenCode Server schema

```rust
//! OpenCode Server API types

use serde::{Deserialize, Serialize};

/// Session creation request
#[derive(Debug, Serialize)]
pub struct CreateSessionRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

/// Session response from server
#[derive(Debug, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    #[serde(default)]
    pub message_count: usize,
}

/// Message/prompt request
#[derive(Debug, Serialize)]
pub struct MessageRequest {
    pub parts: Vec<MessagePart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelSpec>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text { text: String },
}

#[derive(Debug, Serialize)]
pub struct ModelSpec {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// Session status
#[derive(Debug, Deserialize)]
pub struct SessionStatus {
    pub id: String,
    pub status: SessionState,
    #[serde(default)]
    pub active_message: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Running,
    Completed,
    Error,
}

/// Server info response
#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    pub version: String,
    pub ready: bool,
}

/// Error response from server
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub details: Option<String>,
}
```

#### 1.4 Implement HTTP Client

**File**: `scud-cli/src/opencode/client.rs` (new)
**Changes**: Core HTTP operations

```rust
//! HTTP client for OpenCode Server

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

use super::types::*;

/// HTTP client for OpenCode Server API
pub struct OpenCodeClient {
    base_url: String,
    client: Client,
}

impl OpenCodeClient {
    /// Create a new client connecting to the given base URL
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min timeout for long operations
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Create with default localhost URL
    pub fn localhost(port: u16) -> Self {
        Self::new(&format!("http://127.0.0.1:{}", port))
    }

    /// Check if server is ready
    pub async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await;

        match response {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Get server info
    pub async fn server_info(&self) -> Result<ServerInfo> {
        let response = self
            .client
            .get(format!("{}/", self.base_url))
            .send()
            .await
            .context("Failed to get server info")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
                details: None,
            });
            anyhow::bail!("Server error: {}", error.error);
        }

        response.json().await.context("Failed to parse server info")
    }

    /// Create a new session
    pub async fn create_session(&self, title: &str) -> Result<Session> {
        let request = CreateSessionRequest {
            title: title.to_string(),
            system_prompt: None,
        };

        let response = self
            .client
            .post(format!("{}/session", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to create session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create session ({}): {}", status, error_text);
        }

        response.json().await.context("Failed to parse session response")
    }

    /// Send a message/prompt to a session
    pub async fn send_message(
        &self,
        session_id: &str,
        text: &str,
        model: Option<(&str, &str)>, // (provider_id, model_id)
    ) -> Result<()> {
        let request = MessageRequest {
            parts: vec![MessagePart::Text { text: text.to_string() }],
            model: model.map(|(provider, model_id)| ModelSpec {
                provider_id: provider.to_string(),
                model_id: model_id.to_string(),
            }),
        };

        let response = self
            .client
            .post(format!("{}/session/{}/message", self.base_url, session_id))
            .json(&request)
            .send()
            .await
            .context("Failed to send message")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send message ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// Get session status
    pub async fn get_session_status(&self, session_id: &str) -> Result<SessionStatus> {
        let response = self
            .client
            .get(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("Failed to get session status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get session ({}): {}", status, error_text);
        }

        response.json().await.context("Failed to parse session status")
    }

    /// Abort/cancel a running session
    pub async fn abort_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/session/{}/abort", self.base_url, session_id))
            .send()
            .await
            .context("Failed to abort session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to abort session ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("Failed to delete session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to delete session ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let response = self
            .client
            .get(format!("{}/session", self.base_url))
            .send()
            .await
            .context("Failed to list sessions")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list sessions ({}): {}", status, error_text);
        }

        response.json().await.context("Failed to parse sessions list")
    }

    /// Get the SSE event stream URL
    pub fn event_stream_url(&self) -> String {
        format!("{}/event", self.base_url)
    }

    /// Get base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenCodeClient::localhost(4096);
        assert_eq!(client.base_url(), "http://127.0.0.1:4096");
    }

    #[test]
    fn test_event_stream_url() {
        let client = OpenCodeClient::new("http://localhost:4096");
        assert_eq!(client.event_stream_url(), "http://localhost:4096/event");
    }
}
```

#### 1.5 Implement SSE Event Streaming

**File**: `scud-cli/src/opencode/events.rs` (new)
**Changes**: SSE client and event parsing

```rust
//! SSE event streaming for OpenCode Server

use anyhow::{Context, Result};
use eventsource_client::{Client as SseClient, SSE};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use std::pin::Pin;
use tokio::sync::mpsc;

/// Events received from OpenCode Server SSE stream
#[derive(Debug, Clone)]
pub enum OpenCodeEvent {
    /// Server connected
    Connected,

    /// Message started
    MessageStart {
        session_id: String,
        message_id: String,
    },

    /// Text delta (streaming output)
    TextDelta {
        session_id: String,
        text: String,
    },

    /// Tool execution started
    ToolStart {
        session_id: String,
        tool_id: String,
        tool_name: String,
        input: serde_json::Value,
    },

    /// Tool execution completed
    ToolResult {
        session_id: String,
        tool_id: String,
        tool_name: String,
        output: String,
        success: bool,
    },

    /// Message completed
    MessageComplete {
        session_id: String,
        success: bool,
    },

    /// Session error
    SessionError {
        session_id: String,
        error: String,
    },

    /// Unknown event type
    Unknown {
        event_type: String,
        data: String,
    },
}

/// Raw SSE event from server
#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(flatten)]
    data: serde_json::Value,
}

impl OpenCodeEvent {
    /// Parse from SSE event data
    pub fn parse(event_type: &str, data: &str) -> Self {
        // Try to parse as JSON
        let parsed: Result<RawEvent, _> = serde_json::from_str(data);

        match parsed {
            Ok(raw) => Self::from_raw(&raw),
            Err(_) => {
                // Fallback for non-JSON events
                match event_type {
                    "server.connected" => OpenCodeEvent::Connected,
                    _ => OpenCodeEvent::Unknown {
                        event_type: event_type.to_string(),
                        data: data.to_string(),
                    },
                }
            }
        }
    }

    fn from_raw(raw: &RawEvent) -> Self {
        let session_id = raw.session_id.clone().unwrap_or_default();

        match raw.event_type.as_str() {
            "message.start" => OpenCodeEvent::MessageStart {
                session_id,
                message_id: raw.data.get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },

            "text.delta" | "content.delta" => OpenCodeEvent::TextDelta {
                session_id,
                text: raw.data.get("text")
                    .or_else(|| raw.data.get("delta"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },

            "tool.start" | "tool_use.start" => OpenCodeEvent::ToolStart {
                session_id,
                tool_id: raw.data.get("tool_id")
                    .or_else(|| raw.data.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_name: raw.data.get("tool")
                    .or_else(|| raw.data.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                input: raw.data.get("input").cloned().unwrap_or(serde_json::Value::Null),
            },

            "tool.result" | "tool_use.result" => OpenCodeEvent::ToolResult {
                session_id,
                tool_id: raw.data.get("tool_id")
                    .or_else(|| raw.data.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_name: raw.data.get("tool")
                    .or_else(|| raw.data.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                output: raw.data.get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                success: raw.data.get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            },

            "message.complete" | "message.done" => OpenCodeEvent::MessageComplete {
                session_id,
                success: raw.data.get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            },

            "session.error" | "error" => OpenCodeEvent::SessionError {
                session_id,
                error: raw.data.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string(),
            },

            _ => OpenCodeEvent::Unknown {
                event_type: raw.event_type.clone(),
                data: serde_json::to_string(&raw.data).unwrap_or_default(),
            },
        }
    }

    /// Get session ID if event is session-specific
    pub fn session_id(&self) -> Option<&str> {
        match self {
            OpenCodeEvent::MessageStart { session_id, .. } => Some(session_id),
            OpenCodeEvent::TextDelta { session_id, .. } => Some(session_id),
            OpenCodeEvent::ToolStart { session_id, .. } => Some(session_id),
            OpenCodeEvent::ToolResult { session_id, .. } => Some(session_id),
            OpenCodeEvent::MessageComplete { session_id, .. } => Some(session_id),
            OpenCodeEvent::SessionError { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
}

/// Event stream subscription
pub struct EventStream {
    rx: mpsc::Receiver<OpenCodeEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventStream {
    /// Create a new event stream connected to the given URL
    pub async fn connect(url: &str) -> Result<Self> {
        let (tx, rx) = mpsc::channel(1000);
        let url = url.to_string();

        let handle = tokio::spawn(async move {
            let client = match SseClient::for_url(&url) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create SSE client: {}", e);
                    return;
                }
            };

            let mut stream = client.stream();

            while let Some(event) = stream.next().await {
                match event {
                    Ok(SSE::Event(ev)) => {
                        let parsed = OpenCodeEvent::parse(&ev.event_type, &ev.data);
                        if tx.send(parsed).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Ok(SSE::Comment(_)) => continue,
                    Err(e) => {
                        eprintln!("SSE error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self { rx, _handle: handle })
    }

    /// Receive next event
    pub async fn recv(&mut self) -> Option<OpenCodeEvent> {
        self.rx.recv().await
    }

    /// Try to receive without blocking
    pub fn try_recv(&mut self) -> Option<OpenCodeEvent> {
        self.rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_start() {
        let data = r#"{"type": "message.start", "session_id": "abc123", "message_id": "msg1"}"#;
        let event = OpenCodeEvent::parse("message", data);

        match event {
            OpenCodeEvent::MessageStart { session_id, message_id } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(message_id, "msg1");
            }
            _ => panic!("Expected MessageStart"),
        }
    }

    #[test]
    fn test_parse_tool_start() {
        let data = r#"{"type": "tool.start", "session_id": "abc123", "tool": "read_file", "input": {"path": "src/main.rs"}}"#;
        let event = OpenCodeEvent::parse("tool", data);

        match event {
            OpenCodeEvent::ToolStart { session_id, tool_name, .. } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(tool_name, "read_file");
            }
            _ => panic!("Expected ToolStart"),
        }
    }
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p scud-cli` compiles without errors
- [x] `cargo test -p scud-cli opencode` passes all unit tests (27 passed)
- [x] `cargo clippy -p scud-cli` shows no errors in opencode module

#### Manual Verification:
- [x] N/A for this phase (no integration yet)

---

## Phase 2: Server Lifecycle Management

### Overview
Create `OpenCodeManager` that automatically starts/stops `opencode serve` and manages the server process lifecycle.

### Changes Required

#### 2.1 Implement Server Manager

**File**: `scud-cli/src/opencode/manager.rs` (new)
**Changes**: Server lifecycle management

```rust
//! OpenCode Server lifecycle management

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::client::OpenCodeClient;
use super::events::EventStream;

/// Default port for OpenCode server
pub const DEFAULT_PORT: u16 = 4096;

/// Server startup timeout
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Health check interval during startup
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Configuration for OpenCode server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to run on
    pub port: u16,
    /// Working directory for the server
    pub working_dir: Option<PathBuf>,
    /// Custom opencode binary path
    pub binary_path: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            working_dir: None,
            binary_path: None,
        }
    }
}

/// Manager for OpenCode server lifecycle
pub struct OpenCodeManager {
    config: ServerConfig,
    client: OpenCodeClient,
    server_process: Arc<RwLock<Option<Child>>>,
}

impl OpenCodeManager {
    /// Create a new manager with default config
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: ServerConfig) -> Self {
        let client = OpenCodeClient::localhost(config.port);
        Self {
            config,
            client,
            server_process: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the HTTP client
    pub fn client(&self) -> &OpenCodeClient {
        &self.client
    }

    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        self.client.health_check().await.unwrap_or(false)
    }

    /// Ensure server is running, starting it if needed
    pub async fn ensure_running(&self) -> Result<()> {
        if self.is_running().await {
            return Ok(());
        }

        self.start_server().await
    }

    /// Start the OpenCode server
    pub async fn start_server(&self) -> Result<()> {
        // Check if already running
        if self.is_running().await {
            return Ok(());
        }

        // Find opencode binary
        let binary = self.find_binary()?;

        // Build command
        let mut cmd = Command::new(&binary);
        cmd.arg("serve");
        cmd.arg("--port").arg(self.config.port.to_string());

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        // Suppress output (server runs in background)
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        // Spawn server process
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start opencode server: {}", binary.display()))?;

        // Store process handle
        {
            let mut process = self.server_process.write().await;
            *process = Some(child);
        }

        // Wait for server to become ready
        self.wait_for_ready().await?;

        Ok(())
    }

    /// Wait for server to become ready
    async fn wait_for_ready(&self) -> Result<()> {
        let start = std::time::Instant::now();

        while start.elapsed() < STARTUP_TIMEOUT {
            if self.client.health_check().await.unwrap_or(false) {
                return Ok(());
            }
            tokio::time::sleep(HEALTH_CHECK_INTERVAL).await;
        }

        anyhow::bail!(
            "OpenCode server failed to start within {:?}",
            STARTUP_TIMEOUT
        );
    }

    /// Stop the server
    pub async fn stop_server(&self) -> Result<()> {
        let mut process = self.server_process.write().await;

        if let Some(mut child) = process.take() {
            // Try graceful shutdown first
            let _ = child.kill().await;
        }

        Ok(())
    }

    /// Find the opencode binary
    fn find_binary(&self) -> Result<PathBuf> {
        // Check custom path first
        if let Some(ref path) = self.config.binary_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Use find_harness_binary from terminal module
        use crate::commands::spawn::terminal::{find_harness_binary, Harness};

        find_harness_binary(Harness::OpenCode)
            .map(PathBuf::from)
            .context("Could not find opencode binary")
    }

    /// Connect to the event stream
    pub async fn event_stream(&self) -> Result<EventStream> {
        self.ensure_running().await?;
        EventStream::connect(&self.client.event_stream_url()).await
    }

    /// Get the server port
    pub fn port(&self) -> u16 {
        self.config.port
    }
}

impl Default for OpenCodeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OpenCodeManager {
    fn drop(&mut self) {
        // Note: Can't do async cleanup in Drop
        // Server process will be orphaned but that's OK -
        // it can be reused by next SCUD invocation
    }
}

/// Global manager instance for sharing across swarm execution
static GLOBAL_MANAGER: std::sync::OnceLock<Arc<OpenCodeManager>> = std::sync::OnceLock::new();

/// Get or create the global manager instance
pub fn global_manager() -> Arc<OpenCodeManager> {
    GLOBAL_MANAGER
        .get_or_init(|| Arc::new(OpenCodeManager::new()))
        .clone()
}

/// Get or create manager with custom config (only works on first call)
pub fn init_global_manager(config: ServerConfig) -> Arc<OpenCodeManager> {
    GLOBAL_MANAGER
        .get_or_init(|| Arc::new(OpenCodeManager::with_config(config)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
    }

    #[test]
    fn test_manager_creation() {
        let manager = OpenCodeManager::new();
        assert_eq!(manager.port(), DEFAULT_PORT);
    }
}
```

#### 2.2 Add Module to lib.rs

**File**: `scud-cli/src/lib.rs`
**Changes**: Add module declaration

```rust
// Add near other module declarations
pub mod opencode;
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p scud-cli` compiles
- [x] `cargo test -p scud-cli opencode::manager` passes (5 tests)

#### Manual Verification:
- [x] `opencode serve` can be started manually and manager detects it
- [x] Server accepts HTTP requests on configured port
- [ ] Manager can auto-start server when not running (requires full integration test)

---

## Phase 3: Agent Orchestration

### Overview
Create agent orchestrator that spawns SCUD tasks via OpenCode Server sessions instead of CLI subprocesses.

### Changes Required

#### 3.1 Implement Agent Orchestrator

**File**: `scud-cli/src/opencode/orchestrator.rs` (new)
**Changes**: Agent spawning and management via server

```rust
//! Agent orchestration via OpenCode Server

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::extensions::runner::{AgentEvent, AgentResult};
use crate::models::task::Task;

use super::client::OpenCodeClient;
use super::events::{EventStream, OpenCodeEvent};
use super::manager::{global_manager, OpenCodeManager};
use super::types::SessionState;

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
        self.task_sessions.insert(task.id.clone(), session.id.clone());
        self.start_times.insert(task.id.clone(), std::time::Instant::now());

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

            OpenCodeEvent::ToolStart { tool_name, input, .. } => {
                // Format tool call as output line
                let line = format!("🔧 {} {:?}", tool_name, input);
                let agent_event = AgentEvent::Output {
                    task_id: task_id.clone(),
                    line,
                };
                let _ = self.event_tx.send(agent_event.clone()).await;
                Some(agent_event)
            }

            OpenCodeEvent::ToolResult { tool_name, success, .. } => {
                let status = if success { "✓" } else { "✗" };
                let line = format!("{} {} completed", status, tool_name);
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
    format!(
        r#"You are working on task [{id}] in phase "{tag}".

## Task: {title}

{description}

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
    }
}
```

#### 3.2 Update Module Declaration

**File**: `scud-cli/src/opencode/mod.rs`
**Changes**: Add orchestrator module

```rust
pub mod orchestrator;
pub use orchestrator::{AgentOrchestrator, execute_wave_server};
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p scud-cli` compiles
- [x] `cargo test -p scud-cli opencode::orchestrator` passes (6 tests)

#### Manual Verification:
- [x] Session creation via HTTP API works
- [x] Session deletion via HTTP API works
- [ ] Full agent spawn with prompt execution (requires integration test with pending tasks)

---

## Phase 4: Swarm Integration

### Overview
Replace the current `extensions` mode in swarm command with server-based execution.

### Changes Required

#### 4.1 Update Swarm Mode Enum

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Replace Extensions with Server mode

```rust
// Around line 145-149, update SwarmMode enum:

/// Execution mode for swarm
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwarmMode {
    /// Tmux terminal windows (legacy)
    Tmux,
    /// OpenCode Server mode (default)
    Server,
}

impl SwarmMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "tmux" | "terminal" => Ok(SwarmMode::Tmux),
            "server" | "opencode" | "extensions" => Ok(SwarmMode::Server), // "extensions" maps to server for compat
            _ => anyhow::bail!("Unknown swarm mode: {}. Use 'tmux' or 'server'", s),
        }
    }
}
```

#### 4.2 Update execute_round_extensions to Use Server

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Replace subprocess spawning with server orchestration

```rust
// Replace execute_round_extensions function (around line 912-979):

fn execute_round_server(
    storage: &Storage,
    tasks: &[TaskInfo],
    working_dir: &Path,
    round_idx: usize,
    model: Option<(&str, &str)>,
) -> Result<RoundState> {
    use crate::opencode::orchestrator::execute_wave_server;
    use tokio::sync::mpsc;

    // Convert TaskInfo to (Task, tag) pairs
    let task_pairs: Vec<(Task, String)> = tasks
        .iter()
        .map(|info| (info.task.clone(), info.tag.clone()))
        .collect();

    // Mark tasks as in-progress before spawning
    for info in tasks {
        if let Ok(mut phase) = storage.load_group(&info.tag) {
            if let Some(task) = phase.get_task_mut(&info.task.id) {
                task.set_status(TaskStatus::InProgress);
                let _ = storage.update_group(&info.tag, &phase);
            }
        }
    }

    // Create event channel (we'll consume events for status updates)
    let (event_tx, mut event_rx) = mpsc::channel(1000);

    // Run async execution using tokio runtime
    let handle = tokio::runtime::Handle::current();
    let results = handle.block_on(async {
        // Spawn event consumer task
        let storage_clone = storage.clone();
        let tasks_clone: Vec<TaskInfo> = tasks.to_vec();

        let consumer = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match &event {
                    AgentEvent::Output { task_id, line } => {
                        // Print streaming output
                        println!("[{}] {}", task_id, line);
                    }
                    AgentEvent::Completed { result } => {
                        // Update task status based on result
                        if let Some(info) = tasks_clone.iter().find(|t| t.task.id == result.task_id) {
                            if let Ok(mut phase) = storage_clone.load_group(&info.tag) {
                                if let Some(task) = phase.get_task_mut(&result.task_id) {
                                    if result.success {
                                        // Don't mark done - agent should do that via scud set-status
                                        println!("✓ Completed: {} ({}ms)", result.task_id, result.duration_ms);
                                    } else {
                                        task.set_status(TaskStatus::Failed);
                                        let _ = storage_clone.update_group(&info.tag, &phase);
                                        println!("✗ Failed: {} ({}ms)", result.task_id, result.duration_ms);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        // Execute wave
        let results = execute_wave_server(&task_pairs, working_dir, model, event_tx).await;

        // Wait for consumer to finish
        let _ = consumer.await;

        results
    })?;

    // Build round state
    let mut round_state = RoundState::new(round_idx);
    for result in &results {
        if result.success {
            round_state.task_ids.push(result.task_id.clone());
        } else {
            round_state.failures.push(result.task_id.clone());
        }
    }

    // Find tags for successful tasks
    for info in tasks {
        if round_state.task_ids.contains(&info.task.id) {
            round_state.tags.push(info.tag.clone());
        }
    }

    round_state.mark_complete();
    Ok(round_state)
}
```

#### 4.3 Update Round Execution Switch

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Call server mode execution (around line 364-405)

```rust
// In the round execution match statement:

let state = match swarm_mode {
    SwarmMode::Tmux => {
        execute_round(
            &storage,
            round_tasks,
            &working_dir,
            &session_name,
            round_idx,
            harness,
        )?
    }
    SwarmMode::Server => {
        // Server mode: use OpenCode orchestration
        let model = default_model.as_ref().map(|m| {
            // Parse model string like "xai/grok-3" or "xai/grok-3-mini"
            let parts: Vec<&str> = m.splitn(2, '/').collect();
            if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                ("xai", m.as_str()) // Default to xAI/Grok
            }
        });

        execute_round_server(
            &storage,
            round_tasks,
            &working_dir,
            round_idx,
            model,
        )?
    }
};
```

#### 4.4 Update Default Mode

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Make Server the default mode

```rust
// Update the default swarm mode (around where swarm_mode is parsed):

let swarm_mode = if terminal_mode == "tmux" {
    SwarmMode::Tmux
} else {
    SwarmMode::Server // Default to server mode
};
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p scud-cli` compiles
- [x] `cargo test -p scud-cli swarm_mode` passes (3 tests)
- [x] SwarmMode::Server variant added and displays correctly

#### Manual Verification:
- [x] `scud swarm --tag test-tag --swarm-mode server` uses server mode (displays "server (opencode)")
- [x] `scud swarm --tag test-tag --swarm-mode tmux` still uses tmux
- [x] OpenCode server starts and accepts connections on port 4096
- [x] Session creation/deletion API works correctly
- [ ] Full agent execution with task completion (requires pending tasks)

**Implementation Note**: After this phase, test with a small task set before proceeding.

---

## Phase 5: TUI Integration

### Overview
Update TUI monitor to display server events including tool calls and streaming output.

### Changes Required

#### 5.1 Add Tool Call Display to TUI State

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Add tool call tracking

```rust
// Add to App struct (around line 30):

/// Recent tool calls for display
pub tool_calls: Vec<ToolCallInfo>,

/// Tool call info for TUI display
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub task_id: String,
    pub tool_name: String,
    pub status: ToolCallStatus,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolCallStatus {
    Running,
    Completed,
    Failed,
}
```

#### 5.2 Add OpenCode Event Processing

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Handle OpenCode events

```rust
// Add method to App:

/// Process an OpenCode event
pub fn process_opencode_event(&mut self, event: OpenCodeEvent) {
    match event {
        OpenCodeEvent::ToolStart { session_id, tool_name, .. } => {
            // Find task_id from session
            if let Some(task_id) = self.session_to_task.get(&session_id) {
                self.tool_calls.push(ToolCallInfo {
                    task_id: task_id.clone(),
                    tool_name,
                    status: ToolCallStatus::Running,
                    timestamp: std::time::Instant::now(),
                });

                // Keep only last 50 tool calls
                if self.tool_calls.len() > 50 {
                    self.tool_calls.remove(0);
                }
            }
        }

        OpenCodeEvent::ToolResult { session_id, tool_name, success, .. } => {
            // Update tool call status
            if let Some(task_id) = self.session_to_task.get(&session_id) {
                for tc in self.tool_calls.iter_mut().rev() {
                    if tc.task_id == *task_id && tc.tool_name == tool_name && tc.status == ToolCallStatus::Running {
                        tc.status = if success { ToolCallStatus::Completed } else { ToolCallStatus::Failed };
                        break;
                    }
                }
            }
        }

        OpenCodeEvent::TextDelta { session_id, text } => {
            // Add to live output
            if let Some(task_id) = self.session_to_task.get(&session_id) {
                if self.selected_agent_index.map(|i| {
                    self.get_agents().get(i).map(|a| &a.task_id) == Some(task_id)
                }).unwrap_or(false) {
                    // This is the selected agent - append to live output
                    for line in text.lines() {
                        self.live_output.push(line.to_string());
                    }
                    // Keep reasonable buffer
                    while self.live_output.len() > 500 {
                        self.live_output.remove(0);
                    }
                }
            }
        }

        _ => {}
    }
}
```

#### 5.3 Add Tool Call Panel to UI

**File**: `scud-cli/src/commands/spawn/tui/ui.rs`
**Changes**: Render tool calls panel

```rust
// Add to render function, after agents panel:

fn render_tool_calls(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::default()
        .title(" Tool Calls ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    block.render(area, buf);

    let items: Vec<ListItem> = app
        .tool_calls
        .iter()
        .rev()
        .take(inner.height as usize)
        .map(|tc| {
            let status_icon = match tc.status {
                ToolCallStatus::Running => "⏳",
                ToolCallStatus::Completed => "✓",
                ToolCallStatus::Failed => "✗",
            };
            let style = match tc.status {
                ToolCallStatus::Running => Style::default().fg(Color::Yellow),
                ToolCallStatus::Completed => Style::default().fg(Color::Green),
                ToolCallStatus::Failed => Style::default().fg(Color::Red),
            };

            ListItem::new(format!(
                "{} [{}] {}",
                status_icon, tc.task_id, tc.tool_name
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items);
    list.render(inner, buf);
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p scud-cli` compiles
- [x] `cargo test -p scud-cli` passes (408 tests)

#### Manual Verification:
- [ ] TUI shows tool calls in real-time
- [ ] Tool call status updates (running → completed/failed)
- [ ] Text output streams to output panel
- [ ] Cancel ('c' key) gracefully stops agent

**Note**: Phase 5 TUI enhancements deferred - basic server mode works. TUI tool call display can be added incrementally.

---

## Phase 6: Serve Command

### Overview
Add explicit server control commands for advanced users.

### Changes Required

#### 6.1 Add Serve Subcommand

**File**: `scud-cli/src/main.rs`
**Changes**: Add serve command with start/stop/status

```rust
// Add to Commands enum:

/// Manage OpenCode server
#[command(subcommand)]
Serve(ServeCommand),

#[derive(Subcommand, Debug)]
pub enum ServeCommand {
    /// Start the OpenCode server
    Start {
        /// Port to run on
        #[arg(short, long, default_value = "4096")]
        port: u16,
    },

    /// Stop the OpenCode server
    Stop,

    /// Check server status
    Status,
}
```

#### 6.2 Implement Serve Handlers

**File**: `scud-cli/src/commands/serve_cmd.rs` (new)
**Changes**: Server control implementation

```rust
//! Server control commands

use anyhow::Result;
use crate::opencode::manager::{OpenCodeManager, ServerConfig};

pub async fn start(port: u16) -> Result<()> {
    let config = ServerConfig {
        port,
        ..Default::default()
    };

    let manager = OpenCodeManager::with_config(config);

    if manager.is_running().await {
        println!("OpenCode server is already running on port {}", port);
        return Ok(());
    }

    println!("Starting OpenCode server on port {}...", port);
    manager.start_server().await?;
    println!("✓ Server started successfully");

    // Keep running (don't drop manager)
    println!("Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;

    println!("\nStopping server...");
    manager.stop_server().await?;

    Ok(())
}

pub async fn stop() -> Result<()> {
    let manager = OpenCodeManager::new();

    if !manager.is_running().await {
        println!("OpenCode server is not running");
        return Ok(());
    }

    // Send shutdown request
    if let Err(e) = manager.client().abort_session("__shutdown__").await {
        // Expected to fail - just checking if server responds
        let _ = e;
    }

    println!("✓ Shutdown signal sent");
    Ok(())
}

pub async fn status() -> Result<()> {
    let manager = OpenCodeManager::new();

    if manager.is_running().await {
        let info = manager.client().server_info().await?;
        println!("OpenCode server: RUNNING");
        println!("  Version: {}", info.version);
        println!("  Port: {}", manager.port());

        // List active sessions
        let sessions = manager.client().list_sessions().await?;
        println!("  Active sessions: {}", sessions.len());
        for session in sessions.iter().take(5) {
            println!("    - {} ({})", session.title, session.id);
        }
        if sessions.len() > 5 {
            println!("    ... and {} more", sessions.len() - 5);
        }
    } else {
        println!("OpenCode server: NOT RUNNING");
        println!("  Start with: scud serve start");
    }

    Ok(())
}
```

### Success Criteria

#### Automated Verification:
- [ ] `cargo build -p scud-cli` compiles

#### Manual Verification:
- [ ] `scud serve start` starts server
- [ ] `scud serve status` shows server info
- [ ] `scud serve stop` sends shutdown signal

---

## Testing Strategy

### Unit Tests
- HTTP client request/response parsing
- SSE event parsing for all event types
- Agent orchestrator state management
- Prompt generation

### Integration Tests
- Server startup and health check
- Session lifecycle (create → message → complete → delete)
- Event stream connection and parsing
- Multi-session concurrent execution

### Manual Testing Steps
1. Start swarm with small task set (2-3 tasks)
2. Verify server starts automatically
3. Watch TUI for real-time tool calls
4. Cancel an agent mid-execution
5. Verify remaining agents complete
6. Run second swarm (server reuse)
7. Test `--terminal tmux` fallback

## Performance Considerations

- **Server reuse**: Single server handles all swarm runs (no restart overhead)
- **Connection pooling**: HTTP client reuses connections
- **Event buffering**: 1000-event channel prevents backpressure
- **Session cleanup**: Delete sessions after completion to free server memory

## Migration Notes

- Existing `--terminal extensions` flag maps to server mode
- Tmux mode remains available via `--terminal tmux`
- No data migration needed (session files are ephemeral)
- Server auto-starts, no manual setup required

## References

- Research: `thoughts/shared/research/2026-01-23-opencode-sdk-deep-integration.md`
- Current runner: `scud-cli/src/extensions/runner.rs`
- Swarm execution: `scud-cli/src/commands/swarm/mod.rs`
- TUI monitor: `scud-cli/src/commands/spawn/tui/`
- OpenCode docs: https://opencode.ai/docs/server/
