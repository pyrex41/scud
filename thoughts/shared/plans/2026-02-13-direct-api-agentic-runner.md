# Direct API Agentic Runner Implementation Plan

## Overview

Replace the `claude -p` harness subprocess approach with direct Anthropic API calls for code generation loops. Instead of spawning external CLI tools, scud will run its own agentic loop: send messages with tool definitions, execute tool_use blocks locally, return results, repeat until the model finishes.

This uses OAuth tokens from Claude Code's keychain for subscription billing (Pro/Max), with fallback to `ANTHROPIC_API_KEY` for standard API billing.

Protected behind a feature/config flag since the OAuth impersonation approach may break if Anthropic changes requirements.

## Current State Analysis

**Two paths exist today:**
1. **Direct API** (`llm/client.rs:202-244`): Simple text-in/text-out for PRD parsing. No tool use.
2. **Harness subprocess** (`spawn/headless/runner.rs`, `spawn/terminal.rs`): Spawns `claude -p` as external process. It handles tool execution internally.

**The gap:** Direct API path has no tool-use support. Code generation requires an agentic loop with local tool execution.

**HeadlessRunner trait** (`spawn/headless/runner.rs:84-105`):
```rust
pub trait HeadlessRunner: Send + Sync {
    fn start(&self, task_id, prompt, working_dir, model) -> BoxFuture<Result<SessionHandle>>;
    fn interactive_command(&self, session_id: &str) -> Vec<String>;
    fn harness(&self) -> Harness;
}
```
Returns `SessionHandle` with `events: mpsc::Receiver<StreamEvent>` channel.

**Config** (`.scud/config.toml`): Has `[llm]` and `[swarm]` sections. Providers include `anthropic`, `xai`, `openai`, `claude-cli`.

**Cargo features** (`scud-cli/Cargo.toml:53-57`): `real-llm`, `real-terminal`, `zmq` features exist.

### Key Discoveries:
- OAuth tokens in macOS Keychain under `"Claude Code-credentials"` with `sk-ant-oat01-` prefix
- Required headers: `Authorization: Bearer`, `user-agent: claude-cli/2.1.2 (external, cli)`, `x-app: cli`, `anthropic-beta: claude-code-20250219,oauth-2025-04-20`
- System prompt must prepend: "You are Claude Code, Anthropic's official CLI for Claude."
- Fast tools available on system: `rg` (ripgrep), `fd`, `ambs`, `ambr`
- `HeadlessRunner` + `AnyRunner` enum is the right plug-in point
- `StreamEvent` types (`TextDelta`, `ToolStart`, `ToolResult`, `Complete`, `Error`, `SessionAssigned`) are what TUI consumes

## Desired End State

A new `DirectApiRunner` implementing `HeadlessRunner` that:
- Calls Anthropic Messages API directly with OAuth Bearer auth
- Runs a full agentic loop: prompt -> tool_use -> execute -> tool_result -> repeat
- Implements Read, Write, Edit, Bash, Search (rg), Find (fd) tools locally
- Emits `StreamEvent`s on the channel for TUI/headless consumption
- Is selectable via config flag + Cargo feature

**Verification:**
- `scud spawn --harness direct-api --tag <tag>` spawns agents using direct API
- `scud ralph --harness direct-api --tag <tag>` runs ralph loop with direct API
- Agent can read files, write files, edit files, run bash commands, search codebases
- Falls back gracefully to `claude` harness when feature/config disabled

## What We're NOT Doing

- Implementing ALL Claude Code tools (WebFetch, WebSearch, NotebookEdit, Task, etc.)
- Building our own OAuth PKCE login flow (we read Claude Code's existing tokens)
- Replacing the tmux spawning infrastructure (agents still run in tmux windows via a wrapper)
- Supporting OpenAI/xAI tool use (Anthropic only for now)
- Implementing conversation resumption/session continuity (fresh context per task, like ralph already does)
- Token refresh back to keychain (read-only; if expired, fall back to API key)

## Implementation Approach

The runner implements `HeadlessRunner` and runs the agentic loop in a tokio task. The loop sends/receives messages via the Anthropic streaming API, executes tool_use blocks locally, and emits StreamEvents on the channel for TUI display.

For tmux-based modes (ralph, swarm), the runner is invoked via a new `scud agent-exec` subcommand that runs in the foreground, so it can be launched inside tmux windows just like `claude -p` is today.

## Phase 1: OAuth Token Reading & API Client

### Overview
Read Claude Code's OAuth tokens from macOS Keychain and create an authenticated Anthropic API client.

### Changes Required:

#### 1. New module: OAuth token reader
**File**: `scud-cli/src/llm/oauth.rs` (NEW)

Reads OAuth credentials from macOS Keychain, parses the JSON, checks expiry.

```rust
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeOAuthCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeychainData {
    claude_ai_oauth: Option<ClaudeOAuthCredentials>,
}

/// Read OAuth credentials from Claude Code's macOS Keychain entry
pub fn read_claude_oauth() -> Result<Option<ClaudeOAuthCredentials>> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .context("Failed to read from macOS Keychain")?;

    if !output.status.success() {
        return Ok(None); // No credentials stored
    }

    let json_str = String::from_utf8(output.stdout)
        .context("Keychain data is not valid UTF-8")?;
    let data: KeychainData = serde_json::from_str(json_str.trim())
        .context("Failed to parse keychain JSON")?;

    Ok(data.claude_ai_oauth)
}

/// Check if OAuth token is still valid (with 5-minute buffer)
pub fn is_token_valid(creds: &ClaudeOAuthCredentials) -> bool {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    creds.expires_at > now_ms + 300_000 // 5 minute buffer
}

/// Resolve the best available API credential for Anthropic.
/// Priority: OAuth token (subscription) > ANTHROPIC_API_KEY env var
pub fn resolve_anthropic_credential() -> Result<ApiCredential> {
    // Try OAuth first
    if let Ok(Some(creds)) = read_claude_oauth() {
        if is_token_valid(&creds) {
            return Ok(ApiCredential::OAuth(creds.access_token));
        }
        // Token expired - could try refresh, but for now fall through
        tracing::warn!("Claude Code OAuth token expired, falling back to API key");
    }

    // Fall back to API key
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        return Ok(ApiCredential::ApiKey(key));
    }

    anyhow::bail!(
        "No Anthropic credentials available. Either:\n\
         - Log in to Claude Code (`claude` CLI) for OAuth, or\n\
         - Set ANTHROPIC_API_KEY environment variable"
    )
}

pub enum ApiCredential {
    OAuth(String),  // Bearer token from subscription
    ApiKey(String), // Standard x-api-key
}
```

#### 2. Expose module
**File**: `scud-cli/src/llm/mod.rs`

Add `pub mod oauth;` to the module.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli --features direct-api` compiles
- [ ] Unit test: `read_claude_oauth()` returns credentials on macOS with Claude Code installed
- [ ] Unit test: `is_token_valid()` correctly checks expiry
- [ ] Unit test: `resolve_anthropic_credential()` returns OAuth when available, falls back to API key

---

## Phase 2: Tool Definitions & Execution

### Overview
Define the tools the agent can use and implement their local execution.

### Changes Required:

#### 1. New module: Tool implementations
**File**: `scud-cli/src/llm/tools.rs` (NEW)

Define tool schemas (JSON for API) and execution logic.

**Tools to implement:**

| Tool | API Name | Implementation |
|------|----------|----------------|
| Read | `Read` | `std::fs::read_to_string` with line range support |
| Write | `Write` | `std::fs::write` |
| Edit | `Edit` | In-memory string replacement (`old_string` -> `new_string`) |
| Bash | `Bash` | `tokio::process::Command` with timeout, working_dir |
| Search | `Search` | Shell out to `rg` (ripgrep) with pattern, path, flags |
| Find | `Find` | Shell out to `fd` with pattern, path |

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Tool definition for the Anthropic API
#[derive(Debug, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Return all tool definitions for the API request
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_tool_def(),
        write_tool_def(),
        edit_tool_def(),
        bash_tool_def(),
        search_tool_def(),
        find_tool_def(),
    ]
}

/// Execute a tool call and return the result as a string
pub async fn execute_tool(
    name: &str,
    input: &Value,
    working_dir: &Path,
) -> ToolResult {
    match name {
        "Read" => execute_read(input, working_dir).await,
        "Write" => execute_write(input, working_dir).await,
        "Edit" => execute_edit(input, working_dir).await,
        "Bash" => execute_bash(input, working_dir).await,
        "Search" => execute_search(input, working_dir).await,
        "Find" => execute_find(input, working_dir).await,
        _ => ToolResult {
            content: format!("Unknown tool: {}", name),
            is_error: true,
        },
    }
}

pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}
```

**Read tool** - reads a file, supports `offset` and `limit` for line ranges:
```rust
async fn execute_read(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
    let path = if file_path.starts_with('/') {
        PathBuf::from(file_path)
    } else {
        working_dir.join(file_path)
    };

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let limit = input.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

            let lines: Vec<&str> = content.lines().collect();
            let start = offset.min(lines.len());
            let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());

            // Format with line numbers like cat -n
            let numbered: Vec<String> = lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
                .collect();

            ToolResult { content: numbered.join("\n"), is_error: false }
        }
        Err(e) => ToolResult { content: format!("Error reading {}: {}", path.display(), e), is_error: true },
    }
}
```

**Write tool** - writes content to a file:
```rust
async fn execute_write(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
    let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let path = resolve_path(file_path, working_dir);

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(&path, content) {
        Ok(_) => ToolResult { content: format!("Wrote {} bytes to {}", content.len(), path.display()), is_error: false },
        Err(e) => ToolResult { content: format!("Error writing {}: {}", path.display(), e), is_error: true },
    }
}
```

**Edit tool** - exact string replacement in a file:
```rust
async fn execute_edit(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
    let old_string = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new_string = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
    let path = resolve_path(file_path, working_dir);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return ToolResult { content: format!("Error reading: {}", e), is_error: true },
    };

    let count = content.matches(old_string).count();
    if count == 0 {
        return ToolResult { content: "old_string not found in file".into(), is_error: true };
    }
    if count > 1 {
        return ToolResult {
            content: format!("old_string found {} times (must be unique). Provide more context.", count),
            is_error: true
        };
    }

    let new_content = content.replacen(old_string, new_string, 1);
    match std::fs::write(&path, new_content) {
        Ok(_) => ToolResult { content: format!("Edited {}", path.display()), is_error: false },
        Err(e) => ToolResult { content: format!("Error writing: {}", e), is_error: true },
    }
}
```

**Bash tool** - execute a command with timeout:
```rust
async fn execute_bash(input: &Value, working_dir: &Path) -> ToolResult {
    let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let timeout_ms = input.get("timeout").and_then(|v| v.as_u64()).unwrap_or(120_000);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .output()
    ).await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();
            if !stdout.is_empty() { result.push_str(&stdout); }
            if !stderr.is_empty() {
                if !result.is_empty() { result.push('\n'); }
                result.push_str("STDERR:\n");
                result.push_str(&stderr);
            }
            // Truncate if very large
            if result.len() > 30_000 {
                result.truncate(30_000);
                result.push_str("\n... (truncated)");
            }
            ToolResult { content: result, is_error: !output.status.success() }
        }
        Ok(Err(e)) => ToolResult { content: format!("Failed to execute: {}", e), is_error: true },
        Err(_) => ToolResult { content: format!("Command timed out after {}ms", timeout_ms), is_error: true },
    }
}
```

**Search tool** - shell out to `rg`:
```rust
async fn execute_search(input: &Value, working_dir: &Path) -> ToolResult {
    let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let path = input.get("path").and_then(|v| v.as_str());
    let glob_filter = input.get("glob").and_then(|v| v.as_str());
    let context_lines = input.get("context").and_then(|v| v.as_u64());

    let search_path = path.map(|p| resolve_path(p, working_dir))
        .unwrap_or_else(|| working_dir.to_path_buf());

    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--no-heading").arg("--line-number").arg("--color=never");

    if let Some(g) = glob_filter { cmd.arg("--glob").arg(g); }
    if let Some(c) = context_lines { cmd.arg("-C").arg(c.to_string()); }

    cmd.arg(pattern).arg(&search_path);

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut result = stdout.to_string();
            if result.len() > 30_000 {
                result.truncate(30_000);
                result.push_str("\n... (truncated)");
            }
            ToolResult { content: result, is_error: false }
        }
        Err(e) => ToolResult { content: format!("Search failed: {}", e), is_error: true },
    }
}
```

**Find tool** - shell out to `fd`:
```rust
async fn execute_find(input: &Value, working_dir: &Path) -> ToolResult {
    let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let path = input.get("path").and_then(|v| v.as_str());

    let search_path = path.map(|p| resolve_path(p, working_dir))
        .unwrap_or_else(|| working_dir.to_path_buf());

    let mut cmd = tokio::process::Command::new("fd");
    cmd.arg("--color=never").arg(pattern).arg(&search_path);

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            ToolResult { content: stdout.to_string(), is_error: false }
        }
        Err(e) => ToolResult { content: format!("Find failed: {}", e), is_error: true },
    }
}
```

Tool definitions will use Claude Code's exact parameter names and schemas so the model already knows how to use them from training. Example for Read:
```json
{
  "name": "Read",
  "description": "Read a file from the filesystem. Returns file contents with line numbers.",
  "input_schema": {
    "type": "object",
    "properties": {
      "file_path": { "type": "string", "description": "Absolute path to the file" },
      "offset": { "type": "integer", "description": "Line number to start from (0-indexed)" },
      "limit": { "type": "integer", "description": "Max lines to read" }
    },
    "required": ["file_path"]
  }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli --features direct-api`
- [ ] Unit tests for each tool: read, write, edit, bash, search, find
- [ ] Edit tool rejects non-unique matches
- [ ] Bash tool respects timeout
- [ ] Search/Find tools produce expected output

---

## Phase 3: Agentic Loop (Core Engine)

### Overview
The streaming agentic loop: send messages to API, receive tool_use blocks, execute tools, send results back, repeat.

### Changes Required:

#### 1. New module: Agentic loop engine
**File**: `scud-cli/src/llm/agent.rs` (NEW)

```rust
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

use super::oauth::{ApiCredential, resolve_anthropic_credential};
use super::tools::{self, ToolResult};
use crate::commands::spawn::headless::events::{StreamEvent, StreamEventKind};

const CLAUDE_CODE_VERSION: &str = "2.1.2";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const MAX_TURNS: usize = 200; // Safety limit

/// Anthropic Messages API request/response types
#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<Message>,
    tools: Vec<serde_json::Value>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}

/// Run the agentic loop.
/// Sends prompt to Anthropic API, executes tool calls, repeats until done.
/// Emits StreamEvents on the channel for TUI consumption.
pub async fn run_agent_loop(
    prompt: &str,
    system_prompt: Option<&str>,
    working_dir: &Path,
    model: Option<&str>,
    max_tokens: u32,
    event_tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
    let credential = resolve_anthropic_credential()?;
    let client = Client::new();
    let model = model.unwrap_or(DEFAULT_MODEL).to_string();

    // Build system prompt with Claude Code identity prefix for OAuth
    let system = build_system_prompt(&credential, system_prompt);

    // Build tool definitions
    let tool_defs = tools::tool_definitions_json();

    // Initial messages
    let mut messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text { text: prompt.to_string() }],
    }];

    // Agentic loop
    for turn in 0..MAX_TURNS {
        // Send request (non-streaming first for simplicity; streaming in Phase 4)
        let response = send_request(
            &client, &credential, &model, max_tokens, &system, &messages, &tool_defs,
        ).await?;

        // Process response content blocks
        let mut tool_calls = Vec::new();
        let mut has_text = false;

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    has_text = true;
                    let _ = event_tx.send(StreamEvent::text_delta(text)).await;
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // Emit tool start event
                    let summary = tools::summarize_input(input);
                    let _ = event_tx.send(StreamEvent::tool_start(name, id, &summary)).await;
                    tool_calls.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        // Add assistant response to conversation
        messages.push(Message {
            role: "assistant".to_string(),
            content: response.content.clone(),
        });

        // If no tool calls, we're done
        if tool_calls.is_empty() {
            break;
        }

        // Execute tool calls and collect results
        let mut tool_results = Vec::new();
        for (id, name, input) in &tool_calls {
            let result = tools::execute_tool(name, input, working_dir).await;

            // Emit tool result event
            let _ = event_tx.send(StreamEvent::new(StreamEventKind::ToolResult {
                tool_name: name.clone(),
                tool_id: id.clone(),
                success: !result.is_error,
            })).await;

            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: result.content,
                is_error: result.is_error,
            });
        }

        // Add tool results as user message
        messages.push(Message {
            role: "user".to_string(),
            content: tool_results,
        });
    }

    let _ = event_tx.send(StreamEvent::complete(true)).await;
    Ok(())
}

fn build_system_prompt(credential: &ApiCredential, custom: Option<&str>) -> Vec<SystemBlock> {
    let mut blocks = Vec::new();

    // Claude Code identity is required for OAuth
    if matches!(credential, ApiCredential::OAuth(_)) {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: "You are Claude Code, Anthropic's official CLI for Claude.".to_string(),
        });
    }

    if let Some(prompt) = custom {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: prompt.to_string(),
        });
    }

    blocks
}

/// Build and send the HTTP request to Anthropic API
async fn send_request(
    client: &Client,
    credential: &ApiCredential,
    model: &str,
    max_tokens: u32,
    system: &[SystemBlock],
    messages: &[Message],
    tools: &[serde_json::Value],
) -> Result<AssistantResponse> {
    let body = MessagesRequest {
        model: model.to_string(),
        max_tokens,
        system: system.to_vec(),
        messages: messages.to_vec(),
        tools: tools.to_vec(),
        stream: false, // Non-streaming initially; Phase 4 adds streaming
    };

    let mut req = client.post(ANTHROPIC_API_URL)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");

    // Auth and identity headers based on credential type
    match credential {
        ApiCredential::OAuth(token) => {
            req = req
                .bearer_auth(token)
                .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                .header("user-agent", format!("claude-cli/{} (external, cli)", CLAUDE_CODE_VERSION))
                .header("x-app", "cli");
        }
        ApiCredential::ApiKey(key) => {
            req = req.header("x-api-key", key);
        }
    }

    let response = req.json(&body).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error ({}): {}", status, text);
    }

    let resp: AssistantResponse = response.json().await?;
    Ok(resp)
}

#[derive(Debug, Deserialize)]
struct AssistantResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli --features direct-api`
- [ ] Integration test (feature-gated behind `real-llm`): send a simple prompt, get text response
- [ ] Integration test: send prompt requiring Read tool, verify file is read and content returned
- [ ] Agent loop terminates when model returns text-only (no tool_use)
- [ ] Agent loop respects MAX_TURNS safety limit

---

## Phase 4: HeadlessRunner Implementation

### Overview
Wire the agentic loop into the existing HeadlessRunner/AnyRunner infrastructure.

### Changes Required:

#### 1. New HeadlessRunner: DirectApiRunner
**File**: `scud-cli/src/commands/spawn/headless/direct_api.rs` (NEW)

```rust
use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;

use super::events::StreamEvent;
use super::runner::{BoxFuture, HeadlessRunner, SessionHandle};
use crate::commands::spawn::terminal::Harness;
use crate::llm::agent;

pub struct DirectApiRunner {
    model: Option<String>,
    max_tokens: u32,
}

impl DirectApiRunner {
    pub fn new() -> Self {
        Self {
            model: None,
            max_tokens: 16_000,
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

            // Spawn the agentic loop as a background task
            let tx_clone = tx.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = agent::run_agent_loop(
                    &prompt,
                    None, // system prompt handled inside agent
                    &working_dir,
                    model.as_deref(),
                    max_tokens,
                    tx_clone,
                ).await {
                    let _ = tx.send(StreamEvent::error(&e.to_string())).await;
                    let _ = tx.send(StreamEvent::complete(false)).await;
                }
            });

            // We don't have a real child process, so we create a dummy SessionHandle
            // The tokio task handle serves as our "process"
            Ok(SessionHandle::from_task(task_id_owned, rx, handle))
        })
    }

    fn interactive_command(&self, _session_id: &str) -> Vec<String> {
        // Direct API sessions can't be resumed interactively
        // Return a helpful message instead
        vec!["echo".to_string(), "Direct API sessions cannot be resumed".to_string()]
    }

    fn harness(&self) -> Harness {
        Harness::DirectApi
    }
}
```

#### 2. Extend Harness enum
**File**: `scud-cli/src/commands/spawn/terminal.rs`

Add `DirectApi` variant:
```rust
pub enum Harness {
    Claude,
    OpenCode,
    Cursor,
    #[cfg(feature = "direct-api")]
    DirectApi,
}

impl Harness {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Harness::Claude),
            "opencode" | "open-code" => Ok(Harness::OpenCode),
            "cursor" | "cursor-agent" => Ok(Harness::Cursor),
            #[cfg(feature = "direct-api")]
            "direct-api" | "direct" | "api" => Ok(Harness::DirectApi),
            other => anyhow::bail!("Unknown harness: '{}'", other),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Cursor => "cursor",
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => "direct-api",
        }
    }

    pub fn binary_name(&self) -> &'static str {
        match self {
            // ... existing ...
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => "scud", // Uses scud itself
        }
    }

    pub fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
        match self {
            // ... existing ...
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                format!(
                    r#"'{}' agent-exec --prompt-file '{}'{}"#,
                    binary_path,
                    prompt_file.display(),
                    model_flag
                )
            }
        }
    }
}
```

#### 3. Extend AnyRunner
**File**: `scud-cli/src/commands/spawn/headless/runner.rs`

Add `DirectApi` variant to `AnyRunner`:
```rust
pub enum AnyRunner {
    Claude(ClaudeHeadless),
    OpenCode(OpenCodeHeadless),
    Cursor(CursorHeadless),
    #[cfg(feature = "direct-api")]
    DirectApi(DirectApiRunner),
}

impl AnyRunner {
    pub fn new(harness: Harness) -> Result<Self> {
        match harness {
            Harness::Claude => Ok(AnyRunner::Claude(ClaudeHeadless::new()?)),
            Harness::OpenCode => Ok(AnyRunner::OpenCode(OpenCodeHeadless::new()?)),
            Harness::Cursor => Ok(AnyRunner::Cursor(CursorHeadless::new()?)),
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => Ok(AnyRunner::DirectApi(DirectApiRunner::new())),
        }
    }
    // ... extend start(), interactive_command(), harness() match arms
}
```

#### 4. Extend SessionHandle for tokio tasks
**File**: `scud-cli/src/commands/spawn/headless/runner.rs`

Add a constructor for task-based sessions (no child process):
```rust
impl SessionHandle {
    /// Create a SessionHandle backed by a tokio task instead of a child process
    pub fn from_task(
        task_id: String,
        events: mpsc::Receiver<StreamEvent>,
        task_handle: tokio::task::JoinHandle<()>,
    ) -> Self {
        // Store task handle for cancellation
        SessionHandle {
            task_id,
            session_id: None,
            child: ??? // Need to handle this - either make child optional or use a wrapper
            events,
        }
    }
}
```

**Note:** `SessionHandle.child` is currently `Child` (tokio process). For the direct-api case there's no child process. Options:
- Make `child` an `Option<Child>` and add a `task_handle: Option<JoinHandle<()>>`
- Create an enum wrapper: `enum SessionProcess { Child(Child), Task(JoinHandle<()>) }`
- Use the second approach since `wait()`, `interrupt()`, `kill()` need different implementations

```rust
enum SessionProcess {
    Child(Child),
    Task(tokio::task::JoinHandle<()>),
}

pub struct SessionHandle {
    pub task_id: String,
    pub session_id: Option<String>,
    process: SessionProcess,
    pub events: mpsc::Receiver<StreamEvent>,
}

impl SessionHandle {
    pub async fn wait(self) -> Result<bool> {
        match self.process {
            SessionProcess::Child(mut child) => {
                let status = child.wait().await?;
                Ok(status.success())
            }
            SessionProcess::Task(handle) => {
                let _ = handle.await;
                Ok(true)
            }
        }
    }

    pub fn interrupt(&mut self) -> Result<()> {
        match &mut self.process {
            SessionProcess::Child(child) => { /* existing SIGINT logic */ }
            SessionProcess::Task(handle) => { handle.abort(); Ok(()) }
        }
    }
}
```

#### 5. New subcommand: `scud agent-exec`
**File**: `scud-cli/src/commands/agent_exec.rs` (NEW)

For tmux-based modes (ralph, swarm), runs the agentic loop in the foreground so it can be spawned inside tmux windows:

```rust
/// Execute an agent loop using direct API calls.
/// This command is designed to be spawned in tmux windows by ralph/swarm,
/// replacing `claude -p` with a direct API agentic loop.
pub async fn run(prompt: Option<String>, prompt_file: Option<PathBuf>, model: Option<String>) -> Result<()> {
    let prompt = if let Some(f) = prompt_file {
        std::fs::read_to_string(&f)?
    } else {
        prompt.unwrap_or_default()
    };

    let working_dir = std::env::current_dir()?;
    let (tx, mut rx) = mpsc::channel(1000);

    // Spawn agent loop
    let agent_handle = tokio::spawn(async move {
        agent::run_agent_loop(&prompt, None, &working_dir, model.as_deref(), 16_000, tx).await
    });

    // Print events to stdout (visible in tmux)
    while let Some(event) = rx.recv().await {
        match &event.kind {
            StreamEventKind::TextDelta { text } => print!("{}", text),
            StreamEventKind::ToolStart { tool_name, input_summary, .. } => {
                eprintln!("\n[{}] {}", tool_name, input_summary);
            }
            StreamEventKind::ToolResult { tool_name, success, .. } => {
                eprintln!("[{}] {}", tool_name, if *success { "ok" } else { "FAILED" });
            }
            StreamEventKind::Error { message } => eprintln!("\nERROR: {}", message),
            StreamEventKind::Complete { success } => {
                if !success { eprintln!("\nAgent completed with errors"); }
                break;
            }
            _ => {}
        }
    }

    agent_handle.await??;
    Ok(())
}
```

#### 6. Register subcommand in CLI
**File**: `scud-cli/src/main.rs` (or wherever clap commands are registered)

Add `agent-exec` subcommand behind `#[cfg(feature = "direct-api")]`.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli --features direct-api`
- [ ] `Harness::parse("direct-api")` returns `Harness::DirectApi`
- [ ] `AnyRunner::new(Harness::DirectApi)` creates a `DirectApiRunner`
- [ ] `scud agent-exec --prompt-file /tmp/test.txt` runs the agent loop (manual test)
- [ ] `cargo build -p scud-cli` (without `direct-api` feature) still compiles - no regressions

#### Manual Verification:
- [ ] `scud agent-exec --prompt-file <file>` executes tools and prints output in terminal
- [ ] `scud ralph --harness direct-api --tag <tag>` completes a simple task
- [ ] TUI headless mode shows streaming events from DirectApiRunner

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Config Integration & Feature Flag

### Overview
Wire up the config flag and Cargo feature so the direct-api mode is properly gated and configurable.

### Changes Required:

#### 1. Cargo feature
**File**: `scud-cli/Cargo.toml`

```toml
[features]
default = []
real-llm = []
real-terminal = []
zmq = ["dep:zmq"]
direct-api = []  # Enable direct Anthropic API agentic runner
```

#### 2. Config flag
**File**: `scud-cli/src/config.rs`

Add to `SwarmConfig`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    #[serde(default = "default_swarm_harness")]
    pub harness: String,
    #[serde(default = "default_round_size")]
    pub round_size: usize,
    #[serde(default = "default_default_tag")]
    pub default_tag: Option<String>,
    /// Use direct Anthropic API instead of CLI harnesses.
    /// Requires `direct-api` Cargo feature.
    #[serde(default)]
    pub use_direct_api: bool,
}
```

Also add env var override:
```rust
fn default_swarm_harness() -> String {
    if std::env::var("SCUD_USE_DIRECT_API").is_ok() {
        return "direct-api".to_string();
    }
    std::env::var("SCUD_HARNESS").unwrap_or_else(|_| "claude".to_string())
}
```

#### 3. Config-to-harness resolution
**File**: `scud-cli/src/commands/ralph.rs` and `spawn/mod.rs`

When `use_direct_api` is true in config, override harness to `Harness::DirectApi`:

```rust
let harness = if config.swarm.use_direct_api {
    #[cfg(feature = "direct-api")]
    { Harness::DirectApi }
    #[cfg(not(feature = "direct-api"))]
    {
        eprintln!("Warning: use_direct_api is set but direct-api feature not compiled. Using claude harness.");
        Harness::parse(harness_arg)?
    }
} else {
    Harness::parse(harness_arg)?
};
```

### Success Criteria:

#### Automated Verification:
- [ ] `.scud/config.toml` with `use_direct_api = true` causes harness override
- [ ] `SCUD_USE_DIRECT_API=1 scud ralph ...` uses direct-api harness
- [ ] Missing feature flag produces helpful warning, not a crash

---

## Testing Strategy

### Unit Tests:
- OAuth token parsing and validation (mock keychain output)
- Tool execution: Read, Write, Edit, Bash, Search, Find
- Edit tool: unique match validation, error cases
- Bash tool: timeout enforcement
- API request building: OAuth headers vs API key headers
- System prompt construction for OAuth vs API key

### Integration Tests (behind `real-llm` feature):
- Full agent loop: prompt -> tool use -> completion
- Multi-turn conversation with tool calls
- Error handling: API errors, tool execution failures
- Large output truncation

### Manual Testing Steps:
1. `scud agent-exec --prompt "Read the file src/main.rs and tell me what it does"` - verify Read tool works
2. `scud agent-exec --prompt "Create a file /tmp/scud-test.txt with 'hello world'"` - verify Write tool works
3. `scud agent-exec --prompt "Search for 'fn main' in this project"` - verify Search tool works
4. `scud ralph --harness direct-api --tag <tag> --max-iterations 1` - verify integration with ralph loop
5. Verify OAuth token is used (check logs for Bearer auth vs x-api-key)

## Performance Considerations

- Non-streaming API calls in Phase 3 (adds latency). Streaming can be added later as a follow-up.
- Tool execution is sequential (same as Claude Code). Parallel tool execution could be a future optimization.
- Bash tool timeout defaults to 2 minutes, matching Claude Code behavior.
- Output truncation at 30KB prevents memory issues from large file reads or command output.

## Migration Notes

- Feature is opt-in: no changes to existing behavior unless `direct-api` feature is compiled and `use_direct_api` is set
- Existing harnesses (Claude, OpenCode, Cursor) continue to work unchanged
- Config file is backward-compatible: `use_direct_api` defaults to `false`
- If OAuth token expires mid-session, the agent will fail and ralph/swarm will mark the task as failed (normal failure handling)

## References

- OAuth research: `thoughts/shared/research/2026-02-13-claude-code-authentication-flow.md`
- Headless architecture: `thoughts/shared/research/2026-02-03-headless-mode-architecture.md`
- Current LLM client: `scud-cli/src/llm/client.rs`
- HeadlessRunner trait: `scud-cli/src/commands/spawn/headless/runner.rs:84-105`
- Ralph loop: `scud-cli/src/commands/ralph.rs:156-312`
- Config: `scud-cli/src/config.rs`
- Anthropic Messages API: https://docs.anthropic.com/en/api/messages
