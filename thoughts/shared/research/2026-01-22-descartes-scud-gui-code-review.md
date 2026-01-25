---
date: 2026-01-22T22:33:04Z
researcher: Claude
git_commit: 6807cad3db74503b23fc7e289ccc17c5a5919909
branch: trunk
repository: loom
topic: "Descartes + Scud GUI Code Review for Phase 3"
tags: [research, descartes, scud, iced, gui, code-review]
status: complete
last_updated: 2026-01-22
last_updated_by: Claude
---

# Research: Descartes + Scud GUI Code Review

**Date**: 2026-01-22T22:33:04Z
**Researcher**: Claude
**Git Commit**: 6807cad3db74503b23fc7e289ccc17c5a5919909
**Branch**: trunk
**Repository**: loom

## Research Question

Full code review of Descartes and Scud to understand what needs to be done for Phase 3 of the workspace merger plan. Focus on:
1. Current GUI state and issues
2. ScudBridge integration
3. Event flow and state management
4. What's broken vs what works

## Summary

The Descartes GUI is a **functional but incomplete** Iced 0.14 application that visualizes Scud tasks. The core architecture is sound - it uses proper message-driven patterns, channel-based communication with ScudBridge, and Iced subscriptions for event streaming. However, there are **structural issues** that prevent full integration:

### What Works
- Basic GUI rendering (three views: Waves, Agents, Output)
- ScudBridge subprocess spawning and event parsing
- Channel-based communication pattern
- Task loading via `scud list --json`
- Status updates via `scud set-status`
- Dark theme styling

### What's Broken or Incomplete
1. **Wave computation mismatch** - GUI receives `Vec<Vec<String>>` (task IDs) but needs `Vec<Vec<TaskInfo>>` (full task data)
2. **View modules are placeholders** - All view logic is inlined in `main.rs`, modules are empty
3. **No swarm event handling** - Events are parsed but UI doesn't show swarm progress properly
4. **Theme constants unused** - `theme.rs` defines colors that aren't used
5. **Control channel disconnected** - `control_tx` for AgentCommand is always None

---

## Detailed Findings

### 1. Descartes GUI Architecture

#### File Structure (`/Users/reuben/projects/descartes/descartes-gui/src/`)

| File | Purpose | Status |
|------|---------|--------|
| `main.rs` | Main Iced app, all view logic | **Bloated** - 1,469 lines |
| `state.rs` | AppState, TaskInfo, AgentStatus | Working |
| `theme.rs` | Color constants | **Unused** |
| `scud_bridge.rs` | ScudBridge, ScudEvent, ScudCommand | Working |
| `views/mod.rs` | Module declarations | Empty exports |
| `views/waves.rs` | Placeholder | **Empty** |
| `views/agents.rs` | Placeholder | **Empty** |
| `views/output.rs` | Placeholder | **Empty** |

#### Iced Version and Patterns

- **Version**: Iced 0.14 with `tokio` and `advanced` features
- **Pattern**: Function-based `application()` constructor (not struct-based)
- **State**: `DescartesGui` struct with all app state
- **Messages**: `Message` enum with 25+ variants

### 2. ScudBridge Integration

#### Current Implementation

**Location**: `/Users/reuben/projects/descartes/descartes-gui/src/scud_bridge.rs`

The ScudBridge runs on a **separate thread** with its own tokio runtime:

```rust
// main.rs:131-134
std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(bridge.run());
});
```

**Communication Channels**:
- `scud_command_tx: mpsc::Sender<ScudCommand>` - GUI → Bridge
- `scud_event_rx: mpsc::Receiver<ScudEvent>` - Bridge → GUI (via subscription)
- Buffer size: 100 messages

**ScudCommand Variants** (`scud_bridge.rs:63-87`):
- `LoadTasks { tag }` → `scud list --json [--tag <tag>]`
- `ComputeWaves { tag }` → `scud waves --json --tag <tag>`
- `StartSwarm { tag, harness, round_size }` → `scud swarm --json-events`
- `StopSwarm` → kills subprocess
- `CompleteTask { task_id }` → `scud set-status <id> done`
- `BlockTask { task_id }` → `scud set-status <id> blocked`

**ScudEvent Variants** (`scud_bridge.rs:18-58`):
- `TasksLoaded(Vec<TaskInfo>)`
- `WavesComputed(Vec<Vec<String>>)` - **Note: Returns IDs, not TaskInfo**
- `SwarmStarted { tag, total_waves }`
- `WaveStarted { wave, tasks }`
- `TaskStarted { task_id }`
- `TaskOutput { task_id, text }`
- `TaskCompleted { task_id, success }`
- `ValidationStarted`
- `ValidationCompleted { passed, output }`
- `WaveCompleted { wave }`
- `SwarmCompleted { success }`
- `Output(String)`
- `Error(String)`

#### Issue: Wave Computation Mismatch

**Problem**: `scud waves --json` returns `Vec<Vec<String>>` (task IDs), but the GUI needs `Vec<Vec<TaskInfo>>` (full task objects).

**Current Workaround** (`main.rs:368-386`):
```rust
Message::ScudEvent(ScudEvent::WavesComputed(wave_ids)) => {
    // Try to convert wave IDs to TaskInfo by matching against loaded tasks
    let waves: Vec<Vec<TaskInfo>> = wave_ids
        .into_iter()
        .map(|wave| {
            wave.into_iter()
                .filter_map(|id| {
                    self.state.tasks.iter().find(|t| t.id == id).cloned()
                })
                .collect()
        })
        .collect();
    self.state.waves = waves;
    Task::none()
}
```

This works **only if** `TasksLoaded` has already populated `self.state.tasks`. The dependency is implicit and fragile.

### 3. Event Flow and State Management

#### Message Flow Diagram

```
User Action (button click)
    ↓
generates Message
    ↓
update() processes Message
    ↓ (if async needed)
returns Task::perform(async fn, result_mapper)
    ↓
async fn sends ScudCommand via channel
    ↓
ScudBridge.run() receives command
    ↓
ScudBridge spawns `scud` subprocess
    ↓
ScudBridge parses JSON output
    ↓
ScudBridge sends ScudEvent via channel
    ↓
subscription() yields Message::ScudEvent
    ↓
update() processes ScudEvent
    ↓
State updated, view() re-renders
```

#### Subscription Implementation

**Location**: `main.rs:713-732`

```rust
fn subscription(&self) -> Subscription<Message> {
    let rx = self.scud_event_rx.clone();
    Subscription::run_with(ScudEventReceiver(rx), |ScudEventReceiver(rx)| {
        let rx = rx.clone();
        async_stream::stream! {
            let mut receiver = {
                let mut guard = rx.lock().await;
                guard.take()  // Takes ownership once
            };
            if let Some(ref mut rx) = receiver {
                while let Some(event) = rx.recv().await {
                    yield Message::ScudEvent(event);
                }
            }
        }
    })
}
```

**Pattern**: The receiver is wrapped in `Arc<TokioMutex<Option<...>>>` so the subscription can take ownership exactly once.

#### State Structure

**Location**: `state.rs:21-35`

```rust
pub struct AppState {
    pub waves: Vec<Vec<TaskInfo>>,      // Tasks organized by wave
    pub tasks: Vec<TaskInfo>,            // Flat list for reference
    pub active_tag: Option<String>,      // Current tag filter
    pub agent_status: AgentStatus,       // Idle, Running, Paused
    pub current_task: Option<String>,    // Currently executing task ID
    pub output_buffer: String,           // Accumulated output
}
```

### 4. View Implementation Details

#### Waves View (`main.rs:552-616`)

- Shows tasks grouped by wave
- Each task row has: ID (80px), title (fill), status (100px), action buttons
- Action buttons: Start, Done, Block
- Refresh button at bottom
- Shows active tag filter label

**Issue**: Wave view only renders if `self.state.waves` is populated. If waves haven't been computed, shows "No tasks loaded."

#### Agents View (`main.rs:618-673`)

- Shows agent status (Idle, Running, Paused)
- Control buttons change based on status:
  - Idle: "Start Swarm" button
  - Running: Pause, Stop, Cancel buttons
  - Paused: Resume, Cancel buttons
- Shows current task ID
- Shows active tag

**Issue**: Start Swarm uses hardcoded defaults:
- harness: "claude-code" (`main.rs:634`)
- round_size: 3 (`main.rs:635`)
- tag: active_tag or "refactor" (`main.rs:630`)

#### Output View (`main.rs:675-711`)

- Shows header with task ID and status
- Clear button
- Output text in dark-styled container
- Scrollable

**Working correctly** - just displays `self.state.output_buffer`

### 5. Scud CLI Analysis

#### Task Model (`scud-cli/src/models/task.rs`)

```rust
pub struct Task {
    pub id: String,              // Namespaced: "epic:local_id"
    pub title: String,           // Max 200 chars
    pub description: String,     // Max 5000 chars
    pub status: TaskStatus,      // 9 states
    pub complexity: u32,         // Fibonacci: 0,1,2,3,5,8,13,21,34,55,89
    pub priority: Priority,      // Critical, High, Medium, Low
    pub dependencies: Vec<String>,
    pub parent_id: Option<String>,
    pub subtasks: Vec<String>,
    pub details: Option<String>,
    pub test_strategy: Option<String>,
    pub assigned_to: Option<String>,
    pub agent_type: Option<String>,
}
```

**TaskStatus**: Pending, InProgress, Done, Review, Blocked, Deferred, Cancelled, Expanded, Failed

#### Wave Computation (`scud-cli/src/commands/waves.rs`)

Uses Kahn's algorithm:
1. Build in-degree map (count of unmet dependencies per task)
2. Build dependents map (for each task, list of tasks depending on it)
3. Extract wave = all tasks with in-degree 0
4. Decrement in-degree for dependents
5. Repeat until no tasks remain

**Complexity**: O(V + E) typical, O(V² + VE) worst case

#### SCG Format (`scud-cli/src/formats/scg.rs`)

Token-efficient pipe-delimited format:

```
# SCUD Graph v1
# Phase: refactor

@meta
name refactor
id_format sequential

@nodes
# id | title | status | complexity | priority
1 | Implement auth | P | 8 | H
2 | Add tests | P | 3 | M

@edges
2 -> 1

@details
1:description """
Multi-line description here
"""
```

**Status codes**: P=Pending, I=InProgress, D=Done, R=Review, B=Blocked, F=Deferred, C=Cancelled, X=Expanded, !=Failed

### 6. Descartes CLI Library

#### Spec Building System (`descartes/src/spec.rs`)

The spec system builds prompts for AI agents with:
- **CodebaseContext**: Glob patterns + keywords → file snippets
- **DependencyContext**: Summaries from completed tasks
- **VerificationConfig**: Primary + additional validation commands
- **SpecTemplate**: Placeholders: `{task}`, `{plan}`, `{codebase}`, `{dependencies}`, `{verification}`, `{custom}`

**build_task_spec()** combines all context into ~5k tokens for fixed-allocation prompts.

#### Agent Categories (`descartes/src/agent/category.rs`)

| Category | Harness | Model | Parallel | Backpressure |
|----------|---------|-------|----------|--------------|
| Searcher | OpenCode | grok-code-fast-1 | Yes | No |
| Analyzer | OpenCode | grok-code-fast-1 | Yes | No |
| FastBuilder | OpenCode | grok-code-fast-1 | No | No |
| Builder | Claude Code | opus | No | No |
| Planner | Claude Code | opus | No | No |
| BuilderReviewer | Claude Code | opus | No | No |
| Validator | (any) | (any) | No | **Yes** |

#### Transcript Format (`descartes/src/transcript/scg.rs`)

SCG format for transcripts:
```
@transcript
id: "uuid"
harness: "claude-code"
model: "opus"
started: 2026-01-22T12:00:00Z

@messages
1:user "Implement feature X"
2:assistant "I'll start by..."
3:tool:bash "cargo test"
4:result:ok "All tests passed"

@metrics
tokens_in: 1500
tokens_out: 800
duration_ms: 45000
tools_called: 5
```

---

## Architecture Documentation

### Current Integration Pattern

```
┌─────────────────────────────────────────────────────────────┐
│                    Descartes GUI (Iced)                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   DescartesGui                        │  │
│  │  - view: ViewMode                                     │  │
│  │  - state: AppState (waves, tasks, output_buffer)     │  │
│  │  - scud_command_tx: Sender<ScudCommand>              │  │
│  │  - scud_event_rx: Arc<Mutex<Receiver<ScudEvent>>>    │  │
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
               │   - event_tx      │
               │   - command_rx    │
               └─────────┬─────────┘
                         │ subprocess
                         ↓
               ┌───────────────────┐
               │    scud CLI       │
               │   - list          │
               │   - waves         │
               │   - swarm         │
               │   - set-status    │
               └───────────────────┘
```

### Desired Integration Pattern (Post-Merger)

```
┌─────────────────────────────────────────────────────────────┐
│                    Descartes GUI (Iced)                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   DescartesGui                        │  │
│  │  - state: AppState                                    │  │
│  │  - bridge: ScudBridge (library calls)                │  │
│  └──────────────────────────────────────────────────────┘  │
│                          │                                  │
│                          ↓ direct fn calls                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   scud-core                           │  │
│  │  - Storage::load_tasks()                             │  │
│  │  - compute_waves()                                    │  │
│  │  - Task, Phase types                                  │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
          │ subprocess (only for swarm - needs tmux)
          ↓
┌───────────────────┐
│    scud CLI       │
│   - swarm         │
└───────────────────┘
```

---

## Code References

### Descartes GUI
- `/Users/reuben/projects/descartes/descartes-gui/src/main.rs:47-60` - DescartesGui struct
- `/Users/reuben/projects/descartes/descartes-gui/src/main.rs:72-112` - Message enum
- `/Users/reuben/projects/descartes/descartes-gui/src/main.rs:123-148` - new() initialization
- `/Users/reuben/projects/descartes/descartes-gui/src/main.rs:150-469` - update() handler
- `/Users/reuben/projects/descartes/descartes-gui/src/main.rs:713-732` - subscription()
- `/Users/reuben/projects/descartes/descartes-gui/src/scud_bridge.rs:177-186` - ScudBridge struct
- `/Users/reuben/projects/descartes/descartes-gui/src/scud_bridge.rs:217-250` - run() loop
- `/Users/reuben/projects/descartes/descartes-gui/src/state.rs:21-35` - AppState struct

### Descartes CLI
- `/Users/reuben/projects/descartes/descartes/src/scud/mod.rs:16-32` - next()
- `/Users/reuben/projects/descartes/descartes/src/scud/mod.rs:90-155` - calculate_waves()
- `/Users/reuben/projects/descartes/descartes/src/spec.rs:889-995` - build_task_spec()
- `/Users/reuben/projects/descartes/descartes/src/agent/subagent.rs:105-287` - spawn_subagent_with_options()

### Scud CLI (via scud.xml context)
- `scud-cli/src/models/task.rs` - Task struct and validation
- `scud-cli/src/models/phase.rs` - Phase container
- `scud-cli/src/formats/scg.rs` - SCG parser/serializer
- `scud-cli/src/commands/waves.rs` - Wave computation (Kahn's algorithm)
- `scud-cli/src/storage/mod.rs` - Storage layer

---

## Phase 3 Work Items

Based on this code review, Phase 3 should address:

### Must Fix
1. **WavesComputed handling** - The conversion from `Vec<Vec<String>>` to `Vec<Vec<TaskInfo>>` needs to be robust (ensure tasks are loaded first, or load them inline)
2. **View extraction** - Move view functions from `main.rs` to `views/*.rs` modules
3. **Theme application** - Use the defined theme constants in `theme.rs`

### Should Fix
4. **Swarm event display** - Show swarm progress (wave number, task status) in output view
5. **Control channel wiring** - Connect `control_tx` for pause/resume/cancel during swarm
6. **Error handling** - Show errors from ScudBridge more prominently

### Nice to Have
7. **Configurable defaults** - Allow setting harness/round_size from config or UI
8. **Task detail view** - Click task to see full description/dependencies
9. **Live wave progress** - Show which tasks in current wave are running/complete

---

## Related Research

- `thoughts/shared/plans/2026-01-22-scud-descartes-merger-sprites-integration.md` - The merger plan this supports
- `thoughts/shared/research/2026-01-22-loom-scud-descartes-comparison.md` - Architectural comparison

---

## Open Questions

1. **Swarm subprocess vs library** - Should swarm execution remain as subprocess (needs tmux) or be partially integrated?
2. **Iced 0.14 vs 0.13** - The plan mentions Iced 0.13, but descartes-gui uses 0.14. Which version should the merged workspace target?
3. **Transcript location** - After merger, where should transcripts be stored? `.scud/transcripts/` or `.descartes/transcripts/`?
