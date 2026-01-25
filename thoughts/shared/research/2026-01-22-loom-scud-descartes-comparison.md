---
date: 2026-01-22T00:00:00-08:00
researcher: Claude
git_commit: 6807cad3db74503b23fc7e289ccc17c5a5919909
branch: trunk
repository: loom
topic: "Loom vs Scud vs Descartes Comparison"
tags: [research, comparison, architecture, ai-agents]
status: complete
last_updated: 2026-01-22
last_updated_by: Claude
---

# Comparison: Loom vs Scud vs Descartes

## Executive Summary

| Project | Core Purpose | Language | Key Abstraction |
|---------|--------------|----------|-----------------|
| **Loom** | AI coding assistant with enterprise observability | Rust | Server-side LLM proxy + Thread conversations |
| **Scud** | AI task orchestration via DAG scheduling | Rust | Task dependency graph (SCG format) |
| **Descartes** | AI agent harness abstraction | Rust | Harnesses (Claude Code, OpenCode, Codex) |

**Relationship**: Descartes wraps AI tools and uses Scud for task management. Loom is a separate, more comprehensive platform that could potentially benefit from Scud's task orchestration patterns.

---

## Architecture Comparison

### Loom: Full-Stack AI Platform

```
┌─────────────────────────────────────────────────────────────────┐
│                    User Interfaces                               │
│  CLI (loom)  │  Web UI (Svelte)  │  VS Code (ACP)  │  Weavers   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                     loom-server (Axum)                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐ │
│  │ Auth/ABAC│  │ LLM Proxy│  │ Thread   │  │  Observability   │ │
│  │  OAuth   │  │ Anthropic│  │   API    │  │ Analytics/Crash  │ │
│  │  SCIM    │  │ OpenAI   │  │   Sync   │  │ Crons/Sessions   │ │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘ │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────────────────┐│
│  │ Weaver   │  │   SCM    │  │   SQLite (threads, users, etc.)  ││
│  │ K8s Pods │  │ Git Host │  └──────────────────────────────────┘│
│  └──────────┘  └──────────┘                                      │
└─────────────────────────────────────────────────────────────────┘
```

**Key traits**:
- **Centralized LLM proxy** - API keys never on client
- **Thread-based conversations** - Persistent, syncable state
- **Enterprise features** - Multi-tenant, ABAC, audit logging, SCIM
- **Remote execution** - K8s pods with SPIFFE auth
- **Full observability** - Analytics, crash tracking, cron monitoring

### Scud: Task DAG Scheduler

```
┌─────────────────────────────────────────────────────────────────┐
│                      scud CLI                                    │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                    Task Graph (.scud/)                           │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                SCG Format (tasks.scg)                        ││
│  │  @nodes                                                      ││
│  │  auth:1 | Design auth system | X | 13 | H                   ││
│  │  auth:1.1 | Implement JWT | D | 5 | H                       ││
│  │  @edges                                                      ││
│  │  auth:1.1 -> auth:1                                         ││
│  └─────────────────────────────────────────────────────────────┘│
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│              Wave-Based Execution                                │
│  ┌────────┐    ┌────────┐    ┌────────┐                        │
│  │ Wave 1 │ -> │ Wave 2 │ -> │ Wave 3 │                        │
│  │(ready) │    │(blocked)│    │(blocked)│                       │
│  │ T1, T2 │    │   T3    │    │   T4    │                       │
│  └────────┘    └────────┘    └────────┘                        │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│         Terminal Spawning (tmux, Kitty, WezTerm, etc.)          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                      │
│  │ Agent 1  │  │ Agent 2  │  │ Agent 3  │                      │
│  │(claude)  │  │(claude)  │  │(opencode)│                      │
│  └──────────┘  └──────────┘  └──────────┘                      │
└─────────────────────────────────────────────────────────────────┘
```

**Key traits**:
- **DAG-driven scheduling** - Tasks ready when deps complete
- **Token-efficient format** - SCG achieves ~75% reduction vs JSON
- **Wave computation** - Parallel execution visualization
- **Terminal spawning** - Real terminal windows with live output
- **PRD-to-tasks** - LLM generates task graphs from documents

### Descartes: Harness Abstraction Layer

```
┌─────────────────────────────────────────────────────────────────┐
│                    descartes CLI                                 │
│  Interactive Session  │  Swarm Loop  │  Spawn Subagent          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                  Harness Abstraction                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ ClaudeCode   │  │  OpenCode    │  │    Codex     │          │
│  │  Harness     │  │   Harness    │  │   Harness    │          │
│  │ (headless)   │  │ (TUI/IPC)    │  │   (API)      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                   Agent Definitions                              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  .descartes/agents/code-analyzer/AGENT.md                  │ │
│  │  ---                                                        │ │
│  │  name: code-analyzer                                        │ │
│  │  category: analyzer                                         │ │
│  │  model: opus                                                │ │
│  │  skills: [research, review]                                 │ │
│  │  ---                                                        │ │
│  └────────────────────────────────────────────────────────────┘ │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                 SCUD Integration                                 │
│  Descartes swarm → writes guidance → calls scud swarm           │
└─────────────────────────────────────────────────────────────────┘
```

**Key traits**:
- **Harness abstraction** - Same interface for different AI tools
- **Subagent orchestration** - Searcher → Builder → Validator pipeline
- **Visible transcripts** - Full capture in SCG format
- **User guidance injection** - Customize agent behavior via config
- **SCUD delegation** - Now defers orchestration to Scud

---

## Feature Matrix

| Feature | Loom | Scud | Descartes |
|---------|------|------|-----------|
| **LLM Integration** |
| Multi-provider support | Yes (4 providers) | Via external tools | Via harnesses |
| Server-side API key management | Yes | No | No |
| Streaming SSE | Yes (custom parsers) | N/A | Via harness |
| **Conversation/Task Management** |
| Persistent conversations | Yes (threads) | Yes (SCG tasks) | No (delegates to Scud) |
| Dependency tracking | No | Yes (DAG) | Via Scud |
| Parallel execution | No | Yes (waves) | Via Scud |
| **Tool Execution** |
| File operations | Yes (read, edit, list) | Via spawned agents | Via harness |
| Shell commands | Yes (bash tool) | Via spawned agents | Via harness |
| Web search | Yes (Google CSE, Serper) | Via spawned agents | Via harness |
| **Multi-Agent** |
| Agent spawning | No (single agent) | Yes (terminal spawn) | Yes (subagents) |
| Wave scheduling | No | Yes | Via Scud |
| Backpressure/validation | No | Yes | Via Scud |
| **Infrastructure** |
| Remote execution | Yes (K8s weavers) | No | No |
| Authentication | Yes (OAuth, ABAC) | No | No |
| Multi-tenant | Yes (orgs, teams) | No | No |
| Audit logging | Yes | No | No |
| **Observability** |
| Product analytics | Yes | No | No |
| Crash tracking | Yes (symbolication) | No | No |
| Cron monitoring | Yes | No | No |
| Session health | Yes | No | No |
| **Version Control** |
| Custom VCS | Yes (Spool/jj) | No | No |
| Git hosting | Yes (SCM) | No | No |
| Auto-commit | Yes | No | No |

---

## What Loom Could Borrow from Scud

### 1. DAG-Based Task Orchestration

**Current Loom**: Single-agent REPL with linear conversation flow. No built-in concept of task dependencies or parallel work.

**Scud's approach**: Tasks form a DAG where dependencies unlock parallel execution. `scud waves` shows what can run concurrently.

**Potential adoption**:
```rust
// Thread could track a task graph
struct Thread {
    // ... existing fields ...
    task_graph: Option<TaskGraph>,  // Optional DAG
}

// Agent state could expand for multi-task
enum AgentState {
    // ... existing states ...
    ExecutingWave { wave: Vec<Task>, progress: HashMap<TaskId, Status> },
}
```

**Benefits**:
- Enable parallel tool execution within a session
- Track complex multi-step implementations
- Visualize work progress

### 2. ~~Wave-Based Parallel Execution for Weavers~~ (Not Applicable)

**Important clarification**: Weavers are **NOT** like Scud waves. They are fundamentally different concepts:

| Aspect | Loom Weavers | Scud Waves |
|--------|--------------|------------|
| **What they are** | Isolated K8s pods (compute environments) | Coordinated parallel task execution |
| **Coordination** | None - each weaver is independent | DAG-driven dependency resolution |
| **Purpose** | Provide isolated sandboxed environments | Maximize parallelism across tasks |
| **Lifecycle** | Long-running (4-48 hours TTL) | Ephemeral per-wave execution |
| **Communication** | SSH/WireGuard tunnels to single pod | No inter-task communication |

A weaver is essentially a "remote development environment" - an ephemeral K8s pod where you can run a Loom REPL session in isolation. There is no multi-weaver orchestration or coordination. Each weaver runs independently.

**If you wanted wave-like behavior in Loom**, you would need to build task orchestration on top of the existing infrastructure - weavers would be the compute substrate, but the scheduling logic would need to be added

### 3. SCG Format for Token-Efficient Storage

**Current Loom**: Thread messages stored as JSON (verbose).

**Scud's SCG format**: 75% token reduction, human-readable:
```
@nodes
T-abc123 | Implement auth | IP | 8 | H

@messages
U: Please implement JWT authentication
A: I'll create the auth module...
T[edit_file]: {"path": "auth.rs", ...}
```

**Potential adoption**:
- Alternative thread export format
- Efficient context windows for resumption
- Better diff-ability in version control

### 4. PRD-to-Tasks Generation

**Current Loom**: Users manually describe what they want in the REPL.

**Scud's approach**: `scud generate docs/feature.md` parses PRDs into structured task graphs.

**Potential adoption**:
- Add `/plan` command to generate implementation tasks from description
- Auto-expand complex requests into dependency-aware subtasks

---

## What Loom Could Borrow from Descartes

### 1. Harness Abstraction

**Current Loom**: Direct LLM client implementations (Anthropic, OpenAI, etc.).

**Descartes's approach**: `LlmHarness` trait abstracts different AI tools:
```rust
#[async_trait]
pub trait LlmHarness: Send + Sync {
    async fn create_session(&self, config: SessionConfig) -> Result<SessionHandle>;
    async fn send_message(&self, handle: &SessionHandle, msg: &str) -> Result<ResponseStream>;
    async fn close_session(&self, handle: &SessionHandle) -> Result<()>;
}
```

**Potential adoption**:
- Could wrap Claude Code, OpenCode, Codex as alternative frontends
- Enable "bring your own agent" for weavers
- Support open-source TUIs for privacy-sensitive users

### 2. User Guidance Injection

**Current Loom**: System prompts are fixed in code.

**Descartes's approach**: `[guidance]` config section injects custom context:
```toml
[guidance]
global = "Always follow existing code patterns."
builder = "Run tests after making changes."
validator = "Use cargo test --all-features."
```

**Potential adoption**:
```toml
# ~/.config/loom/config.toml
[guidance]
global = "This is a Rust project using Tokio. Prefer async/await."
edit_file = "Always run cargo fmt after edits."
bash = "Use cargo check before cargo build."
```

### 3. Agent Definitions

**Current Loom**: Fixed tool implementations.

**Descartes's approach**: Custom agents via markdown files with YAML frontmatter:
```markdown
---
name: code-analyzer
category: analyzer
model: opus
skills: [research, review]
---

# Code Analyzer
You are an expert code analyst...
```

**Potential adoption**:
- User-defined "personas" for different tasks
- Custom tool configurations per persona
- Sharable agent templates

### 4. Transcript Capture

**Current Loom**: Conversation stored but tool execution details minimal.

**Descartes's approach**: Full transcripts in SCG format with tool calls, timing, etc.

**Potential adoption**:
- Richer thread export for debugging
- Replay capability for training/analysis
- Better visibility into agent reasoning

---

## What Scud/Descartes Could Borrow from Loom

### 0. Post-Tools Hook Pattern (High Value)

**Loom's approach**: The agent state machine has a `PostToolsHook` state that runs infrastructure operations (like auto-commit) *after* mutating tools complete, but *before* returning to the LLM:

```
ExecutingTools → PostToolsHook → CallingLlm
                    ↓
              (auto-commit, validation, etc.)
```

This keeps the LLM context clean (no commit messages polluting conversation) while enabling powerful automation.

**Potential adoption in Descartes**:
```rust
// Add post-subagent hooks for common operations
pub struct SubagentHooks {
    pub on_completion: Vec<Box<dyn Fn(&SubagentResult) -> Result<()>>>,
}

// Example hooks:
// - Auto-commit changes after builder completes
// - Run `cargo check` after code modifications
// - Update Scud task status
```

**Benefits**:
- Separates infrastructure concerns from LLM reasoning
- Enables automation without polluting agent context
- Consistent behavior across all harnesses

### 1. Server-Side LLM Proxy

**Current Scud/Descartes**: Users must have local API keys or Claude Code installed.

**Loom's approach**: Server proxies all LLM requests, keys never on client.

**Benefit**: Team can share LLM quota without distributing API keys.

### 2. Remote Execution (Weavers)

**Current Scud/Descartes**: Agents run in local terminal sessions.

**Loom's approach**: K8s pods with SPIFFE auth, WireGuard tunnels, eBPF audit.

**Benefit**: Isolated, auditable execution environments for sensitive work.

### 3. Observability Suite

**Current Scud/Descartes**: Limited logging, no analytics.

**Loom's approach**: Full analytics, crash tracking, session health.

**Benefit**: Understand how agents are being used, debug failures.

### 4. Enterprise Auth

**Current Scud/Descartes**: No auth (local tools).

**Loom's approach**: OAuth, ABAC, SCIM, audit logging.

**Benefit**: Required for enterprise deployment.

---

## Integration Opportunities

### Option A: Loom Uses Scud for Task Orchestration

```
User: "Implement authentication with JWT and OAuth"
     │
     ▼
┌────────────────────────────────────┐
│ Loom Server                        │
│  1. Parse request                  │
│  2. Generate task graph (SCG)      │
│  3. Compute waves                  │
│  4. Spawn weavers per wave         │
│  5. Validate between waves         │
└────────────────────────────────────┘
     │
     ▼
Weaver 1: Implement JWT     Weaver 2: Implement OAuth
(wave 1, parallel)          (wave 1, parallel)
     │                           │
     └─────────┬─────────────────┘
               ▼
Weaver 3: Integration tests (wave 2, depends on 1 & 2)
```

**Implementation**:
- Port `scud-cli/src/models/task.rs` → `loom-server-tasks`
- Port `scud-cli/src/formats/scg.rs` → `loom-common-scg`
- Add wave computation to thread/conversation flow
- Weaver provisioner picks tasks from ready queue

### Option B: Descartes Wraps Loom as a Harness

```rust
// In descartes/src/harness/loom.rs
pub struct LoomHarness {
    client: LoomApiClient,
    weaver_id: Option<String>,
}

#[async_trait]
impl LlmHarness for LoomHarness {
    async fn create_session(&self, config: SessionConfig) -> Result<SessionHandle> {
        // Create weaver or thread via Loom API
        let weaver = self.client.create_weaver(CreateWeaverRequest {
            image: "loom-agent".into(),
            model: config.model,
            ..Default::default()
        }).await?;
        Ok(SessionHandle { id: weaver.id, ... })
    }

    async fn send_message(&self, handle: &SessionHandle, msg: &str) -> Result<ResponseStream> {
        // Send to thread via Loom API, stream response
    }
}
```

**Benefits**:
- Descartes users get Loom's enterprise features
- Loom gains Scud's task orchestration patterns
- Unified observability across all agents

### Option C: Shared SCG Format Standard

Both projects could standardize on SCG for:
- Task/conversation storage
- Transcript capture
- Export/import interoperability

**Specification work**:
1. Document SCG format spec (Scud already has this)
2. Add SCG parser/writer to `loom-common-scg`
3. Enable thread export as SCG
4. Enable SCG task import into Loom weavers

---

## Inspiration vs Direct Borrowing

### Patterns to Adopt (conceptual, reimplement)

1. **DAG-based task scheduling** - The concept is valuable; implementation would need to fit Loom's async architecture
2. **Wave visualization** - Great UX pattern for showing parallel work
3. **Token-efficient formats** - SCG's design principles apply broadly
4. **User guidance injection** - Configuration-driven prompt customization

### Code to Potentially Port

1. **SCG parser** (`scud-cli/src/formats/scg.rs`) - Already Rust, well-tested
2. **Wave computation** (`scud-cli/src/commands/waves.rs`) - Core algorithm is reusable
3. **Agent definitions** (`descartes/src/agent/`) - Markdown-based config pattern

### Patterns That Don't Fit

1. **Terminal spawning** - Loom's weavers are K8s pods, not local terminals
2. **Local-first storage** - Loom is server-centric with sync
3. **Headless API mode** - Loom already has this via server proxy

---

## Recommended Next Steps

1. **Short-term**: Add `/plan` command to Loom that generates task graphs from descriptions
2. **Medium-term**: Port SCG format to Loom for efficient thread storage/export
3. **Long-term**: Wave-based weaver orchestration for complex implementations
4. **Exploration**: Create Loom harness in Descartes as integration proof-of-concept

---

## Summary

| Aspect | Loom | Scud | Descartes |
|--------|------|------|-----------|
| **Strength** | Enterprise platform, observability, remote execution | Task orchestration, parallel scheduling, token efficiency | Harness abstraction, subagent visibility |
| **Gap** | No task DAG, no parallel agents | No enterprise features, local-only | Delegates to Scud, minimal standalone |
| **Best for** | Teams needing managed AI coding with compliance | Individuals orchestrating complex implementations | Developers wanting unified agent interface |

The three projects are complementary rather than competing. Loom provides the platform; Scud provides the orchestration patterns; Descartes provides the harness abstraction. Integration could yield a powerful combined system.
