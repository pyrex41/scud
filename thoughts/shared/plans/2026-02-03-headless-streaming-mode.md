# Headless Streaming Mode Implementation Plan

## Overview

Add headless streaming mode to SCUD spawn/swarm that uses CLI streaming output instead of tmux, enabling structured JSON event streams, native session continuation, and better integration with GUI layers.

## Current State Analysis

The current spawn system uses **tmux** as the execution layer:
- `terminal.rs` spawns agents in tmux windows via `spawn_terminal_with_harness_and_model()`
- TUI (`app.rs`) polls tmux panes via `tmux capture-pane` every 500ms
- Output is scraped terminal content, not structured events

**Existing Infrastructure:**
- OpenCode server mode already exists in `swarm/mod.rs:1334-1448` using `AgentOrchestrator`
- SSE event streaming is implemented in `opencode/events.rs`
- `SwarmMode` enum already has `Tmux`, `Extensions`, `Server`, and `Beads` variants

### Key Discoveries:
- `scud-cli/src/commands/spawn/terminal.rs:183-378` - Current tmux spawning logic
- `scud-cli/src/commands/spawn/tui/app.rs:262-329` - TUI output refresh via tmux capture-pane
- `scud-cli/src/opencode/orchestrator.rs` - Existing OpenCode agent orchestration
- `scud-cli/src/commands/swarm/mod.rs:1334-1448` - Server mode execution (OpenCode)
- `descartes-gui/src/scud_bridge.rs:419-491` - GUI swarm execution via subprocess

## Desired End State

After this plan is complete:

1. `scud spawn --headless` runs agents without tmux, streaming JSON events
2. `scud monitor` displays output from StreamStore (not tmux panes)
3. Users can interrupt a headless agent and continue interactively via `--resume`
4. Descartes GUI can directly consume streaming events without subprocess
5. Both Claude Code and OpenCode harnesses support headless mode

### Verification:
- `scud spawn --headless --limit 1` spawns agent without tmux dependency
- `scud monitor` shows live streaming output in TUI
- `scud attach <session_id>` launches interactive session to continue
- All existing tmux-based functionality continues to work (backward compatible)

## What We're NOT Doing

- Removing tmux support (it remains the default for now)
- Changing the task/phase data model
- Modifying the backpressure validation system
- Adding new AI harnesses beyond Claude/OpenCode
- Persisting full event streams to disk (in-memory only for Phase 1)

## Implementation Approach

Build a `HeadlessRunner` abstraction that works for both Claude Code (`claude -p --output-format stream-json`) and OpenCode (`opencode serve` + HTTP API). Store streaming output in a `StreamStore` that the TUI/GUI can read from instead of polling tmux.

---

## Phase 1: StreamStore Core Infrastructure

### Overview
Create the shared output storage and event types for headless agent execution.

### Changes Required:

#### 1. New Module: `scud-cli/src/commands/spawn/headless/mod.rs`
**File**: `scud-cli/src/commands/spawn/headless/mod.rs`
**Changes**: Create new headless module with StreamStore and event types

```rust
//! Headless agent execution with streaming output
//!
//! Provides infrastructure for running agents without tmux,
//! capturing structured JSON events for display in TUI/GUI.

pub mod events;
pub mod runner;
pub mod store;

pub use events::{StreamEvent, StreamEventKind};
pub use runner::{HeadlessRunner, SessionHandle};
pub use store::{SessionStream, StreamStore};
```

#### 2. Stream Events: `scud-cli/src/commands/spawn/headless/events.rs`
**File**: `scud-cli/src/commands/spawn/headless/events.rs`
**Changes**: Define streaming event types

```rust
//! Streaming event types for headless execution

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// A streaming event from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Timestamp when event was received
    pub timestamp_ms: u64,
    /// The event kind
    pub kind: StreamEventKind,
}

/// Types of streaming events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventKind {
    /// Text output delta
    TextDelta { text: String },
    
    /// Tool execution started
    ToolStart {
        tool_name: String,
        tool_id: String,
        input_summary: String,
    },
    
    /// Tool execution completed
    ToolResult {
        tool_name: String,
        tool_id: String,
        success: bool,
    },
    
    /// Agent completed successfully
    Complete { success: bool },
    
    /// Agent encountered an error
    Error { message: String },
    
    /// Session ID assigned (for continuation)
    SessionAssigned { session_id: String },
}

impl StreamEvent {
    pub fn new(kind: StreamEventKind) -> Self {
        Self {
            timestamp_ms: 0, // Will be set by store
            kind,
        }
    }
    
    pub fn text_delta(text: impl Into<String>) -> Self {
        Self::new(StreamEventKind::TextDelta { text: text.into() })
    }
    
    pub fn tool_start(name: &str, id: &str, input: &str) -> Self {
        Self::new(StreamEventKind::ToolStart {
            tool_name: name.to_string(),
            tool_id: id.to_string(),
            input_summary: input.to_string(),
        })
    }
    
    pub fn complete(success: bool) -> Self {
        Self::new(StreamEventKind::Complete { success })
    }
    
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(StreamEventKind::Error { message: message.into() })
    }
}
```

#### 3. Stream Store: `scud-cli/src/commands/spawn/headless/store.rs`
**File**: `scud-cli/src/commands/spawn/headless/store.rs`
**Changes**: Implement in-memory storage for streaming output

```rust
//! In-memory storage for streaming agent output

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::events::{StreamEvent, StreamEventKind};

/// Status of a headless session
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Stream data for a single agent session
#[derive(Debug)]
pub struct SessionStream {
    /// Unique session ID (from harness)
    pub session_id: String,
    /// Associated task ID
    pub task_id: String,
    /// Tag/phase
    pub tag: String,
    /// All events received
    pub events: Vec<StreamEvent>,
    /// Rendered output lines (for display)
    pub output_lines: Vec<String>,
    /// Current status
    pub status: SessionStatus,
    /// When the session started
    pub started_at: Instant,
    /// Process ID (for interruption)
    pub pid: Option<u32>,
}

impl SessionStream {
    pub fn new(task_id: &str, tag: &str) -> Self {
        Self {
            session_id: String::new(),
            task_id: task_id.to_string(),
            tag: tag.to_string(),
            events: Vec::new(),
            output_lines: Vec::new(),
            status: SessionStatus::Starting,
            started_at: Instant::now(),
            pid: None,
        }
    }
    
    /// Add an event and update output lines
    pub fn push_event(&mut self, mut event: StreamEvent) {
        event.timestamp_ms = self.started_at.elapsed().as_millis() as u64;
        
        // Update output lines based on event
        match &event.kind {
            StreamEventKind::TextDelta { text } => {
                // Append text, handling newlines
                for line in text.lines() {
                    if let Some(last) = self.output_lines.last_mut() {
                        if !last.ends_with('\n') {
                            last.push_str(line);
                            continue;
                        }
                    }
                    self.output_lines.push(line.to_string());
                }
            }
            StreamEventKind::ToolStart { tool_name, input_summary, .. } => {
                self.output_lines.push(format!(">> {} {}", tool_name, input_summary));
            }
            StreamEventKind::ToolResult { tool_name, success, .. } => {
                let status = if *success { "ok" } else { "failed" };
                self.output_lines.push(format!("<< {} {}", tool_name, status));
            }
            StreamEventKind::Complete { success } => {
                self.status = if *success {
                    SessionStatus::Completed
                } else {
                    SessionStatus::Failed
                };
            }
            StreamEventKind::Error { message } => {
                self.output_lines.push(format!("ERROR: {}", message));
                self.status = SessionStatus::Failed;
            }
            StreamEventKind::SessionAssigned { session_id } => {
                self.session_id = session_id.clone();
                self.status = SessionStatus::Running;
            }
        }
        
        self.events.push(event);
    }
    
    /// Get the last N output lines
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.output_lines.len().saturating_sub(n);
        &self.output_lines[start..]
    }
    
    /// Check if session is still active
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Starting | SessionStatus::Running)
    }
}

/// Thread-safe store for multiple agent sessions
#[derive(Debug, Clone, Default)]
pub struct StreamStore {
    sessions: Arc<RwLock<HashMap<String, SessionStream>>>,
}

impl StreamStore {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a new session for a task
    pub fn create_session(&self, task_id: &str, tag: &str) -> String {
        let mut sessions = self.sessions.write().unwrap();
        let stream = SessionStream::new(task_id, tag);
        let key = task_id.to_string();
        sessions.insert(key.clone(), stream);
        key
    }
    
    /// Push an event to a session
    pub fn push_event(&self, task_id: &str, event: StreamEvent) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.push_event(event);
        }
    }
    
    /// Set the harness session ID for a task
    pub fn set_session_id(&self, task_id: &str, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.session_id = session_id.to_string();
            stream.status = SessionStatus::Running;
        }
    }
    
    /// Set the process ID for a task
    pub fn set_pid(&self, task_id: &str, pid: u32) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.pid = Some(pid);
        }
    }
    
    /// Get output lines for a task
    pub fn get_output(&self, task_id: &str, limit: usize) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(task_id)
            .map(|s| s.tail(limit).to_vec())
            .unwrap_or_default()
    }
    
    /// Get session status
    pub fn get_status(&self, task_id: &str) -> Option<SessionStatus> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(task_id).map(|s| s.status.clone())
    }
    
    /// Get harness session ID for continuation
    pub fn get_session_id(&self, task_id: &str) -> Option<String> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(task_id).map(|s| s.session_id.clone())
    }
    
    /// List all active task IDs
    pub fn active_tasks(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .iter()
            .filter(|(_, s)| s.is_active())
            .map(|(k, _)| k.clone())
            .collect()
    }
    
    /// Get all task IDs
    pub fn all_tasks(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions.keys().cloned().collect()
    }
}
```

#### 4. Update spawn module exports
**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Add headless module

```rust
// Add after existing module declarations
pub mod headless;
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p scud-cli`
- [ ] Unit tests pass: `cargo test -p scud-cli headless`
- [ ] No clippy warnings: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] StreamStore can be instantiated and used in tests
- [ ] Events are properly timestamped and stored

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to the next phase.

---

## Phase 2: Claude Headless Runner

### Overview
Implement the Claude Code headless runner that spawns `claude -p --output-format stream-json` and parses the streaming output.

### Changes Required:

#### 1. HeadlessRunner Trait: `scud-cli/src/commands/spawn/headless/runner.rs`
**File**: `scud-cli/src/commands/spawn/headless/runner.rs`
**Changes**: Define trait and Claude implementation

```rust
//! Headless runner implementations for different harnesses

use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::events::{StreamEvent, StreamEventKind};
use crate::commands::spawn::terminal::{find_harness_binary, Harness};

/// Handle to a running headless session
pub struct SessionHandle {
    /// Task ID this session is for
    pub task_id: String,
    /// Harness session ID (for continuation)
    pub session_id: Option<String>,
    /// Child process
    child: Child,
    /// Event receiver
    pub events: mpsc::Receiver<StreamEvent>,
}

impl SessionHandle {
    /// Wait for the session to complete
    pub async fn wait(mut self) -> Result<bool> {
        let status = self.child.wait().await?;
        Ok(status.success())
    }
    
    /// Interrupt the session (send SIGINT)
    pub fn interrupt(&mut self) -> Result<()> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            
            if let Some(pid) = self.child.id() {
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGINT);
            }
        }
        Ok(())
    }
    
    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}

/// Trait for headless agent execution
pub trait HeadlessRunner: Send + Sync {
    /// Start an agent with a prompt
    fn start(
        &self,
        task_id: &str,
        prompt: &str,
        working_dir: &Path,
        model: Option<&str>,
    ) -> impl std::future::Future<Output = Result<SessionHandle>> + Send;
    
    /// Get the command to launch interactive mode for session continuation
    fn interactive_command(&self, session_id: &str) -> Vec<String>;
}

/// Claude Code headless runner
pub struct ClaudeHeadless {
    binary_path: String,
    allowed_tools: Vec<String>,
}

impl ClaudeHeadless {
    pub fn new() -> Result<Self> {
        let binary_path = find_harness_binary(Harness::Claude)?.to_string();
        Ok(Self {
            binary_path,
            allowed_tools: vec![
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "Bash".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
            ],
        })
    }
    
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }
}

impl HeadlessRunner for ClaudeHeadless {
    async fn start(
        &self,
        task_id: &str,
        prompt: &str,
        working_dir: &Path,
        model: Option<&str>,
    ) -> Result<SessionHandle> {
        let mut cmd = Command::new(&self.binary_path);
        
        // Core headless flags
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--verbose");
        cmd.arg("--dangerously-skip-permissions");
        
        // Model selection
        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }
        
        // Allowed tools
        if !self.allowed_tools.is_empty() {
            cmd.arg("--allowedTools").arg(self.allowed_tools.join(","));
        }
        
        // Working directory
        cmd.current_dir(working_dir);
        
        // Capture stdout for streaming
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        
        // Create event channel
        let (tx, rx) = mpsc::channel(1000);
        
        // Spawn task to read stdout and parse events
        let stdout = child.stdout.take().expect("stdout was piped");
        let task_id_clone = task_id.to_string();
        
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(event) = parse_claude_event(&line) {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            
            // Send completion event
            let _ = tx.send(StreamEvent::complete(true)).await;
        });
        
        Ok(SessionHandle {
            task_id: task_id.to_string(),
            session_id: None, // Will be set when we parse session_id from events
            child,
            events: rx,
        })
    }
    
    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        vec![
            self.binary_path.clone(),
            "--resume".to_string(),
            session_id.to_string(),
        ]
    }
}

/// Parse a line of Claude stream-json output into a StreamEvent
fn parse_claude_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;
    
    let event_type = json.get("type")?.as_str()?;
    
    match event_type {
        "stream_event" => {
            // Check for text delta
            if let Some(delta) = json.pointer("/event/delta") {
                if delta.get("type")?.as_str()? == "text_delta" {
                    let text = delta.get("text")?.as_str()?;
                    return Some(StreamEvent::text_delta(text));
                }
            }
            None
        }
        "tool_use" => {
            let tool_name = json.get("name")?.as_str()?;
            let tool_id = json.get("id")?.as_str().unwrap_or("unknown");
            let input = json.get("input").cloned().unwrap_or(serde_json::Value::Null);
            let input_summary = summarize_json(&input);
            Some(StreamEvent::tool_start(tool_name, tool_id, &input_summary))
        }
        "tool_result" => {
            let tool_id = json.get("tool_use_id")?.as_str().unwrap_or("unknown");
            let success = !json.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
            Some(StreamEvent::new(StreamEventKind::ToolResult {
                tool_name: String::new(), // Not always available
                tool_id: tool_id.to_string(),
                success,
            }))
        }
        "result" => {
            let session_id = json.get("session_id")?.as_str()?;
            Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                session_id: session_id.to_string(),
            }))
        }
        "error" => {
            let message = json.get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error");
            Some(StreamEvent::error(message))
        }
        _ => None,
    }
}

/// Summarize JSON input for display
fn summarize_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(obj) => {
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

/// OpenCode headless runner (wraps existing server mode)
pub struct OpenCodeHeadless {
    binary_path: String,
}

impl OpenCodeHeadless {
    pub fn new() -> Result<Self> {
        let binary_path = find_harness_binary(Harness::OpenCode)?.to_string();
        Ok(Self { binary_path })
    }
}

impl HeadlessRunner for OpenCodeHeadless {
    async fn start(
        &self,
        task_id: &str,
        prompt: &str,
        working_dir: &Path,
        model: Option<&str>,
    ) -> Result<SessionHandle> {
        // OpenCode uses `run` command with streaming
        let mut cmd = Command::new(&self.binary_path);
        
        cmd.arg("run");
        cmd.arg("--format").arg("json");
        
        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }
        
        cmd.arg(prompt);
        cmd.current_dir(working_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        let (tx, rx) = mpsc::channel(1000);
        
        let stdout = child.stdout.take().expect("stdout was piped");
        
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(event) = parse_opencode_event(&line) {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            
            let _ = tx.send(StreamEvent::complete(true)).await;
        });
        
        Ok(SessionHandle {
            task_id: task_id.to_string(),
            session_id: None,
            child,
            events: rx,
        })
    }
    
    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        // OpenCode uses attach command
        vec![
            self.binary_path.clone(),
            "attach".to_string(),
            format!("http://localhost:4096"),
            "--session".to_string(),
            session_id.to_string(),
        ]
    }
}

/// Parse OpenCode JSON event
fn parse_opencode_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;
    
    // OpenCode uses different event structure
    let event_type = json.get("type")?.as_str()?;
    
    match event_type {
        "assistant" => {
            let text = json.pointer("/message/content/0/text")?.as_str()?;
            Some(StreamEvent::text_delta(text))
        }
        "tool_call" => {
            let subtype = json.get("subtype")?.as_str()?;
            if subtype == "started" {
                // Extract tool info
                let tool_name = json.pointer("/tool_call/name")
                    .or_else(|| json.pointer("/tool_call/writeToolCall"))
                    .map(|_| "write")
                    .or_else(|| json.pointer("/tool_call/readToolCall").map(|_| "read"))
                    .unwrap_or("unknown");
                Some(StreamEvent::tool_start(tool_name, "", ""))
            } else if subtype == "completed" {
                Some(StreamEvent::new(StreamEventKind::ToolResult {
                    tool_name: String::new(),
                    tool_id: String::new(),
                    success: true,
                }))
            } else {
                None
            }
        }
        "result" => {
            Some(StreamEvent::complete(true))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_claude_text_delta() {
        let line = r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hello"}}}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(StreamEvent { kind: StreamEventKind::TextDelta { .. }, .. })));
    }
    
    #[test]
    fn test_parse_claude_tool_use() {
        let line = r#"{"type":"tool_use","name":"Read","id":"tool_1","input":{"path":"src/main.rs"}}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(StreamEvent { kind: StreamEventKind::ToolStart { .. }, .. })));
    }
    
    #[test]
    fn test_summarize_json_object() {
        let value = serde_json::json!({"path": "/foo", "content": "bar"});
        let summary = summarize_json(&value);
        assert!(summary.contains("path"));
    }
    
    #[test]
    fn test_summarize_json_long_string() {
        let long = "a".repeat(100);
        let value = serde_json::json!(long);
        let summary = summarize_json(&value);
        assert!(summary.len() < 60);
        assert!(summary.ends_with("...\""));
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p scud-cli`
- [ ] Unit tests pass: `cargo test -p scud-cli runner`
- [ ] No clippy warnings: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] `ClaudeHeadless::new()` finds the claude binary
- [ ] Event parsing correctly handles text deltas and tool calls

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to the next phase.

---

## Phase 3: Integrate with Spawn Command

### Overview
Add `--headless` flag to the spawn command and wire up the headless runner.

### Changes Required:

#### 1. Add headless execution function
**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Add headless spawn function alongside existing tmux spawn

```rust
// Add to imports at top of file
use self::headless::{ClaudeHeadless, HeadlessRunner, OpenCodeHeadless, StreamStore};

/// Spawn agents in headless mode (no tmux)
pub async fn spawn_headless(
    tasks: &[TaskInfo],
    working_dir: &std::path::Path,
    harness: Harness,
    model: Option<&str>,
    store: &StreamStore,
) -> Result<Vec<String>> {
    let runner: Box<dyn HeadlessRunner> = match harness {
        Harness::Claude => Box::new(ClaudeHeadless::new()?),
        Harness::OpenCode => Box::new(OpenCodeHeadless::new()?),
    };
    
    let mut handles = Vec::new();
    let mut task_ids = Vec::new();
    
    for info in tasks {
        // Create session in store
        store.create_session(&info.task.id, &info.tag);
        
        // Generate prompt
        let prompt = agent::generate_prompt(info.task, &info.tag);
        
        // Start headless session
        match runner.start(&info.task.id, &prompt, working_dir, model).await {
            Ok(handle) => {
                if let Some(pid) = handle.pid() {
                    store.set_pid(&info.task.id, pid);
                }
                handles.push(handle);
                task_ids.push(info.task.id.clone());
                println!(
                    "  {} Spawned (headless): {} | {}",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed()
                );
            }
            Err(e) => {
                store.push_event(&info.task.id, headless::StreamEvent::error(e.to_string()));
                println!(
                    "  {} Failed: {} - {}",
                    "✗".red(),
                    info.task.id.red(),
                    e
                );
            }
        }
    }
    
    // Spawn background tasks to collect events
    for mut handle in handles {
        let store = store.clone();
        let task_id = handle.task_id.clone();
        
        tokio::spawn(async move {
            while let Some(event) = handle.events.recv().await {
                // Check for session ID assignment
                if let headless::StreamEventKind::SessionAssigned { ref session_id } = event.kind {
                    store.set_session_id(&task_id, session_id);
                }
                store.push_event(&task_id, event);
            }
        });
    }
    
    Ok(task_ids)
}
```

#### 2. Update main.rs CLI arguments
**File**: `scud-cli/src/main.rs`
**Changes**: Add `--headless` flag to spawn subcommand

Find the spawn subcommand definition and add:

```rust
#[arg(long, help = "Run in headless mode (no tmux, streaming output)")]
headless: bool,
```

#### 3. Update spawn run function
**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Add headless parameter to run function signature and handle it

Add parameter to `run()` function:
```rust
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    limit: usize,
    all_tags: bool,
    dry_run: bool,
    session: Option<String>,
    attach: bool,
    monitor: bool,
    claim: bool,
    harness_arg: &str,
    model_arg: &str,
    headless: bool,  // NEW
) -> Result<()> {
```

Add headless branch after the dry_run check:

```rust
if headless {
    // Use tokio runtime for async headless execution
    let rt = tokio::runtime::Runtime::new()?;
    let store = StreamStore::new();
    
    let task_infos: Vec<_> = ready_tasks.iter().map(|info| TaskInfo {
        task: info.task,
        tag: info.tag.clone(),
    }).collect();
    
    rt.block_on(async {
        spawn_headless(&task_infos, &working_dir, harness, Some(model_arg), &store).await
    })?;
    
    // If monitor requested, start TUI with store
    if monitor {
        // TODO: Phase 4 - pass store to TUI
        println!("Headless monitor not yet implemented");
    }
    
    return Ok(());
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p scud-cli`
- [ ] CLI help shows `--headless` flag: `cargo run -p scud-cli -- spawn --help`
- [ ] No clippy warnings: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] `scud spawn --headless --limit 1 --dry-run` shows headless mode info
- [ ] `scud spawn --headless --limit 1` spawns agent without tmux (requires claude binary)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to the next phase.

---

## Phase 4: Update TUI Monitor

### Overview
Update the TUI monitor to read from StreamStore when in headless mode instead of polling tmux panes.

### Changes Required:

#### 1. Add StreamStore to App state
**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Add optional StreamStore field and update output refresh

Add to App struct:
```rust
/// Stream store for headless mode (None = tmux mode)
pub stream_store: Option<StreamStore>,
```

Add to App::new():
```rust
pub fn new(
    project_root: Option<PathBuf>,
    session_name: &str,
    swarm_mode: bool,
    stream_store: Option<StreamStore>,  // NEW parameter
) -> Result<Self> {
    // ... existing code ...
    
    let mut app = Self {
        // ... existing fields ...
        stream_store,  // NEW
    };
```

#### 2. Update refresh_live_output
**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Check for StreamStore before falling back to tmux

Replace the `refresh_live_output` method:

```rust
/// Refresh live output from the selected agent
pub fn refresh_live_output(&mut self) {
    // If we have a stream store, use it
    if let Some(ref store) = self.stream_store {
        let agents = self.agents();
        if agents.is_empty() || self.selected >= agents.len() {
            self.live_output = vec!["No agent selected".to_string()];
            return;
        }
        
        let agent = &agents[self.selected];
        self.live_output = store.get_output(&agent.task_id, 100);
        
        if self.live_output.is_empty() {
            self.live_output = vec!["Waiting for output...".to_string()];
        }
        
        self.last_output_refresh = Instant::now();
        return;
    }
    
    // Fall back to tmux capture-pane (existing code)
    let agents = self.agents();
    if agents.is_empty() || self.selected >= agents.len() {
        self.live_output = vec!["No agent selected".to_string()];
        return;
    }
    // ... rest of existing tmux code ...
}
```

#### 3. Update TUI run function
**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Accept optional StreamStore

```rust
pub fn run(
    project_root: Option<PathBuf>,
    session_name: &str,
    swarm_mode: bool,
    stream_store: Option<StreamStore>,  // NEW
) -> Result<()> {
    let mut app = App::new(project_root, session_name, swarm_mode, stream_store)?;
    // ... rest of existing code ...
}
```

#### 4. Update spawn monitor call
**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Pass StreamStore to TUI when in headless mode

In the headless branch, update the monitor call:
```rust
if monitor {
    return tui::run(project_root, &session_name, false, Some(store));
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p scud-cli`
- [ ] No clippy warnings: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] TUI displays "Waiting for output..." when no events yet
- [ ] TUI shows streaming output from headless agents
- [ ] TUI still works in tmux mode (backward compatible)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to the next phase.

---

## Phase 5: Session Continuation

### Overview
Add the ability to interrupt a headless session and continue it interactively using `--resume`.

### Changes Required:

#### 1. Add attach command
**File**: `scud-cli/src/main.rs`
**Changes**: Add new `attach` subcommand for session continuation

```rust
/// Attach to a headless session interactively
#[derive(Parser)]
struct AttachArgs {
    /// Task ID to attach to
    task_id: String,
    
    /// Harness to use (claude, opencode)
    #[arg(short = 'H', long, default_value = "claude")]
    harness: String,
}
```

#### 2. Implement attach command
**File**: `scud-cli/src/commands/attach.rs`
**Changes**: Create new attach command module

```rust
//! Attach command - Continue a headless session interactively

use anyhow::Result;
use colored::Colorize;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use crate::commands::spawn::headless::{ClaudeHeadless, HeadlessRunner, OpenCodeHeadless};
use crate::commands::spawn::terminal::Harness;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    harness_arg: &str,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());
    
    // Try to find session ID from stored metadata
    let session_file = storage.root()
        .join(".scud")
        .join("headless")
        .join(format!("{}.json", task_id));
    
    let session_id = if session_file.exists() {
        let content = std::fs::read_to_string(&session_file)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;
        data.get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };
    
    let session_id = session_id.ok_or_else(|| {
        anyhow::anyhow!("No session ID found for task {}. Was it run in headless mode?", task_id)
    })?;
    
    let harness = Harness::parse(harness_arg)?;
    
    let cmd_args: Vec<String> = match harness {
        Harness::Claude => {
            let runner = ClaudeHeadless::new()?;
            runner.interactive_command(&session_id)
        }
        Harness::OpenCode => {
            let runner = OpenCodeHeadless::new()?;
            runner.interactive_command(&session_id)
        }
    };
    
    println!("{}", "Attaching to session...".cyan());
    println!("Session ID: {}", session_id.dimmed());
    println!();
    
    // Replace current process with interactive session
    let err = Command::new(&cmd_args[0])
        .args(&cmd_args[1..])
        .exec();
    
    // exec() only returns on error
    Err(anyhow::anyhow!("Failed to exec: {}", err))
}
```

#### 3. Save session metadata
**File**: `scud-cli/src/commands/spawn/headless/store.rs`
**Changes**: Add method to persist session ID for continuation

```rust
impl StreamStore {
    /// Save session metadata for later continuation
    pub fn save_session_metadata(&self, task_id: &str, project_root: &Path) -> Result<()> {
        let sessions = self.sessions.read().unwrap();
        let session = sessions.get(task_id).ok_or_else(|| {
            anyhow::anyhow!("Session not found: {}", task_id)
        })?;
        
        let metadata_dir = project_root.join(".scud").join("headless");
        std::fs::create_dir_all(&metadata_dir)?;
        
        let metadata = serde_json::json!({
            "task_id": session.task_id,
            "session_id": session.session_id,
            "tag": session.tag,
            "pid": session.pid,
        });
        
        let metadata_file = metadata_dir.join(format!("{}.json", task_id));
        std::fs::write(&metadata_file, serde_json::to_string_pretty(&metadata)?)?;
        
        Ok(())
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p scud-cli`
- [ ] CLI help shows attach command: `cargo run -p scud-cli -- attach --help`
- [ ] No clippy warnings: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] Session metadata is saved to `.scud/headless/<task_id>.json`
- [ ] `scud attach <task_id>` launches interactive claude with `--resume`
- [ ] User can continue working in the resumed session

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to the next phase.

---

## Phase 6: Descartes GUI Integration

### Overview
Update the Descartes GUI to optionally use headless mode directly instead of spawning subprocesses.

### Changes Required:

#### 1. Add headless mode to ScudBridge
**File**: `descartes-gui/src/scud_bridge.rs`
**Changes**: Add headless execution option

```rust
// Add to ScudCommand enum
/// Run task in headless mode
RunTaskHeadless {
    task_id: String,
    harness: String,
},

// Add handler in run() match
ScudCommand::RunTaskHeadless { task_id, harness } => {
    self.run_task_headless(&task_id, &harness).await;
}
```

#### 2. Implement headless task execution
**File**: `descartes-gui/src/scud_bridge.rs`
**Changes**: Add method to run tasks in headless mode

```rust
/// Run a task using headless mode with direct event streaming
async fn run_task_headless(&mut self, task_id: &str, harness: &str) {
    use scud_core::Storage;
    
    // Load task details
    let task_id_clone = task_id.to_string();
    let task_result = tokio::task::spawn_blocking(move || -> Result<Task, String> {
        let storage = Storage::new(None);
        let tag = storage
            .get_active_group()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No active task group".to_string())?;
        let phase = storage.load_group(&tag).map_err(|e| e.to_string())?;
        phase
            .get_task(&task_id_clone)
            .cloned()
            .ok_or_else(|| format!("Task '{}' not found", task_id_clone))
    })
    .await;
    
    let task = match task_result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) | Err(e) => {
            let _ = self.event_tx.send(ScudEvent::Error(e.to_string())).await;
            return;
        }
    };
    
    // Emit task started
    let _ = self.event_tx.send(ScudEvent::TaskStarted {
        task_id: task_id.to_string(),
    }).await;
    
    // Build command for headless execution
    let prompt = format!(
        "Complete task: {}\n\n{}\n\nWhen done: scud set-status {} done",
        task.title, task.description, task_id
    );
    
    // Use scud run with headless flag
    let args = vec![
        "run",
        "--headless",
        "-H", harness,
        &prompt,
    ];
    
    match Command::new("scud")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                let event_tx = self.event_tx.clone();
                let task_id_for_output = task_id.to_string();
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                
                // Stream JSON events directly
                while let Ok(Some(line)) = lines.next_line().await {
                    // Parse as JSON event
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(text) = json.pointer("/kind/TextDelta/text").and_then(|v| v.as_str()) {
                            let _ = event_tx.send(ScudEvent::TaskOutput {
                                task_id: task_id_for_output.clone(),
                                text: text.to_string(),
                            }).await;
                        }
                    }
                }
            }
            
            match child.wait().await {
                Ok(status) => {
                    let _ = self.event_tx.send(ScudEvent::TaskCompleted {
                        task_id: task_id.to_string(),
                        success: status.success(),
                    }).await;
                }
                Err(e) => {
                    let _ = self.event_tx.send(ScudEvent::Error(e.to_string())).await;
                }
            }
        }
        Err(e) => {
            let _ = self.event_tx.send(ScudEvent::Error(e.to_string())).await;
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build -p descartes-gui`
- [ ] No clippy warnings: `cargo clippy -p descartes-gui`

#### Manual Verification:
- [ ] GUI can run tasks in headless mode
- [ ] Streaming output appears in GUI output panel
- [ ] Task completion is properly detected

**Implementation Note**: After completing this phase and all automated verification passes, the headless mode implementation is complete.

---

## Testing Strategy

### Unit Tests:
- StreamStore event handling and output rendering
- Claude event parsing (text deltas, tool calls, errors)
- OpenCode event parsing
- Session metadata serialization

### Integration Tests:
- Spawn with `--headless` flag creates sessions
- TUI reads from StreamStore correctly
- Session continuation via `--resume` works

### Manual Testing Steps:
1. Run `scud spawn --headless --limit 1` with a simple task
2. Verify streaming output appears (no tmux window created)
3. Interrupt with Ctrl+C
4. Run `scud attach <task_id>` to continue interactively
5. Verify work continues from where it left off
6. Run `scud spawn --limit 1` (without --headless) to verify tmux mode still works

## Performance Considerations

- StreamStore uses `Arc<RwLock<>>` for thread-safe access
- Events are stored in memory (no disk I/O during streaming)
- Output lines are capped at reasonable limits to prevent memory bloat
- TUI refresh interval remains 500ms (same as tmux polling)

## Migration Notes

- Headless mode is opt-in via `--headless` flag
- Tmux mode remains the default for backward compatibility
- Existing spawn sessions continue to work unchanged
- Session metadata stored in `.scud/headless/` (gitignored)

## References

- Research document: `thoughts/shared/research/2026-02-03-headless-mode-architecture.md`
- Claude Code headless docs: https://docs.anthropic.com/en/docs/claude-code/headless
- OpenCode CLI docs: https://opencode.ai/docs/cli/
- Current spawn implementation: `scud-cli/src/commands/spawn/terminal.rs`
- Current TUI implementation: `scud-cli/src/commands/spawn/tui/app.rs`
- Existing OpenCode integration: `scud-cli/src/opencode/`
