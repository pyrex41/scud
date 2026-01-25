# OpenCode SDK Deep Integration Research

## Current State

SCUD currently integrates with OpenCode by **spawning CLI subprocesses**:

```rust
// scud-cli/src/extensions/runner.rs:194-203
Command::new(&binary_path)
    .arg("run")
    .arg("--variant")
    .arg("minimal")
    .args(model_args)
    .arg(&prompt)
    .current_dir(&working_dir)
    .env("SCUD_TASK_ID", task_id)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
```

This approach:
- ✅ Works reliably
- ✅ Simple to implement
- ❌ No structured communication (just stdout/stderr text)
- ❌ Limited control over agent lifecycle
- ❌ Can't easily cancel/pause mid-execution
- ❌ No access to tool calls, thinking, or structured events
- ❌ Each agent spawns its own process with full initialization overhead

## Integration Options

### Option 1: OpenCode Server Mode + HTTP API

OpenCode can run as a headless server exposing a REST API + SSE streaming:

```bash
opencode serve --port 4096
```

**Key Endpoints:**
- `POST /session` - Create new session
- `POST /session/{id}/message` - Send message/prompt
- `GET /event` - SSE stream for real-time events
- `DELETE /session/{id}` - Clean up session
- `POST /session/{id}/abort` - Cancel running operation

**Architecture:**
```
┌─────────────────────────────────────────────────────────┐
│                     SCUD Swarm                          │
│                                                         │
│  ┌──────────┐  HTTP/SSE   ┌──────────────────────────┐ │
│  │ Rust     │◄───────────►│ OpenCode Server          │ │
│  │ Executor │             │ (single instance)        │ │
│  └──────────┘             │                          │ │
│       │                   │  ┌────────┐ ┌────────┐   │ │
│       │                   │  │Session1│ │Session2│   │ │
│       ▼                   │  │ Task A │ │ Task B │   │ │
│  Task Status              │  └────────┘ └────────┘   │ │
│  Updates                  └──────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

**Benefits:**
- Single server process handles multiple concurrent sessions
- Structured JSON events via SSE (tool calls, thinking, completions)
- Proper abort/cancel support
- Lower per-agent overhead (no process spawn per task)
- Access to full event stream (tool use, file edits, etc.)

**Implementation in Rust:**
```rust
use reqwest::Client;
use eventsource_client::{Client as SseClient, SSE};

struct OpenCodeClient {
    base_url: String,
    client: Client,
}

impl OpenCodeClient {
    async fn create_session(&self, title: &str) -> Result<Session> {
        self.client.post(&format!("{}/session", self.base_url))
            .json(&json!({"title": title}))
            .send().await?
            .json().await
    }

    async fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<()> {
        self.client.post(&format!("{}/session/{}/message", self.base_url, session_id))
            .json(&json!({
                "parts": [{"type": "text", "text": prompt}]
            }))
            .send().await?;
        Ok(())
    }

    fn subscribe_events(&self) -> impl Stream<Item = OpenCodeEvent> {
        let url = format!("{}/event", self.base_url);
        SseClient::for_url(&url).unwrap()
            .stream()
            .filter_map(|event| parse_opencode_event(event))
    }
}
```

### Option 2: OpenCode SDK (TypeScript/JavaScript)

The `@opencode-ai/sdk` npm package provides a type-safe client:

```typescript
import { createOpencode, createOpencodeClient } from '@opencode-ai/sdk'

// Start server + client together
const { client } = await createOpencode()

// Or connect to existing server
const client = createOpencodeClient({ baseUrl: "http://localhost:4096" })

// Create session and send prompt
const session = await client.session.create({ body: { title: "Task 1" } })
await client.session.prompt({
    path: { id: session.id },
    body: {
        model: { providerID: "anthropic", modelID: "claude-sonnet-4-20250514" },
        parts: [{ type: "text", text: "Implement feature X" }]
    }
})

// Subscribe to events
const events = client.event.subscribe()
for await (const event of events) {
    console.log(event.type, event.data)
}
```

**Benefits:**
- Full type safety
- Handles server lifecycle
- Clean async/await API

**Drawbacks for SCUD:**
- Requires Node.js runtime
- SCUD is primarily Rust-based
- Would need to bridge via subprocess or embed Node

### Option 3: Hybrid - Rust HTTP Client to OpenCode Server

Best of both worlds for SCUD's Rust codebase:

1. SCUD starts `opencode serve` as a managed subprocess
2. SCUD uses `reqwest` + SSE client to communicate
3. Multiple sessions share single server instance
4. Full event stream access for monitoring UI

**Proposed Architecture:**
```rust
pub struct OpenCodeOrchestrator {
    server_process: Option<Child>,
    client: OpenCodeClient,
    sessions: HashMap<String, SessionHandle>,
}

impl OpenCodeOrchestrator {
    pub async fn new() -> Result<Self> {
        // Start server if not already running
        let server = Command::new("opencode")
            .args(["serve", "--port", "4096"])
            .spawn()?;

        // Wait for server ready
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(Self {
            server_process: Some(server),
            client: OpenCodeClient::new("http://127.0.0.1:4096"),
            sessions: HashMap::new(),
        })
    }

    pub async fn spawn_agent(&mut self, task: &Task, prompt: &str) -> Result<SessionHandle> {
        let session = self.client.create_session(&task.title).await?;
        self.client.send_prompt(&session.id, prompt).await?;

        let handle = SessionHandle {
            session_id: session.id.clone(),
            task_id: task.id.clone(),
        };
        self.sessions.insert(task.id.clone(), handle.clone());
        Ok(handle)
    }

    pub async fn cancel_agent(&mut self, task_id: &str) -> Result<()> {
        if let Some(handle) = self.sessions.get(task_id) {
            self.client.abort_session(&handle.session_id).await?;
        }
        Ok(())
    }

    pub fn event_stream(&self) -> impl Stream<Item = AgentEvent> {
        self.client.subscribe_events()
            .filter_map(|e| self.map_to_agent_event(e))
    }
}
```

## Event Types Available via Server

When using server mode, you get structured events:

```json
{"type": "message.start", "data": {"session_id": "...", "message_id": "..."}}
{"type": "text.delta", "data": {"text": "Let me analyze..."}}
{"type": "tool.start", "data": {"tool": "read_file", "input": {"path": "src/main.rs"}}}
{"type": "tool.result", "data": {"tool": "read_file", "output": "..."}}
{"type": "message.complete", "data": {"session_id": "...", "success": true}}
```

This enables:
- Real-time streaming to TUI monitor
- Tracking which files agents are reading/editing
- Detecting tool failures before completion
- Building detailed execution logs

## Comparison Matrix

| Feature | CLI Subprocess | Server + HTTP | SDK (TS) |
|---------|---------------|---------------|----------|
| Language | Rust native | Rust native | TypeScript |
| Process overhead | High (per agent) | Low (shared) | Low (shared) |
| Structured events | ❌ | ✅ | ✅ |
| Cancel support | ❌ Kill process | ✅ Graceful | ✅ Graceful |
| Type safety | N/A | Manual | ✅ Full |
| Implementation effort | Done | Medium | High (bridge) |
| Tool call visibility | ❌ | ✅ | ✅ |
| File edit tracking | ❌ | ✅ | ✅ |

## Recommendation

**Short-term:** Keep CLI subprocess approach for stability.

**Medium-term:** Implement Option 3 (Rust HTTP client to OpenCode Server):

1. Add `OpenCodeOrchestrator` that manages server lifecycle
2. Create sessions via HTTP API instead of spawning processes
3. Stream events via SSE for real-time monitoring
4. Implement graceful cancellation

**Benefits:**
- Single server process for all agents (lower resource usage)
- Structured event stream for better monitoring
- Graceful abort/cancel support
- Foundation for richer TUI with tool call visibility
- No need for Node.js runtime

**Migration Path:**
1. Add `--orchestrator` flag to swarm command
2. Default to CLI subprocess (current behavior)
3. Opt-in to server mode for testing
4. Graduate to default once stable

## Questions to Resolve

1. **Server lifecycle:** Should SCUD manage the server, or expect user to run `opencode serve`?
2. **Multiple projects:** Can one server handle sessions in different working directories?
3. **Authentication:** Do we need password protection for local server?
4. **Port conflicts:** How to handle if port 4096 is already in use?
5. **Session cleanup:** How aggressively to clean up completed sessions?

## Next Steps

1. Prototype `OpenCodeClient` with reqwest + SSE
2. Test session creation and prompt sending
3. Validate event stream parsing
4. Benchmark: CLI subprocess vs server mode (memory, startup time)
5. Design TUI integration for event stream display
