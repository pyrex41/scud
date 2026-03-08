//! Headless runner implementations for different harnesses
//!
//! This module provides the `HeadlessRunner` trait and implementations for running
//! AI coding agents (Claude Code and OpenCode) in headless/non-interactive mode,
//! parsing their streaming JSON output into unified `StreamEvent` types.

use anyhow::Result;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, trace};

use super::events::{StreamEvent, StreamEventKind};
use crate::commands::spawn::terminal::{find_harness_binary, Harness};

/// Boxed async result type for dyn-compatible async trait methods
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Process backing a headless session - either an OS child process or a tokio task.
///
/// Call [`kill()`](SessionProcess::kill) to terminate the process.
pub enum SessionProcess {
    /// Standard child process (Claude, OpenCode, Cursor harnesses)
    Child(Child),
    /// Tokio task (direct API runner)
    #[cfg(feature = "direct-api")]
    Task(tokio::task::JoinHandle<()>),
}

impl SessionProcess {
    /// Kill the backing process immediately.
    pub fn kill(&mut self) -> Result<()> {
        match self {
            SessionProcess::Child(child) => {
                child.start_kill()?;
                Ok(())
            }
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(handle) => {
                handle.abort();
                Ok(())
            }
        }
    }

    /// Wait for the backing process/task to complete.
    pub async fn wait(&mut self) -> Result<bool> {
        match self {
            SessionProcess::Child(child) => {
                let status = child.wait().await?;
                Ok(status.success())
            }
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(handle) => {
                let _ = handle.await;
                Ok(true)
            }
        }
    }
}

/// Handle to a running headless session
pub struct SessionHandle {
    /// Task ID this session is for
    pub task_id: String,
    /// Harness session ID (for continuation)
    pub session_id: Option<String>,
    /// Backing process or task
    process: SessionProcess,
    /// Event receiver
    pub events: mpsc::Receiver<StreamEvent>,
}

impl SessionHandle {
    /// Create a SessionHandle backed by a child process
    pub fn from_child(task_id: String, child: Child, events: mpsc::Receiver<StreamEvent>) -> Self {
        Self {
            task_id,
            session_id: None,
            process: SessionProcess::Child(child),
            events,
        }
    }

    /// Create a SessionHandle backed by a tokio task
    #[cfg(feature = "direct-api")]
    pub fn from_task(
        task_id: String,
        events: mpsc::Receiver<StreamEvent>,
        handle: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self {
            task_id,
            session_id: None,
            process: SessionProcess::Task(handle),
            events,
        }
    }

    /// Wait for the session to complete
    pub async fn wait(self) -> Result<bool> {
        match self.process {
            SessionProcess::Child(mut child) => {
                let status = child.wait().await?;
                Ok(status.success())
            }
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(handle) => {
                let _ = handle.await;
                Ok(true)
            }
        }
    }

    /// Interrupt the session (send SIGINT)
    pub fn interrupt(&mut self) -> Result<()> {
        match &mut self.process {
            SessionProcess::Child(child) => {
                #[cfg(unix)]
                {
                    if let Some(pid) = child.id() {
                        // Send SIGINT using kill command (avoids nix dependency)
                        let _ = std::process::Command::new("kill")
                            .arg("-INT")
                            .arg(pid.to_string())
                            .status();
                    }
                }

                #[cfg(not(unix))]
                {
                    // On non-Unix, just kill the process
                    let _ = child.start_kill();
                }

                Ok(())
            }
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(handle) => {
                handle.abort();
                Ok(())
            }
        }
    }

    /// Kill the session immediately
    pub fn kill(&mut self) -> Result<()> {
        match &mut self.process {
            SessionProcess::Child(child) => {
                child.start_kill()?;
                Ok(())
            }
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(handle) => {
                handle.abort();
                Ok(())
            }
        }
    }

    /// Decompose the session into its event receiver and a killable process handle.
    ///
    /// This allows the caller to own the events stream and process separately,
    /// e.g. for bridging events to a different channel while retaining the
    /// ability to kill the subprocess on cancellation.
    pub fn into_parts(self) -> (mpsc::Receiver<StreamEvent>, SessionProcess) {
        (self.events, self.process)
    }

    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        match &self.process {
            SessionProcess::Child(child) => child.id(),
            #[cfg(feature = "direct-api")]
            SessionProcess::Task(_) => None,
        }
    }
}

/// Trait for headless agent execution
///
/// Implementations provide the ability to start agents in headless mode
/// with streaming JSON output, and to generate commands for interactive
/// session continuation.
///
/// This trait uses boxed futures to be dyn-compatible, allowing runtime
/// polymorphism via `Box<dyn HeadlessRunner>`.
pub trait HeadlessRunner: Send + Sync {
    /// Start an agent with a prompt
    ///
    /// Returns a SessionHandle that can be used to receive events,
    /// wait for completion, or interrupt the session.
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>>;

    /// Get the command to launch interactive mode for session continuation
    ///
    /// Returns the command and arguments needed to resume the session
    /// interactively (e.g., `claude --resume <session_id>`).
    fn interactive_command(&self, session_id: &str) -> Vec<String>;

    /// Get the harness type this runner supports
    fn harness(&self) -> Harness;
}

/// Claude Code headless runner
///
/// Runs Claude Code in headless mode with `--output-format stream-json`,
/// parsing the streaming JSON events for display in TUI/GUI.
pub struct ClaudeHeadless {
    binary_path: String,
    allowed_tools: Vec<String>,
}

impl ClaudeHeadless {
    /// Create a new Claude headless runner
    ///
    /// Finds the Claude binary in PATH or common installation locations.
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

    /// Create with an explicit binary path (useful for testing)
    #[cfg(test)]
    pub fn with_binary_path(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
            allowed_tools: vec![],
        }
    }

    /// Set the allowed tools for this runner
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    /// Get the binary path
    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }
}

impl HeadlessRunner for ClaudeHeadless {
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>> {
        Box::pin(async move {
            let mut cmd = Command::new(&self.binary_path);

            // Core headless flags
            cmd.arg("-p").arg(prompt);
            cmd.arg("--output-format").arg("stream-json");
            cmd.arg("--verbose");
            cmd.arg("--include-partial-messages");
            cmd.arg("--dangerously-skip-permissions");

            // Model selection
            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }

            // Allowed tools
            if !self.allowed_tools.is_empty() {
                cmd.arg("--allowedTools").arg(self.allowed_tools.join(","));
            }

            // Working directory and environment
            cmd.current_dir(working_dir);
            cmd.env("SCUD_TASK_ID", task_id);
            // Allow nested Claude sessions (e.g. attractor pipelines from within Claude Code)
            cmd.env_remove("CLAUDECODE");

            // Capture stdout for streaming
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn()?;

            // Create event channel
            let (tx, rx) = mpsc::channel(1000);

            // Spawn task to read stdout and parse events
            let stdout = child.stdout.take().expect("stdout was piped");
            let task_id_clone = task_id.to_string();
            let task_id_for_events = task_id.to_string();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(event) = parse_claude_event(&line) {
                        trace!(task_id = %task_id_for_events, "claude event: {:?}", event.kind);
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    } else if !line.trim().is_empty() {
                        debug!(task_id = %task_id_for_events, "claude: unparsed line: {}", if line.len() > 200 { &line[..200] } else { &line });
                    }
                }

                // Send completion event
                let _ = tx.send(StreamEvent::complete(true)).await;
            });

            Ok(SessionHandle::from_child(task_id_clone, child, rx))
        })
    }

    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        vec![
            self.binary_path.clone(),
            "--resume".to_string(),
            session_id.to_string(),
        ]
    }

    fn harness(&self) -> Harness {
        Harness::Claude
    }
}

/// Parse a line of Claude stream-json output into a StreamEvent
fn parse_claude_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let event_type = json.get("type")?.as_str()?;

    match event_type {
        "system" => {
            // System init event carries session_id (same pattern as Cursor parser)
            let session_id = json.get("session_id").and_then(|v| v.as_str())?;
            Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                session_id: session_id.to_string(),
            }))
        }
        "stream_event" => {
            // Check for text delta (streaming incremental tokens)
            if let Some(delta) = json.pointer("/event/delta") {
                if delta.get("type")?.as_str()? == "text_delta" {
                    let text = delta.get("text")?.as_str()?;
                    return Some(StreamEvent::text_delta(text));
                }
            }
            None
        }
        "content_block_delta" => {
            // Streaming delta format (incremental tokens)
            if let Some(text) = json.pointer("/delta/text").and_then(|v| v.as_str()) {
                return Some(StreamEvent::text_delta(text));
            }
            None
        }
        "assistant" => {
            // "assistant" events are full message snapshots (not incremental deltas).
            // When --include-partial-messages is used, these duplicate text already
            // captured from "stream_event" deltas. Skip them to avoid double-counting.
            None
        }
        "tool_use" => {
            let tool_name = json.get("name")?.as_str()?;
            let tool_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let input = json
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let input_summary = summarize_json(&input);
            Some(StreamEvent::tool_start(tool_name, tool_id, &input_summary))
        }
        "tool_result" => {
            let tool_id = json
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let success = !json
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(StreamEvent::new(StreamEventKind::ToolResult {
                tool_name: String::new(), // Not always available
                tool_id: tool_id.to_string(),
                success,
            }))
        }
        "result" => {
            // If the result carries a session_id, emit SessionAssigned
            if let Some(session_id) = json.get("session_id").and_then(|v| v.as_str()) {
                return Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                    session_id: session_id.to_string(),
                }));
            }
            let is_error = json
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // The "result" field contains the full text, but this duplicates
            // what was already captured from "assistant" or "stream_event" events.
            // We only signal completion here.
            Some(StreamEvent::complete(!is_error))
        }
        "error" => {
            let message = json
                .get("error")
                .and_then(|e| e.as_str())
                .or_else(|| json.get("message").and_then(|e| e.as_str()))
                .unwrap_or("Unknown error");
            Some(StreamEvent::error(message))
        }
        _ => None,
    }
}

/// Parse a line of Cursor Agent CLI stream-json output into a StreamEvent
///
/// Cursor Agent CLI (`agent -p --output-format stream-json`) outputs newline-delimited
/// JSON events with the following structure:
///
/// - `{"type":"system","subtype":"init","session_id":"..."}` - Session init
/// - `{"type":"tool_call","subtype":"started","call_id":"...","tool_call":{"editToolCall":{...}}}` - Tool start
/// - `{"type":"tool_call","subtype":"completed","call_id":"...","tool_call":{...}}` - Tool result
/// - `{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"..."}]}}` - Text output
/// - `{"type":"result","subtype":"success","is_error":false,"result":"...","session_id":"..."}` - Completion
/// - `{"type":"user",...}` - User message echo (ignored)
fn parse_cursor_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = json.get("type")?.as_str()?;

    match event_type {
        "system" => {
            // System init event carries session_id
            let session_id = json.get("session_id").and_then(|v| v.as_str())?;
            Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                session_id: session_id.to_string(),
            }))
        }
        "tool_call" => {
            let subtype = json
                .get("subtype")
                .and_then(|v| v.as_str())
                .unwrap_or("started");
            let call_id = json.get("call_id").and_then(|v| v.as_str()).unwrap_or("");

            // Extract tool name from Cursor's *ToolCall nested structure
            let tool_name = json
                .get("tool_call")
                .and_then(|tc| tc.as_object())
                .and_then(|obj| obj.keys().next())
                .map(|k| {
                    // Convert "editToolCall" -> "Edit", "bashToolCall" -> "Bash", etc.
                    k.trim_end_matches("ToolCall")
                        .chars()
                        .next()
                        .map(|c| {
                            let mut s = c.to_uppercase().to_string();
                            s.push_str(&k.trim_end_matches("ToolCall")[c.len_utf8()..]);
                            s
                        })
                        .unwrap_or_else(|| k.to_string())
                })
                .unwrap_or_else(|| "tool".to_string());

            match subtype {
                "started" => {
                    // Extract args summary from the tool call
                    let input_summary = json
                        .get("tool_call")
                        .and_then(|tc| tc.as_object())
                        .and_then(|obj| obj.values().next())
                        .and_then(|v| v.get("args"))
                        .map(summarize_json)
                        .unwrap_or_default();
                    Some(StreamEvent::tool_start(&tool_name, call_id, &input_summary))
                }
                "completed" => {
                    let success = json
                        .get("tool_call")
                        .and_then(|tc| tc.as_object())
                        .and_then(|obj| obj.values().next())
                        .and_then(|v| v.get("result"))
                        .map(|r| r.get("success").is_some())
                        .unwrap_or(true);
                    Some(StreamEvent::new(StreamEventKind::ToolResult {
                        tool_name,
                        tool_id: call_id.to_string(),
                        success,
                    }))
                }
                _ => None,
            }
        }
        "assistant" => {
            let text = json
                .pointer("/message/content/0/text")
                .and_then(|v| v.as_str())?;
            Some(StreamEvent::text_delta(text))
        }
        "result" => {
            let is_error = json
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(StreamEvent::complete(!is_error))
        }
        // Ignore user message echo and unknown types
        _ => None,
    }
}

/// OpenCode headless runner
///
/// Runs OpenCode CLI in headless mode using `opencode run --format json`
/// and parses the streaming JSON output into unified StreamEvent types.
pub struct OpenCodeHeadless {
    binary_path: String,
}

impl OpenCodeHeadless {
    /// Create a new OpenCode headless runner
    ///
    /// Locates the OpenCode binary using the standard harness discovery mechanism.
    pub fn new() -> Result<Self> {
        let binary_path = find_harness_binary(Harness::OpenCode)?.to_string();
        Ok(Self { binary_path })
    }

    /// Create with an explicit binary path (useful for testing)
    #[cfg(test)]
    pub fn with_binary_path(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl HeadlessRunner for OpenCodeHeadless {
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>> {
        Box::pin(async move {
            // OpenCode uses `run` command with JSON format for headless streaming
            let mut cmd = Command::new(&self.binary_path);

            cmd.arg("run");
            cmd.arg("--format").arg("json");
            cmd.arg("--variant").arg("minimal");

            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }

            cmd.arg(prompt);
            cmd.current_dir(working_dir);
            cmd.env("SCUD_TASK_ID", task_id);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn()?;
            let (tx, rx) = mpsc::channel(1000);

            let stdout = child.stdout.take().expect("stdout was piped");
            let task_id_for_events = task_id.to_string();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(event) = parse_opencode_event(&line) {
                        trace!(task_id = %task_id_for_events, "opencode event: {:?}", event.kind);
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    } else if !line.trim().is_empty() {
                        debug!(task_id = %task_id_for_events, "opencode: unparsed line: {}", if line.len() > 200 { &line[..200] } else { &line });
                    }
                }

                let _ = tx.send(StreamEvent::complete(true)).await;
            });

            Ok(SessionHandle::from_child(task_id.to_string(), child, rx))
        })
    }

    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        // OpenCode uses attach command for session continuation
        vec![
            self.binary_path.clone(),
            "attach".to_string(),
            "http://localhost:4096".to_string(),
            "--session".to_string(),
            session_id.to_string(),
        ]
    }

    fn harness(&self) -> Harness {
        Harness::OpenCode
    }
}

/// Cursor Agent headless runner
///
/// Runs Cursor Agent CLI in headless mode with `-p` flag,
/// parsing the streaming JSON output into unified StreamEvent types.
pub struct CursorHeadless {
    binary_path: String,
}

impl CursorHeadless {
    /// Create a new Cursor headless runner
    pub fn new() -> Result<Self> {
        let binary_path = find_harness_binary(Harness::Cursor)?.to_string();
        Ok(Self { binary_path })
    }
}

impl HeadlessRunner for CursorHeadless {
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>> {
        Box::pin(async move {
            let mut cmd = Command::new(&self.binary_path);

            cmd.arg("-p");

            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }

            // Request streaming JSON output
            cmd.arg("--output-format").arg("stream-json");
            cmd.arg(prompt);
            cmd.current_dir(working_dir);
            cmd.env("SCUD_TASK_ID", task_id);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn()?;
            let (tx, rx) = mpsc::channel(1000);

            let stdout = child.stdout.take().expect("stdout was piped");
            let task_id_for_events = task_id.to_string();

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    // Try Cursor-specific parsing first
                    if let Some(event) = parse_cursor_event(&line) {
                        trace!(task_id = %task_id_for_events, "cursor event: {:?}", event.kind);
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    } else if !line.trim().is_empty() {
                        if serde_json::from_str::<serde_json::Value>(&line).is_err() {
                            // Non-JSON output treated as text
                            let _ = tx
                                .send(StreamEvent::text_delta(format!("{}\n", line)))
                                .await;
                        } else {
                            debug!(task_id = %task_id_for_events, "cursor: unparsed json: {}", if line.len() > 200 { &line[..200] } else { &line });
                        }
                    }
                }

                let _ = tx.send(StreamEvent::complete(true)).await;
            });

            Ok(SessionHandle::from_child(task_id.to_string(), child, rx))
        })
    }

    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        vec![
            self.binary_path.clone(),
            "--resume".to_string(),
            session_id.to_string(),
        ]
    }

    fn harness(&self) -> Harness {
        Harness::Cursor
    }
}

/// Rho CLI headless runner
///
/// Runs rho-cli with `--output-format stream-json` and parses the
/// newline-delimited JSON events into unified `StreamEvent` types.
pub struct RhoHeadless {
    binary_path: String,
    model: Option<String>,
}

impl RhoHeadless {
    /// Create a new Rho headless runner
    pub fn new(model: Option<String>) -> Result<Self> {
        let binary_path = find_harness_binary(Harness::Rho)?.to_string();
        Ok(Self {
            binary_path,
            model,
        })
    }

    #[cfg(test)]
    pub fn with_binary_path(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
            model: None,
        }
    }

    #[cfg(test)]
    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }
}

impl HeadlessRunner for RhoHeadless {
    fn start<'a>(
        &'a self,
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        model: Option<&'a str>,
    ) -> BoxFuture<'a, Result<SessionHandle>> {
        Box::pin(async move {
            let mut cmd = Command::new(&self.binary_path);

            // Use stream-json output for structured event parsing
            cmd.arg("--output-format").arg("stream-json");
            cmd.arg("-p").arg(prompt);
            cmd.arg("-C").arg(working_dir);

            // Model: prefer the per-call override, fall back to constructor model
            let effective_model = model.or(self.model.as_deref());
            if let Some(m) = effective_model {
                cmd.arg("--model").arg(m);
            }

            cmd.current_dir(working_dir);
            cmd.env("SCUD_TASK_ID", task_id);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn()?;
            let (tx, rx) = mpsc::channel(1000);

            let stdout = child.stdout.take().expect("stdout was piped");
            let task_id_for_events = task_id.to_string();

            // Parse stdout as newline-delimited JSON (stream-json format)
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(event) = parse_rho_event(&line) {
                        trace!(task_id = %task_id_for_events, "rho event: {:?}", event.kind);
                        if tx.send(event).await.is_err() {
                            break;
                        }
                    } else if !line.trim().is_empty() {
                        debug!(task_id = %task_id_for_events, "rho: unparsed line: {}", if line.len() > 200 { &line[..200] } else { &line });
                    }
                }

                // Send completion event (in case rho exited without a "complete" event)
                let _ = tx.send(StreamEvent::complete(true)).await;
            });

            Ok(SessionHandle::from_child(task_id.to_string(), child, rx))
        })
    }

    fn interactive_command(&self, session_id: &str) -> Vec<String> {
        vec![
            self.binary_path.clone(),
            "--resume".to_string(),
            session_id.to_string(),
        ]
    }

    fn harness(&self) -> Harness {
        Harness::Rho
    }
}

/// Parse a line of rho stream-json output into a StreamEvent
///
/// Rho CLI (`rho-cli --output-format stream-json`) outputs newline-delimited
/// JSON events with the following structure:
///
/// - `{"type":"session","session_id":"<uuid>"}` - Session assignment
/// - `{"type":"text_delta","text":"..."}` - Incremental text output
/// - `{"type":"tool_start","tool_name":"read","tool_id":"tc_1","input_summary":"..."}` - Tool start
/// - `{"type":"tool_result","tool_name":"read","tool_id":"tc_1","success":true}` - Tool result
/// - `{"type":"complete","success":true,"session_id":"<uuid>"}` - Completion
/// - `{"type":"error","message":"..."}` - Error
fn parse_rho_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let event_type = json.get("type")?.as_str()?;

    match event_type {
        "session" => {
            let session_id = json.get("session_id").and_then(|v| v.as_str())?;
            Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                session_id: session_id.to_string(),
            }))
        }
        "text_delta" => {
            let text = json.get("text").and_then(|v| v.as_str())?;
            Some(StreamEvent::text_delta(text))
        }
        "tool_start" => {
            let tool_name = json
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tool_id = json
                .get("tool_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let input_summary = json
                .get("input_summary")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(StreamEvent::tool_start(tool_name, tool_id, input_summary))
        }
        "tool_result" => {
            let tool_name = json
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_id = json
                .get("tool_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let success = json
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Some(StreamEvent::new(StreamEventKind::ToolResult {
                tool_name,
                tool_id,
                success,
            }))
        }
        "complete" => {
            let success = json
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Some(StreamEvent::complete(success))
        }
        "error" => {
            let message = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Some(StreamEvent::error(message))
        }
        _ => None,
    }
}

/// Enum-based runner that wraps concrete implementations
///
/// This provides polymorphism without requiring the trait to be object-safe.
/// Use this instead of `Box<dyn HeadlessRunner>` when you need to store
/// or pass around a runner of unknown concrete type.
pub enum AnyRunner {
    Claude(ClaudeHeadless),
    OpenCode(OpenCodeHeadless),
    Cursor(CursorHeadless),
    Rho(RhoHeadless),
    #[cfg(feature = "direct-api")]
    DirectApi(super::direct_api::DirectApiRunner),
}

impl AnyRunner {
    /// Create a DirectApi runner with an explicit provider
    #[cfg(feature = "direct-api")]
    pub fn new_direct_api(provider: crate::llm::provider::AgentProvider) -> Self {
        AnyRunner::DirectApi(super::direct_api::DirectApiRunner::new().with_provider(provider))
    }

    /// Create a runner for the specified harness
    pub fn new(harness: Harness) -> Result<Self> {
        match harness {
            Harness::Claude => Ok(AnyRunner::Claude(ClaudeHeadless::new()?)),
            Harness::OpenCode => Ok(AnyRunner::OpenCode(OpenCodeHeadless::new()?)),
            Harness::Cursor => Ok(AnyRunner::Cursor(CursorHeadless::new()?)),
            Harness::Rho => Ok(AnyRunner::Rho(RhoHeadless::new(None)?)),
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => Ok(AnyRunner::DirectApi(
                super::direct_api::DirectApiRunner::new(),
            )),
        }
    }

    /// Start an agent with a prompt
    pub async fn start(
        &self,
        task_id: &str,
        prompt: &str,
        working_dir: &Path,
        model: Option<&str>,
    ) -> Result<SessionHandle> {
        match self {
            AnyRunner::Claude(runner) => runner.start(task_id, prompt, working_dir, model).await,
            AnyRunner::OpenCode(runner) => runner.start(task_id, prompt, working_dir, model).await,
            AnyRunner::Cursor(runner) => runner.start(task_id, prompt, working_dir, model).await,
            AnyRunner::Rho(runner) => runner.start(task_id, prompt, working_dir, model).await,
            #[cfg(feature = "direct-api")]
            AnyRunner::DirectApi(runner) => runner.start(task_id, prompt, working_dir, model).await,
        }
    }

    /// Get the command to launch interactive mode for session continuation
    pub fn interactive_command(&self, session_id: &str) -> Vec<String> {
        match self {
            AnyRunner::Claude(runner) => runner.interactive_command(session_id),
            AnyRunner::OpenCode(runner) => runner.interactive_command(session_id),
            AnyRunner::Cursor(runner) => runner.interactive_command(session_id),
            AnyRunner::Rho(runner) => runner.interactive_command(session_id),
            #[cfg(feature = "direct-api")]
            AnyRunner::DirectApi(runner) => runner.interactive_command(session_id),
        }
    }

    /// Get the harness type this runner supports
    pub fn harness(&self) -> Harness {
        match self {
            AnyRunner::Claude(runner) => runner.harness(),
            AnyRunner::OpenCode(runner) => runner.harness(),
            AnyRunner::Cursor(runner) => runner.harness(),
            AnyRunner::Rho(runner) => runner.harness(),
            #[cfg(feature = "direct-api")]
            AnyRunner::DirectApi(runner) => runner.harness(),
        }
    }
}

/// Create a headless runner for the specified harness
///
/// This is a convenience function that returns an `AnyRunner` enum
/// which provides a unified interface for all runner implementations.
pub fn create_runner(harness: Harness) -> Result<AnyRunner> {
    AnyRunner::new(harness)
}

/// Parse a line of OpenCode JSON output into a StreamEvent
///
/// OpenCode CLI (`opencode run --format json`) outputs newline-delimited JSON events
/// with the following structure:
///
/// - `{"type": "assistant", "message": {"content": [{"text": "..."}]}}` - Text output
/// - `{"type": "tool_call", "subtype": "started", "tool_call": {"name": "...", "input": {...}}}` - Tool start
/// - `{"type": "tool_call", "subtype": "completed", "tool_call": {...}, "result": {...}}` - Tool result
/// - `{"type": "result", "success": true}` - Completion
/// - `{"type": "error", "message": "..."}` - Error
/// - `{"type": "session", "session_id": "..."}` - Session assignment
///
/// Returns `None` for unparseable or unknown event types (graceful degradation).
pub fn parse_opencode_event(line: &str) -> Option<StreamEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let event_type = json.get("type")?.as_str()?;

    match event_type {
        // Assistant text output - may have various content structures
        "assistant" | "message" | "content" => {
            // Try multiple paths for text content
            let text = json
                .pointer("/message/content/0/text")
                .or_else(|| json.pointer("/content/0/text"))
                .or_else(|| json.pointer("/message/text"))
                .or_else(|| json.get("text"))
                .or_else(|| json.get("delta"))
                .and_then(|v| v.as_str())?;
            Some(StreamEvent::text_delta(text))
        }

        // Tool call events with subtype
        "tool_call" | "tool_use" => {
            let subtype = json
                .get("subtype")
                .or_else(|| json.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("started");

            match subtype {
                "started" | "start" | "pending" => {
                    // Extract tool name from various possible locations
                    let tool_name = json
                        .pointer("/tool_call/name")
                        .or_else(|| json.pointer("/tool_call/tool"))
                        .or_else(|| json.get("name"))
                        .or_else(|| json.get("tool"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    // Extract tool ID
                    let tool_id = json
                        .pointer("/tool_call/id")
                        .or_else(|| json.get("id"))
                        .or_else(|| json.get("tool_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Extract and summarize input
                    let input = json
                        .pointer("/tool_call/input")
                        .or_else(|| json.get("input"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let input_summary = summarize_json(&input);

                    Some(StreamEvent::tool_start(tool_name, tool_id, &input_summary))
                }
                "completed" | "complete" | "done" | "success" => {
                    let tool_name = json
                        .pointer("/tool_call/name")
                        .or_else(|| json.get("name"))
                        .or_else(|| json.get("tool"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let tool_id = json
                        .pointer("/tool_call/id")
                        .or_else(|| json.get("id"))
                        .or_else(|| json.get("tool_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Check for error in result
                    let success = !json
                        .pointer("/result/is_error")
                        .or_else(|| json.get("is_error"))
                        .or_else(|| json.get("error"))
                        .map(|v| v.as_bool().unwrap_or(false) || v.is_string())
                        .unwrap_or(false);

                    Some(StreamEvent::new(StreamEventKind::ToolResult {
                        tool_name: tool_name.to_string(),
                        tool_id: tool_id.to_string(),
                        success,
                    }))
                }
                "failed" | "error" => {
                    let tool_name = json
                        .pointer("/tool_call/name")
                        .or_else(|| json.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let tool_id = json
                        .pointer("/tool_call/id")
                        .or_else(|| json.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    Some(StreamEvent::new(StreamEventKind::ToolResult {
                        tool_name: tool_name.to_string(),
                        tool_id: tool_id.to_string(),
                        success: false,
                    }))
                }
                _ => None,
            }
        }

        // Completion event
        "result" | "done" | "complete" => {
            let success = json
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Some(StreamEvent::complete(success))
        }

        // Error event
        "error" => {
            let message = json
                .get("message")
                .or_else(|| json.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Some(StreamEvent::error(message))
        }

        // Session assignment
        "session" | "session_start" | "init" => {
            let session_id = json
                .get("session_id")
                .or_else(|| json.get("id"))
                .and_then(|v| v.as_str())?;
            Some(StreamEvent::new(StreamEventKind::SessionAssigned {
                session_id: session_id.to_string(),
            }))
        }

        // Unknown event type - return None for graceful handling
        _ => None,
    }
}

/// Summarize JSON input for compact display
///
/// Produces a short, human-readable summary of JSON values:
/// - Objects: `{key1, key2, ...}` (max 3 keys)
/// - Strings: First 50 chars with ellipsis
/// - Other: JSON stringified, truncated
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
        serde_json::Value::Null => String::new(),
        serde_json::Value::Array(arr) => {
            format!("[{} items]", arr.len())
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // =======================
    // Claude event parsing
    // =======================

    #[test]
    fn test_parse_claude_text_delta() {
        let line =
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hello"}}}"#;
        let event = parse_claude_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Hello"
        ));
    }

    #[test]
    fn test_parse_claude_tool_use() {
        let line =
            r#"{"type":"tool_use","name":"Read","id":"tool_1","input":{"path":"src/main.rs"}}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ref input_summary,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_id, "tool_1");
                assert!(input_summary.contains("path"));
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_parse_claude_error() {
        let line = r#"{"type":"error","error":"Rate limit exceeded"}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::Error { ref message },
                ..
            }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_parse_claude_system_init_session() {
        let line = r#"{"type":"system","subtype":"init","session_id":"sess-init-123"}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "sess-init-123");
            }
            _ => panic!("Expected SessionAssigned from system init event"),
        }
    }

    #[test]
    fn test_parse_claude_result_with_session() {
        let line = r#"{"type":"result","session_id":"sess-abc123"}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "sess-abc123");
            }
            _ => panic!("Expected SessionAssigned"),
        }
    }

    #[test]
    fn test_parse_claude_result_completion() {
        let line = r#"{"type":"result"}"#;
        let event = parse_claude_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_claude_tool_result() {
        let line = r#"{"type":"tool_result","tool_use_id":"tool_1","content":"success"}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_id,
                        success,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_id, "tool_1");
                assert!(success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_claude_tool_result_error() {
        let line = r#"{"type":"tool_result","tool_use_id":"tool_2","is_error":true}"#;
        let event = parse_claude_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                assert!(!success);
            }
            _ => panic!("Expected ToolResult with failure"),
        }
    }

    #[test]
    fn test_parse_claude_unknown_type_returns_none() {
        let line = r#"{"type":"unknown_event","data":"test"}"#;
        let event = parse_claude_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_claude_interactive_command() {
        let runner = ClaudeHeadless::with_binary_path("/usr/local/bin/claude");
        let cmd = runner.interactive_command("sess_123");
        assert_eq!(cmd[0], "/usr/local/bin/claude");
        assert_eq!(cmd[1], "--resume");
        assert_eq!(cmd[2], "sess_123");
    }

    // =======================
    // OpenCode event parsing
    // =======================

    #[test]
    fn test_parse_assistant_text_with_message_content() {
        let line = r#"{"type": "assistant", "message": {"content": [{"text": "Hello world"}]}}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Hello world"
        ));
    }

    #[test]
    fn test_parse_content_type_with_text() {
        let line = r#"{"type": "content", "content": [{"text": "Response text"}]}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Response text"
        ));
    }

    #[test]
    fn test_parse_message_type_with_direct_text() {
        let line = r#"{"type": "message", "text": "Direct text"}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Direct text"
        ));
    }

    #[test]
    fn test_parse_assistant_with_delta_field() {
        let line = r#"{"type": "assistant", "delta": "Streaming chunk"}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Streaming chunk"
        ));
    }

    // ===================
    // Tool call parsing
    // ===================

    #[test]
    fn test_parse_tool_call_started() {
        let line = r#"{"type": "tool_call", "subtype": "started", "tool_call": {"name": "read_file", "id": "tool_1", "input": {"path": "src/main.rs"}}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ref input_summary,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "read_file");
                assert_eq!(tool_id, "tool_1");
                assert!(input_summary.contains("path"));
            }
            _ => panic!("Expected ToolStart, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_use_start() {
        let line = r#"{"type": "tool_use", "status": "start", "name": "bash", "id": "t123"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_name, "bash");
                assert_eq!(tool_id, "t123");
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_parse_tool_call_completed() {
        let line = r#"{"type": "tool_call", "subtype": "completed", "tool_call": {"name": "write_file", "id": "t2"}, "result": {}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_name,
                        ref tool_id,
                        success,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "write_file");
                assert_eq!(tool_id, "t2");
                assert!(success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_tool_call_with_error() {
        let line = r#"{"type": "tool_call", "subtype": "completed", "name": "bash", "result": {"is_error": true}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                assert!(!success);
            }
            _ => panic!("Expected ToolResult with failure"),
        }
    }

    #[test]
    fn test_parse_tool_call_failed_subtype() {
        let line = r#"{"type": "tool_call", "subtype": "failed", "name": "git", "id": "t3"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                assert!(!success);
            }
            _ => panic!("Expected failed ToolResult"),
        }
    }

    // ===================
    // Completion parsing
    // ===================

    #[test]
    fn test_parse_result_success() {
        let line = r#"{"type": "result", "success": true}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_result_failure() {
        let line = r#"{"type": "result", "success": false}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: false },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_done_type() {
        let line = r#"{"type": "done"}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    // ===================
    // Error parsing
    // ===================

    #[test]
    fn test_parse_error_with_message() {
        let line = r#"{"type": "error", "message": "Connection failed"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::Error { ref message },
                ..
            }) => {
                assert_eq!(message, "Connection failed");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_parse_error_with_error_field() {
        let line = r#"{"type": "error", "error": "Rate limited"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::Error { ref message },
                ..
            }) => {
                assert_eq!(message, "Rate limited");
            }
            _ => panic!("Expected Error event"),
        }
    }

    // ===================
    // Session parsing
    // ===================

    #[test]
    fn test_parse_session_assignment() {
        let line = r#"{"type": "session", "session_id": "sess_abc123"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "sess_abc123");
            }
            _ => panic!("Expected SessionAssigned"),
        }
    }

    #[test]
    fn test_parse_session_with_id_field() {
        let line = r#"{"type": "init", "id": "session_xyz"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "session_xyz");
            }
            _ => panic!("Expected SessionAssigned"),
        }
    }

    // ===================
    // Edge cases
    // ===================

    #[test]
    fn test_parse_unknown_event_returns_none() {
        let line = r#"{"type": "custom_event", "data": "something"}"#;
        let event = parse_opencode_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_invalid_json_returns_none() {
        let line = "not json at all";
        let event = parse_opencode_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_missing_type_returns_none() {
        let line = r#"{"message": "no type field"}"#;
        let event = parse_opencode_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_empty_json_returns_none() {
        let line = "{}";
        let event = parse_opencode_event(line);
        assert!(event.is_none());
    }

    // ===================
    // JSON summarization
    // ===================

    #[test]
    fn test_summarize_json_object() {
        let value = serde_json::json!({"path": "/foo", "content": "bar"});
        let summary = summarize_json(&value);
        assert!(summary.contains("path"));
        assert!(summary.contains("content"));
    }

    #[test]
    fn test_summarize_json_object_truncated() {
        let value = serde_json::json!({
            "key1": "v1",
            "key2": "v2",
            "key3": "v3",
            "key4": "v4"
        });
        let summary = summarize_json(&value);
        assert!(summary.contains("..."));
    }

    #[test]
    fn test_summarize_json_empty_object() {
        let value = serde_json::json!({});
        let summary = summarize_json(&value);
        assert_eq!(summary, "{}");
    }

    #[test]
    fn test_summarize_json_string() {
        let value = serde_json::json!("short string");
        let summary = summarize_json(&value);
        assert_eq!(summary, "\"short string\"");
    }

    #[test]
    fn test_summarize_json_long_string() {
        let long = "a".repeat(100);
        let value = serde_json::json!(long);
        let summary = summarize_json(&value);
        assert!(summary.len() < 60);
        assert!(summary.ends_with("...\""));
    }

    #[test]
    fn test_summarize_json_null() {
        let value = serde_json::Value::Null;
        let summary = summarize_json(&value);
        assert_eq!(summary, "");
    }

    #[test]
    fn test_summarize_json_array() {
        let value = serde_json::json!([1, 2, 3, 4, 5]);
        let summary = summarize_json(&value);
        assert_eq!(summary, "[5 items]");
    }

    #[test]
    fn test_summarize_json_number() {
        let value = serde_json::json!(42);
        let summary = summarize_json(&value);
        assert_eq!(summary, "42");
    }

    // ===================
    // Interactive command
    // ===================

    #[test]
    fn test_interactive_command_format() {
        let runner = OpenCodeHeadless::with_binary_path("/usr/local/bin/opencode");
        let cmd = runner.interactive_command("session_123");
        assert_eq!(cmd[0], "/usr/local/bin/opencode");
        assert_eq!(cmd[1], "attach");
        assert!(cmd.contains(&"--session".to_string()));
        assert!(cmd.contains(&"session_123".to_string()));
    }

    // ===================
    // OpenCodeHeadless struct tests
    // ===================

    #[test]
    fn test_opencode_headless_with_binary_path() {
        let runner = OpenCodeHeadless::with_binary_path("/custom/path/opencode");
        // Verify harness returns OpenCode
        assert!(matches!(runner.harness(), Harness::OpenCode));
    }

    #[test]
    fn test_opencode_interactive_command_structure() {
        let runner = OpenCodeHeadless::with_binary_path("/bin/opencode");
        let cmd = runner.interactive_command("sess-xyz-789");

        // Should produce: opencode attach http://localhost:4096 --session sess-xyz-789
        assert_eq!(cmd.len(), 5);
        assert_eq!(cmd[0], "/bin/opencode");
        assert_eq!(cmd[1], "attach");
        assert_eq!(cmd[2], "http://localhost:4096");
        assert_eq!(cmd[3], "--session");
        assert_eq!(cmd[4], "sess-xyz-789");
    }

    #[test]
    fn test_opencode_harness_type() {
        let runner = OpenCodeHeadless::with_binary_path("opencode");
        assert_eq!(runner.harness(), Harness::OpenCode);
    }

    // ===================
    // ClaudeHeadless struct tests
    // ===================

    #[test]
    fn test_claude_headless_with_binary_path() {
        let runner = ClaudeHeadless::with_binary_path("/custom/claude");
        assert_eq!(runner.binary_path(), "/custom/claude");
        assert!(matches!(runner.harness(), Harness::Claude));
    }

    #[test]
    fn test_claude_headless_with_allowed_tools() {
        let runner = ClaudeHeadless::with_binary_path("/bin/claude")
            .with_allowed_tools(vec!["Read".to_string(), "Write".to_string()]);
        // The runner should accept the tools (no getter, but constructor works)
        assert_eq!(runner.binary_path(), "/bin/claude");
    }

    #[test]
    fn test_claude_interactive_command_structure() {
        let runner = ClaudeHeadless::with_binary_path("/usr/bin/claude");
        let cmd = runner.interactive_command("sess-abc-123");

        // Should produce: claude --resume sess-abc-123
        assert_eq!(cmd.len(), 3);
        assert_eq!(cmd[0], "/usr/bin/claude");
        assert_eq!(cmd[1], "--resume");
        assert_eq!(cmd[2], "sess-abc-123");
    }

    #[test]
    fn test_claude_harness_type() {
        let runner = ClaudeHeadless::with_binary_path("claude");
        assert_eq!(runner.harness(), Harness::Claude);
    }

    // ===================
    // AnyRunner enum tests
    // ===================

    #[test]
    fn test_any_runner_claude_variant() {
        let runner = AnyRunner::Claude(ClaudeHeadless::with_binary_path("/bin/claude"));
        assert_eq!(runner.harness(), Harness::Claude);

        let cmd = runner.interactive_command("session-1");
        assert_eq!(cmd[0], "/bin/claude");
        assert_eq!(cmd[1], "--resume");
    }

    #[test]
    fn test_any_runner_opencode_variant() {
        let runner = AnyRunner::OpenCode(OpenCodeHeadless::with_binary_path("/bin/opencode"));
        assert_eq!(runner.harness(), Harness::OpenCode);

        let cmd = runner.interactive_command("session-2");
        assert_eq!(cmd[0], "/bin/opencode");
        assert_eq!(cmd[1], "attach");
    }

    #[test]
    fn test_any_runner_rho_variant_resume_command() {
        let runner = AnyRunner::Rho(RhoHeadless::with_binary_path("/bin/rho-cli"));
        assert_eq!(runner.harness(), Harness::Rho);

        let cmd = runner.interactive_command("session-rho-1");
        assert_eq!(cmd.len(), 3);
        assert_eq!(cmd[0], "/bin/rho-cli");
        assert_eq!(cmd[1], "--resume");
        assert_eq!(cmd[2], "session-rho-1");
    }

    #[test]
    fn test_any_runner_harness_matches() {
        let claude = AnyRunner::Claude(ClaudeHeadless::with_binary_path("claude"));
        let opencode = AnyRunner::OpenCode(OpenCodeHeadless::with_binary_path("opencode"));

        // Verify harness() returns correct type for each variant
        assert!(matches!(claude.harness(), Harness::Claude));
        assert!(matches!(opencode.harness(), Harness::OpenCode));
    }

    // ===================
    // Additional OpenCode parsing edge cases
    // ===================

    #[test]
    fn test_parse_opencode_tool_with_pending_status() {
        let line =
            r#"{"type": "tool_call", "status": "pending", "tool": "write_file", "id": "t99"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_name, "write_file");
                assert_eq!(tool_id, "t99");
            }
            _ => panic!("Expected ToolStart for pending status"),
        }
    }

    #[test]
    fn test_parse_opencode_tool_done_status() {
        let line = r#"{"type": "tool_call", "subtype": "done", "name": "exec", "id": "t50"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_name,
                        success,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_name, "exec");
                assert!(success);
            }
            _ => panic!("Expected ToolResult for done subtype"),
        }
    }

    #[test]
    fn test_parse_opencode_tool_success_status() {
        let line = r#"{"type": "tool_use", "subtype": "success", "tool_call": {"name": "bash", "id": "t77"}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                assert!(success);
            }
            _ => panic!("Expected ToolResult for success subtype"),
        }
    }

    #[test]
    fn test_parse_opencode_complete_type() {
        let line = r#"{"type": "complete", "success": true}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_opencode_session_start_type() {
        let line = r#"{"type": "session_start", "session_id": "sess-start-001"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "sess-start-001");
            }
            _ => panic!("Expected SessionAssigned for session_start type"),
        }
    }

    #[test]
    fn test_parse_opencode_assistant_with_message_text() {
        let line = r#"{"type": "assistant", "message": {"text": "Thinking about this..."}}"#;
        let event = parse_opencode_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Thinking about this..."
        ));
    }

    #[test]
    fn test_parse_opencode_tool_call_error_subtype() {
        let line = r#"{"type": "tool_call", "subtype": "error", "tool_call": {"name": "git", "id": "t88"}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_name,
                        success,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_name, "git");
                assert!(!success);
            }
            _ => panic!("Expected failed ToolResult for error subtype"),
        }
    }

    #[test]
    fn test_parse_opencode_tool_with_nested_input() {
        let line = r#"{"type": "tool_call", "subtype": "started", "tool_call": {"name": "write_file", "id": "t100", "input": {"path": "src/lib.rs", "content": "// Code here", "mode": "overwrite"}}}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref input_summary,
                        ..
                    },
                ..
            }) => {
                assert_eq!(tool_name, "write_file");
                // Input should be summarized with keys
                assert!(input_summary.contains("path"));
            }
            _ => panic!("Expected ToolStart with input summary"),
        }
    }

    #[test]
    fn test_parse_opencode_tool_result_with_error_string() {
        let line = r#"{"type": "tool_call", "subtype": "completed", "name": "bash", "error": "Command not found"}"#;
        let event = parse_opencode_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                // error field as string should indicate failure
                assert!(!success);
            }
            _ => panic!("Expected failed ToolResult"),
        }
    }

    #[test]
    fn test_parse_opencode_unknown_subtype_returns_none() {
        let line = r#"{"type": "tool_call", "subtype": "unknown_status", "name": "bash"}"#;
        let event = parse_opencode_event(line);
        assert!(event.is_none());
    }

    // =======================
    // Cursor event parsing
    // =======================

    #[test]
    fn test_parse_cursor_system_init() {
        let line = r#"{"type":"system","subtype":"init","session_id":"013608ef-dda7-4b38-9741-54fb0323ce1c","model":"Claude 4.5 Opus"}"#;
        let event = parse_cursor_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "013608ef-dda7-4b38-9741-54fb0323ce1c");
            }
            _ => panic!("Expected SessionAssigned from system init"),
        }
    }

    #[test]
    fn test_parse_cursor_tool_call_started() {
        let line = r#"{"type":"tool_call","subtype":"started","call_id":"toolu_123","tool_call":{"editToolCall":{"args":{"path":"/tmp/hello.py","streamContent":"print(\"Hello\")\n"}}}}"#;
        let event = parse_cursor_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ref input_summary,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "Edit");
                assert_eq!(tool_id, "toolu_123");
                assert!(input_summary.contains("path"));
            }
            _ => panic!("Expected ToolStart, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_cursor_tool_call_completed() {
        let line = r#"{"type":"tool_call","subtype":"completed","call_id":"toolu_123","tool_call":{"editToolCall":{"args":{"path":"/tmp/hello.py"},"result":{"success":{"path":"/tmp/hello.py","linesAdded":1}}}}}"#;
        let event = parse_cursor_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_name,
                        ref tool_id,
                        success,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "Edit");
                assert_eq!(tool_id, "toolu_123");
                assert!(success);
            }
            _ => panic!("Expected ToolResult, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_cursor_assistant_message() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Created hello.py"}]}}"#;
        let event = parse_cursor_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Created hello.py"
        ));
    }

    #[test]
    fn test_parse_cursor_result_success() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"Done","session_id":"sess-123"}"#;
        let event = parse_cursor_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_cursor_result_error() {
        let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"Failed"}"#;
        let event = parse_cursor_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: false },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_cursor_user_message_ignored() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Do something"}]}}"#;
        let event = parse_cursor_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_cursor_invalid_json() {
        let event = parse_cursor_event("not json");
        assert!(event.is_none());
    }

    // =======================
    // Rho event parsing
    // =======================

    #[test]
    fn test_parse_rho_session() {
        let line = r#"{"type":"session","session_id":"abc-123-def"}"#;
        let event = parse_rho_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::SessionAssigned { ref session_id },
                ..
            }) => {
                assert_eq!(session_id, "abc-123-def");
            }
            _ => panic!("Expected SessionAssigned"),
        }
    }

    #[test]
    fn test_parse_rho_text_delta() {
        let line = r#"{"type":"text_delta","text":"Hello world"}"#;
        let event = parse_rho_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::TextDelta { ref text },
                ..
            }) if text == "Hello world"
        ));
    }

    #[test]
    fn test_parse_rho_tool_start() {
        let line =
            r#"{"type":"tool_start","tool_name":"read","tool_id":"tc_1","input_summary":"src/main.rs"}"#;
        let event = parse_rho_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolStart {
                        ref tool_name,
                        ref tool_id,
                        ref input_summary,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "read");
                assert_eq!(tool_id, "tc_1");
                assert_eq!(input_summary, "src/main.rs");
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_parse_rho_tool_result() {
        let line = r#"{"type":"tool_result","tool_name":"read","tool_id":"tc_1","success":true}"#;
        let event = parse_rho_event(line);
        match event {
            Some(StreamEvent {
                kind:
                    StreamEventKind::ToolResult {
                        ref tool_name,
                        ref tool_id,
                        success,
                    },
                ..
            }) => {
                assert_eq!(tool_name, "read");
                assert_eq!(tool_id, "tc_1");
                assert!(success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_rho_tool_result_failure() {
        let line =
            r#"{"type":"tool_result","tool_name":"bash","tool_id":"tc_2","success":false}"#;
        let event = parse_rho_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::ToolResult { success, .. },
                ..
            }) => {
                assert!(!success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_rho_complete() {
        let line = r#"{"type":"complete","success":true,"session_id":"abc-123"}"#;
        let event = parse_rho_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: true },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_rho_complete_failure() {
        let line = r#"{"type":"complete","success":false}"#;
        let event = parse_rho_event(line);
        assert!(matches!(
            event,
            Some(StreamEvent {
                kind: StreamEventKind::Complete { success: false },
                ..
            })
        ));
    }

    #[test]
    fn test_parse_rho_error() {
        let line = r#"{"type":"error","message":"Rate limit exceeded"}"#;
        let event = parse_rho_event(line);
        match event {
            Some(StreamEvent {
                kind: StreamEventKind::Error { ref message },
                ..
            }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_parse_rho_unknown_type() {
        let line = r#"{"type":"unknown_event","data":"something"}"#;
        let event = parse_rho_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_rho_invalid_json() {
        let event = parse_rho_event("not json at all");
        assert!(event.is_none());
    }
}
