---
date: 2026-02-03T12:00:00-08:00
researcher: Claude
git_commit: bf71b9da139ceb84571bbc85feb2de5b674bddc6
branch: master
repository: scud
topic: "Headless Mode Architecture: Replacing Tmux with Streaming CLI"
tags: [research, architecture, headless, claude-code, opencode, streaming, spawn, swarm]
status: complete
last_updated: 2026-02-03
last_updated_by: Claude
---

# Research: Headless Mode Architecture - Replacing Tmux with Streaming CLI

**Date**: 2026-02-03T12:00:00-08:00
**Researcher**: Claude
**Git Commit**: bf71b9da139ceb84571bbc85feb2de5b674bddc6
**Branch**: master
**Repository**: scud

## Research Question

Instead of using tmux and running agents then attaching, what if we run them in headless mode with streaming output, store that stream, and display it in the monitor/Descartes GUI? We could interrupt the process and launch an interactive session using the same session ID to continue.

## Summary

The current SCUD spawn system uses **tmux** as the execution environment for AI coding agents. This research documents how a **headless streaming architecture** could replace tmux, using:

1. **Claude Code**: `claude -p` with `--output-format stream-json` for streaming, `--resume <session_id>` to continue sessions
2. **OpenCode**: `opencode serve` server mode with HTTP/SSE API, `opencode attach` for interactive continuation

This approach would be cleaner because:
- No tmux dependency required
- Structured JSON event streams instead of terminal scraping
- Native session continuation without terminal state management
- Better integration with GUI layers (monitor TUI, Descartes GUI)

## Detailed Findings

### Current Architecture: Tmux-Based Spawning

The current spawn system in `scud-cli/src/commands/spawn/` uses tmux as the execution layer:

```
┌─────────────────────────────────────────────────────────────┐
│                    Current Architecture                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  scud spawn                                                  │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────┐                                            │
│  │ terminal.rs │ ──► tmux new-session / new-window          │
│  └─────────────┘                                            │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  tmux session "scud-{tag}"                          │    │
│  │  ├── ctrl (monitoring window)                       │    │
│  │  ├── task-{id} (claude/opencode interactive)        │    │
│  │  ├── task-{id} (claude/opencode interactive)        │    │
│  │  └── ...                                            │    │
│  └─────────────────────────────────────────────────────┘    │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────┐                                            │
│  │  monitor.rs │ ──► tmux capture-pane for output           │
│  └─────────────┘                                            │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────┐                                            │
│  │   TUI/GUI   │ ◄── scraped terminal content               │
│  └─────────────┘                                            │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

Key files:
- `scud-cli/src/commands/spawn/terminal.rs` - Tmux session/window management
- `scud-cli/src/commands/spawn/monitor.rs` - Agent state tracking via JSON files
- `scud-cli/src/commands/spawn/tui/app.rs` - TUI that reads tmux pane content

### Claude Code Headless Mode

From the Claude Code documentation, headless mode (`-p` flag) provides:

**Basic Usage:**
```bash
claude -p "Your prompt here"
```

**Streaming Output:**
```bash
claude -p "Explain recursion" \
  --output-format stream-json \
  --verbose \
  --include-partial-messages
```

Each line is a JSON event:
- `type: "stream_event"` with `event.delta.type: "text_delta"` for text
- Tool calls, completions, etc.

**Session Continuation:**
```bash
# First request - capture session ID
session_id=$(claude -p "Start a review" --output-format json | jq -r '.session_id')

# Continue that specific session
claude -p "Continue that review" --resume "$session_id"

# Or continue most recent
claude -p "Now focus on database queries" --continue
```

**Auto-Approve Tools:**
```bash
claude -p "Run tests and fix failures" \
  --allowedTools "Bash,Read,Edit"
```

### OpenCode Server Mode

OpenCode provides a server mode (`opencode serve`) with HTTP API:

**Start Server:**
```bash
opencode serve --port 4096 --hostname 0.0.0.0
```

**Attach TUI to Running Server:**
```bash
opencode attach http://localhost:4096
```

**Run Commands Against Server:**
```bash
# Avoid MCP cold boot on every run
opencode serve  # In one terminal

# Run commands that attach to it
opencode run --attach http://localhost:4096 "Explain async/await"
```

**HTTP API Endpoints:**
- `POST /session` - Create session
- `POST /session/{id}/message` - Send prompt
- `GET /session/{id}` - Get status
- `POST /session/{id}/abort` - Cancel
- `GET /event` - SSE event stream

SCUD already has an OpenCode client implementation:
- `scud-cli/src/opencode/client.rs` - HTTP client
- `scud-cli/src/opencode/manager.rs` - Server lifecycle
- `scud-cli/src/opencode/orchestrator.rs` - Agent orchestration
- `scud-cli/src/opencode/events.rs` - SSE event parsing

### Proposed Architecture: Headless Streaming

```
┌─────────────────────────────────────────────────────────────┐
│                   Proposed Architecture                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  scud spawn --headless                                       │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────────┐                                        │
│  │ HeadlessRunner  │                                        │
│  └─────────────────┘                                        │
│       │                                                      │
│       ├──► Claude: claude -p --output-format stream-json    │
│       │                                                      │
│       └──► OpenCode: opencode serve + HTTP API              │
│                                                              │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  StreamStore (per agent)                            │    │
│  │  ├── session_id: "sess-abc123"                      │    │
│  │  ├── events: Vec<StreamEvent>                       │    │
│  │  ├── status: Running | Completed | Failed           │    │
│  │  └── output_buffer: String                          │    │
│  └─────────────────────────────────────────────────────┘    │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────────┐    ┌─────────────────┐                 │
│  │  Monitor TUI    │    │  Descartes GUI  │                 │
│  │  (ratatui)      │    │  (iced)         │                 │
│  └─────────────────┘    └─────────────────┘                 │
│       │                        │                             │
│       └────────────────────────┘                             │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Interactive Takeover                               │    │
│  │  ├── Claude: claude --resume <session_id>           │    │
│  │  └── OpenCode: opencode attach <server_url>         │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Key Components to Build

#### 1. HeadlessRunner Trait

```rust
/// Abstraction over headless execution modes
pub trait HeadlessRunner {
    /// Start an agent with a prompt, return session ID
    async fn start(&self, prompt: &str, model: Option<&str>) -> Result<SessionHandle>;
    
    /// Get event stream for a session
    fn event_stream(&self, session_id: &str) -> impl Stream<Item = StreamEvent>;
    
    /// Interrupt/cancel a running session
    async fn interrupt(&self, session_id: &str) -> Result<()>;
    
    /// Launch interactive mode to continue session
    async fn attach_interactive(&self, session_id: &str) -> Result<()>;
}
```

#### 2. Claude Headless Implementation

```rust
pub struct ClaudeHeadless {
    binary_path: PathBuf,
    allowed_tools: Vec<String>,
}

impl HeadlessRunner for ClaudeHeadless {
    async fn start(&self, prompt: &str, model: Option<&str>) -> Result<SessionHandle> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--verbose");
        cmd.arg("--include-partial-messages");
        
        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }
        
        // Spawn and capture stdout for streaming
        let child = cmd.stdout(Stdio::piped()).spawn()?;
        
        // Parse first event to get session_id
        // Return handle with child process and session_id
    }
    
    async fn attach_interactive(&self, session_id: &str) -> Result<()> {
        // Launch: claude --resume <session_id>
        // This replaces current process with interactive claude
    }
}
```

#### 3. OpenCode Server Implementation

SCUD already has most of this in `scud-cli/src/opencode/`:

```rust
pub struct OpenCodeServer {
    manager: Arc<OpenCodeManager>,
}

impl HeadlessRunner for OpenCodeServer {
    async fn start(&self, prompt: &str, model: Option<&str>) -> Result<SessionHandle> {
        self.manager.ensure_running().await?;
        let client = self.manager.client();
        let session = client.create_session("Task").await?;
        client.send_message(&session.id, prompt, model).await?;
        // Return handle with session_id
    }
    
    async fn attach_interactive(&self, session_id: &str) -> Result<()> {
        // Launch: opencode attach http://localhost:4096 --session <session_id>
    }
}
```

#### 4. StreamStore

```rust
/// Stores streaming output for display in TUI/GUI
pub struct StreamStore {
    sessions: HashMap<String, SessionStream>,
}

pub struct SessionStream {
    session_id: String,
    task_id: String,
    events: Vec<StreamEvent>,
    output_lines: Vec<String>,
    status: SessionStatus,
    started_at: Instant,
}

pub enum StreamEvent {
    TextDelta { text: String },
    ToolStart { name: String, input: Value },
    ToolResult { name: String, success: bool },
    Complete { success: bool },
    Error { message: String },
}
```

### Integration Points

#### Monitor TUI Changes

Current `app.rs` uses `tmux capture-pane` to get output:

```rust
// Current: tmux capture-pane
let output = Command::new("tmux")
    .args(["capture-pane", "-t", &window_target, "-p", "-S", "-100"])
    .output();
```

Would become:

```rust
// Proposed: read from StreamStore
let output = self.stream_store.get_output(&session_id);
```

#### Descartes GUI Changes

Current `scud_bridge.rs` spawns `scud swarm --json-events` subprocess:

```rust
// Current: subprocess with JSON events
Command::new("scud")
    .args(&["swarm", "--tag", tag, "--json-events"])
    .stdout(Stdio::piped())
    .spawn()
```

Would become:

```rust
// Proposed: direct integration with HeadlessRunner
let runner = ClaudeHeadless::new();
let handle = runner.start(&prompt, model).await?;

// Stream events directly
while let Some(event) = handle.event_stream().next().await {
    self.event_tx.send(event.into()).await?;
}
```

### Benefits

1. **No tmux dependency** - Works on systems without tmux installed
2. **Structured events** - JSON events instead of terminal scraping
3. **Native session continuation** - Use `--resume` / `attach` instead of terminal state
4. **Better error handling** - Structured error events vs parsing terminal output
5. **Cleaner architecture** - Direct event streaming vs polling tmux panes
6. **Cross-platform** - No Unix-specific terminal multiplexer

### Migration Path

1. **Phase 1**: Add `--headless` flag to `scud spawn` that uses new architecture
2. **Phase 2**: Update monitor TUI to read from StreamStore when in headless mode
3. **Phase 3**: Update Descartes GUI to use HeadlessRunner directly
4. **Phase 4**: Make headless the default, deprecate tmux mode

### Open Questions

1. **Session persistence**: How to persist session IDs across SCUD restarts?
2. **Output storage**: Store full event stream or just rendered output?
3. **Interactive takeover UX**: How to smoothly transition from headless to interactive?
4. **Multiple harness support**: Unified interface vs harness-specific implementations?

## Code References

- `scud-cli/src/commands/spawn/terminal.rs:183-378` - Current tmux spawning logic
- `scud-cli/src/commands/spawn/tui/app.rs:262-329` - TUI output refresh via tmux
- `scud-cli/src/opencode/client.rs` - Existing OpenCode HTTP client
- `scud-cli/src/opencode/manager.rs` - Server lifecycle management
- `scud-cli/src/opencode/orchestrator.rs` - Agent orchestration
- `descartes-gui/src/scud_bridge.rs:419-491` - GUI swarm execution

## Architecture Documentation

### Current Harness Abstraction

The `Harness` enum in `terminal.rs` already abstracts over Claude/OpenCode:

```rust
pub enum Harness {
    Claude,
    OpenCode,
}

impl Harness {
    pub fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
        match self {
            Harness::Claude => format!(
                r#"'{}' "$(cat '{}')" --dangerously-skip-permissions{}"#,
                binary_path, prompt_file.display(), model_flag
            ),
            Harness::OpenCode => format!(
                r#"'{}'{} run --variant minimal "$(cat '{}')""#,
                binary_path, model_flag, prompt_file.display()
            ),
        }
    }
}
```

This would be extended to support headless mode:

```rust
impl Harness {
    pub fn headless_command(&self, prompt: &str, model: Option<&str>) -> Command {
        match self {
            Harness::Claude => {
                let mut cmd = Command::new("claude");
                cmd.arg("-p").arg(prompt);
                cmd.arg("--output-format").arg("stream-json");
                // ...
                cmd
            },
            Harness::OpenCode => {
                // Use HTTP API via OpenCodeClient
            }
        }
    }
}
```

## Historical Context (from thoughts/)

No existing research documents found on this specific topic.

## Related Research

- OpenCode server mode is already partially implemented in `scud-cli/src/opencode/`
- The existing `AgentOrchestrator` in `orchestrator.rs` provides a foundation for the headless approach

## Open Questions

1. Should we maintain tmux as a fallback for users who prefer it?
2. How to handle the "Ralph loop" (autonomous retry) in headless mode?
3. What's the best way to persist session state for crash recovery?
4. Should the StreamStore be in-memory only or persisted to disk?
