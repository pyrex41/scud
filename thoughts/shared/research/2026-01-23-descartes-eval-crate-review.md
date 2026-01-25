---
date: 2026-01-23T00:00:00Z
researcher: Claude
git_commit: f2d683dffa8d71e5bd367dcda2a4f1de1dd427a4
branch: master
repository: scud
topic: "Descartes and scud-eval Crate Implementation Review"
tags: [research, descartes, scud-eval, scud-core, gui, evaluation]
status: complete
last_updated: 2026-01-23
last_updated_by: Claude
---

# Research: Descartes and scud-eval Crate Implementation Review

**Date**: 2026-01-23
**Researcher**: Claude
**Git Commit**: f2d683dffa8d71e5bd367dcda2a4f1de1dd427a4
**Branch**: master
**Repository**: scud

## Research Question

Review the work done by another agent on Descartes (descartes/ and descartes-gui/ directories) and the scud-eval crate. Understand what was implemented, the architecture, and current state.

## Summary

The workspace has been restructured into a monorepo with 5 crates working together. Three significant bodies of work have been completed:

1. **scud-core** - Fully extracted shared library with task models, SCG format, wave computation, and storage
2. **descartes + descartes-gui** - Complete AI agent orchestration system with CLI and Iced GUI
3. **scud-eval** - Evaluation framework for comparing execution modes (mostly complete)

### Implementation Status at a Glance

| Component | Status | Notes |
|-----------|--------|-------|
| scud-core | **Complete** | Foundation library with all core types |
| descartes CLI | **Complete** | 3 harnesses, transcripts, specs, SCUD integration |
| descartes-gui | **95% Complete** | Missing single-task agent spawning |
| scud-eval | **90% Complete** | Missing Claude Direct mode implementation |

---

## Detailed Findings

### 1. Workspace Structure

The repository is now a Cargo workspace with 5 members:

```
scud/
├── Cargo.toml              # Workspace root (version 1.44.0)
├── scud-cli/               # Main CLI (depends on nothing)
├── scud-core/              # Core library (foundation)
├── descartes/              # AI orchestration CLI (depends on scud-core)
├── descartes-gui/          # Desktop GUI (depends on descartes + scud-core)
└── scud-eval/              # Evaluation framework (depends on scud-core)
```

**Dependency Graph**:
```
scud-core (foundation)
    ↑
    ├── scud-cli (independent)
    ├── descartes (→ scud-core)
    │       ↑
    │       └── descartes-gui (→ descartes + scud-core)
    └── scud-eval (→ scud-core)
```

---

### 2. scud-core Implementation

**Location**: `scud-core/src/`

A fully-implemented shared library providing:

#### Models (`models/task.rs`, `models/phase.rs`)
- **Task** struct with 15+ fields including id, title, status, complexity, dependencies, subtasks
- **TaskStatus** enum with 9 states (Pending, InProgress, Done, Review, Blocked, Deferred, Cancelled, Expanded, Failed)
- **Priority** enum (Critical, High, Medium, Low)
- **Phase** struct for task collections with statistics
- Validation methods for IDs, titles, complexity (Fibonacci numbers only)
- Circular dependency detection via DFS
- Cross-phase dependency support with namespaced IDs (`epic:local_id`)

#### Formats (`formats/scg.rs`)
- **SCG format** parser and serializer - token-efficient graph-native format
- 7 sections: `@meta`, `@nodes`, `@edges`, `@parents`, `@assignments`, `@agents`, `@details`
- Single-character status codes (P=Pending, D=Done, etc.)
- Multi-phase file support with `---` separator
- Natural sorting for consistent output

#### Waves (`waves.rs`)
- **Kahn's algorithm** implementation for parallel wave computation
- Returns `WaveResult` with waves and circular dependency detection
- Cross-phase ID collision detection

#### Storage (`storage.rs`)
- File-based persistence with **exclusive file locking** via `fs2`
- Retry logic with exponential backoff (10ms to 1000ms, 10 retries)
- Active group caching with RwLock for thread safety
- Atomic read-modify-write operations

**Public API**:
```rust
// Re-exports from lib.rs
pub use models::{IdFormat, Phase, PhaseStats, Priority, Task, TaskStatus};
pub use formats::{natural_sort_ids, parse_scg, serialize_scg, Format};
pub use waves::{compute_waves, detect_id_collisions, Wave, WaveResult};
pub use storage::Storage;
```

---

### 3. Descartes CLI Implementation

**Location**: `descartes/src/` (package: `descartes-cli`, version: 0.3.0)

A complete AI agent orchestration system with:

#### Harness System (`harness/`)
Three fully-implemented harnesses:
- **ClaudeCodeHarness** - Uses `claude` CLI with `--output-format stream-json`
- **OpenCodeHarness** - Uses `opencode run --format json`
- **CodexHarness** - OpenAI-compatible API with function calling

Common trait:
```rust
pub trait Harness: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> HarnessKind;
    async fn start_session(&self, config: SessionConfig) -> Result<SessionHandle>;
    async fn send(&self, handle: &SessionHandle, message: &str) -> Result<ResponseStream>;
    fn detect_subagent_spawn(&self, content: &str) -> Option<SubagentRequest>;
    async fn inject_result(&self, handle: &SessionHandle, result: SubagentResult) -> Result<()>;
    async fn close_session(&self, handle: &SessionHandle) -> Result<()>;
}
```

#### Spec System (`spec.rs` - 2,224 lines)
"Fixed spec allocation" pattern for prompt building:
- **CodebaseContext** - Auto-include relevant file snippets via glob patterns
- **DependencyContext** - Include completed task summaries
- **VerificationConfig** - Configurable validation commands
- **TemplateRegistry** - Per-project prompt templates
- Placeholders: `{task}`, `{plan}`, `{codebase}`, `{dependencies}`, `{verification}`, `{custom}`

#### Agent System (`agent/`)
- **AgentCategory** enum: Searcher, Analyzer, Builder, Validator, Planner, FastBuilder, BuilderReviewer, Custom
- **AgentDefinition** loading from AGENT.md files with YAML frontmatter
- **AgentRegistry** for tracking active agents
- **spawn_subagent()** with 1-level depth limit

#### Transcript System (`transcript/`)
Full visibility recording in SCG format:
- All user/assistant messages
- Tool calls and results
- Subagent spawns and completions
- Metrics (tokens, duration)

#### SCUD Integration (`scud/mod.rs`)
- Direct use of `scud_core::{Storage, Phase, Task}`
- `next()`, `complete()`, `list_tasks()`, `waves()` functions
- Wave calculation using Kahn's algorithm

#### CLI Commands (874 lines in `main.rs`)
- `spawn` - Manual subagent spawning
- `transcripts` - List/show/replay transcripts
- `next`, `complete` - Task management
- `waves` - Show execution waves
- `swarm` - Swarm loop execution (delegates to `scud swarm`)
- `scud-spawn` - Spawn SCUD agents
- `interactive` - REPL session
- `agents`, `skills` - Management commands
- `init`, `wizard`, `config` - Setup commands

#### Default Content
- 12 agent definitions (builder, validator, planner, etc.)
- 2 prompts (plan.md, build.md)
- 1 skill (wizard.md)

---

### 4. Descartes GUI Implementation

**Location**: `descartes-gui/src/` (version: 0.1.0)

An Iced 0.14 desktop application with:

#### Architecture
```
┌─────────────────────────────────────────────────────────────┐
│                    DescartesGui (Iced)                       │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   Message Handling                     │  │
│  │  - Navigation (SwitchView)                            │  │
│  │  - SCUD Commands (LoadTasks, ComputeWaves, etc.)     │  │
│  │  - Swarm Control (Start, Pause, Resume, Stop)        │  │
│  │  - ScudEvents from bridge                             │  │
│  └──────────────────────────────────────────────────────┘  │
│         ↑ subscription()              ↓ Task::perform      │
│         │                             │                     │
│         │ ScudEvent                   │ ScudCommand         │
│         │                             │                     │
└─────────┼─────────────────────────────┼─────────────────────┘
          │                             │
          │    ┌───────────────────┐    │
          └────│   ScudBridge      │────┘
               │   (own thread)    │
               │   - tokio runtime │
               │   - scud-core API │
               │   - subprocess    │
               └─────────┬─────────┘
                         │
               ┌─────────┴─────────┐
               │ Direct Library    │ Subprocess for
               │ (task loading,    │ swarm execution
               │  wave compute,    │ (requires tmux)
               │  status updates)  │
               └───────────────────┘
```

#### Components

**ScudBridge** (`scud_bridge.rs`):
- Runs on separate tokio runtime
- Channel-based communication (100 message buffer)
- Direct scud-core library calls for: LoadTasks, ComputeWaves, CompleteTask, BlockTask
- Subprocess spawning for: StartSwarm (with JSON event streaming)
- Unix signal handling for pause/resume (SIGSTOP/SIGCONT)

**Views** (`views/`):
- **header.rs** - Navigation and status display
- **waves.rs** - Task wave display with Start/Done/Block buttons
- **agents.rs** - Swarm controls and status
- **output.rs** - Streaming output display

**State** (`state.rs`):
- `AgentStatus` enum (Idle, Running, Paused)
- `TaskInfo` display struct
- `SwarmDefaults` from config
- `AppState` with waves, tasks, output buffer

**What's Working**:
- Full SCUD task display and wave visualization
- Swarm execution with JSON event streaming
- Task status updates (Done/Block)
- Pause/Resume/Stop controls
- Configuration loading from `.descartes/config.toml`
- Comprehensive tests (lines 586-1279 in main.rs)

**What's Incomplete**:
- Single task execution (`Message::StartAgent`) - has TODO comment, only displays message
- Theme constants defined but not used in views

---

### 5. scud-eval Implementation

**Location**: `scud-eval/src/`

An evaluation framework for comparing execution modes:

#### Metrics (`metrics.rs`)
```rust
pub struct TaskMetrics {
    task_id, task_title, complexity,
    started_at, completed_at, duration_secs,
    success, first_pass_success, repair_attempts,
    lines_added, lines_removed, files_changed,
    tokens_input, tokens_output, estimated_cost_usd,
}

pub struct EvalRunMetrics {
    run_id, mode, taskset_name, harness, model,
    started_at, completed_at, total_duration_secs,
    total_tasks, tasks_succeeded, tasks_failed,
    first_pass_success_rate, total_repair_attempts,
    // ... aggregated git stats and tokens
    task_metrics: Vec<TaskMetrics>,
    validation_commands: Vec<ValidationMetrics>,
}

pub enum ExecutionMode {
    Swarm { round_size: usize },  // "swarm-N"
    Ralph,                         // "ralph"
    ClaudeDirect,                  // "claude-direct"
}
```

#### Task Sets (`tasksets.rs`)
Four built-in evaluation scenarios:

| Name | Type | Tasks | Complexity | Dependencies |
|------|------|-------|------------|--------------|
| eval-trivial | Synthetic | 5 | All 1 | None |
| eval-moderate | Synthetic | 5 | 3-5 | Yes |
| eval-complex | Synthetic | 8 | 5-13 | Deep chain |
| eval-real-scud | Real | 5 | 3-5 | Yes |

#### Runner (`runner.rs`)
- `setup_eval_workspace()` - Creates isolated git repo with SCG tasks
- `run_swarm()` - Fully implemented with JSON event streaming
- `run_ralph()` - Fully implemented
- `run_claude_direct()` - **NOT IMPLEMENTED** (placeholder only)

#### Collector (`collector.rs`)
- Session JSON parsing for swarm metrics
- Git stats collection (commits with `[task_id]` prefix)
- RFC3339 timestamp parsing

#### Token Estimation (`tokens.rs`)
- Regex patterns for Claude Code and OpenCode output parsing
- Cost estimation by model (Opus: $15/$75 per 1M tokens, Sonnet: $3/$15, etc.)

#### CLI (`main.rs`)
```bash
scud-eval run --mode swarm-4 --tasks eval-moderate
scud-eval list --tasksets
scud-eval list --runs
scud-eval compare <run1> <run2>
scud-eval report <run-id>
```

#### Storage
- Results in `~/.scud-eval/runs/<run-id>/metrics.json`
- Task sets in `~/.scud-eval/tasksets/<name>/`

#### Tests
27 integration tests covering:
- Taskset loading and validation
- Workspace setup
- Metrics serialization roundtrip
- Storage operations
- Taskset installation

---

## Code References

### scud-core
- `scud-core/src/lib.rs` - Public API exports
- `scud-core/src/models/task.rs` - Task struct (lines 75-118)
- `scud-core/src/models/phase.rs` - Phase struct (lines 33-40)
- `scud-core/src/formats/scg.rs` - SCG parser (lines 122-368)
- `scud-core/src/waves.rs` - Wave computation (lines 30-98)
- `scud-core/src/storage.rs` - Storage with locking (lines 21-382)

### descartes
- `descartes/src/lib.rs` - Library exports
- `descartes/src/main.rs` - CLI commands (874 lines)
- `descartes/src/config.rs` - Configuration system (744 lines)
- `descartes/src/spec.rs` - Spec building (2,224 lines)
- `descartes/src/harness/mod.rs` - Harness trait (lines 216-239)
- `descartes/src/scud/mod.rs` - SCUD integration (208 lines)

### descartes-gui
- `descartes-gui/src/main.rs` - Iced app (584+ lines)
- `descartes-gui/src/scud_bridge.rs` - Async bridge (658 lines)
- `descartes-gui/src/state.rs` - State management (69 lines)
- `descartes-gui/src/views/` - View modules

### scud-eval
- `scud-eval/src/lib.rs` - Module organization
- `scud-eval/src/metrics.rs` - Data structures
- `scud-eval/src/tasksets.rs` - Built-in task sets
- `scud-eval/src/runner.rs` - Execution orchestration
- `scud-eval/src/collector.rs` - Metrics collection

---

## Architecture Documentation

### Integration Pattern

The workspace follows a layered architecture:

1. **Foundation Layer** - scud-core provides shared types
2. **CLI Layer** - scud-cli and descartes provide command-line interfaces
3. **GUI Layer** - descartes-gui provides desktop visualization
4. **Evaluation Layer** - scud-eval provides benchmarking

### Key Design Decisions

1. **Dual Integration Strategy** (descartes-gui):
   - Library calls for read operations (task loading, wave compute)
   - Subprocess for complex orchestration (swarm with tmux)

2. **Fixed Spec Allocation** (descartes spec.rs):
   - ~5k token budget for context injection
   - Template-based with configurable placeholders

3. **Event-Driven GUI** (descartes-gui):
   - Channel-based communication prevents blocking
   - Subscription pattern for async event handling

4. **Atomic File Operations** (scud-core storage):
   - Exclusive locks for writes, shared for reads
   - RwLock caching for thread safety

---

## Related Research

- `thoughts/shared/plans/2026-01-22-scud-descartes-merger-sprites-integration.md` - Merger plan
- `thoughts/shared/research/2026-01-22-descartes-scud-gui-code-review.md` - GUI code review
- `thoughts/shared/plans/2026-01-22-scud-eval-crate.md` - Eval crate plan
- `thoughts/shared/research/2026-01-22-loom-scud-descartes-comparison.md` - Architecture comparison

---

## Open Questions

1. **Claude Direct Mode** - scud-eval has placeholder; should it generate a prompt for single-session completion?

2. **Single Task Execution** - descartes-gui has TODO for `StartAgent`; should this use RalphExecutor or spawn via SCUD?

3. **Iced Version Mismatch** - Workspace defines 0.12 but descartes-gui uses 0.14. Should workspace be updated?

4. **Theme Usage** - descartes-gui defines theme constants but uses inline colors. Should views be updated to use theme?

5. **OpenCode Harness Verification** - Protocol format is speculative per CLEANUP_PLAN.md. Has it been tested with real OpenCode?
