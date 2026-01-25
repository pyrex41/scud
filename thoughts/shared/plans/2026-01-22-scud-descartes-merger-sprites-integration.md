# Scud + Descartes Workspace Merger

## Overview

Merge Descartes into the Scud repository as workspace crates with tighter integration.

**What Descartes is**: A native Iced GUI that visualizes and controls Scud. Not an orchestration engine.

**What Scud is**: The engine - DAG task management, wave execution, backpressure validation, TUI monitor.

## Current State

### Scud (`scud-cli`)
- Rust CLI for DAG-based task management
- SCG format for token-efficient task storage
- Wave-based parallel execution via tmux
- Backpressure validation between waves
- TUI monitor (`scud monitor`)
- Harness support (claude, opencode)

### Descartes
- **Already refactored** - removed 2,689 lines of orchestration (v2)
- **Iced GUI** (`descartes-gui`) - native desktop app
- **CLI** (`descartes`) - thin wrappers over SCUD + spec building
- **Delegates to SCUD** via subprocess (`std::process::Command::new("scud")`)
- Has `ScudBridge` that spawns SCUD and streams events

### Problem
- Descartes is a separate repo with path dependency on scud-cli
- Integration is via subprocess (clunky)
- GUI "doesn't totally work right"

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Descartes (Iced GUI)                   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐│
│  │ Waves View  │ │ Agents View │ │ Output View         ││
│  │ (DAG viz)   │ │ (controls)  │ │ (streaming output)  ││
│  └──────┬──────┘ └──────┬──────┘ └──────────┬──────────┘│
│         └───────────────┴───────────────────┘           │
│                         │                                │
│                    scud-core                             │
│              (direct library calls)                      │
└─────────────────────────┬───────────────────────────────┘
                          │
┌─────────────────────────┴───────────────────────────────┐
│                      Scud Engine                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐│
│  │ Task DAG    │ │ Wave Exec   │ │ Backpressure        ││
│  │ (SCG format)│ │ (tmux)      │ │ (validation)        ││
│  └─────────────┘ └─────────────┘ └─────────────────────┘│
│  ┌─────────────┐ ┌─────────────┐                        │
│  │ TUI Monitor │ │ CLI         │                        │
│  │ (ratatui)   │ │ (clap)      │                        │
│  └─────────────┘ └─────────────┘                        │
└─────────────────────────────────────────────────────────┘
```

## Desired End State

```
scud/
├── Cargo.toml                  # Workspace root
├── scud-core/                  # Shared types + logic (NEW)
│   └── src/
│       ├── task.rs             # Task, Phase, TaskStatus
│       ├── scg.rs              # SCG format parsing
│       ├── waves.rs            # Wave computation (Kahn's)
│       └── storage.rs          # .scud/ file operations
│
├── scud-cli/                   # CLI binary
│   └── src/
│       ├── main.rs             # Entry point
│       ├── commands/           # swarm, monitor, etc.
│       └── tui/                # ratatui monitor
│
├── descartes/                  # Iced GUI (visualization + control)
│   └── src/
│       ├── main.rs             # Iced app
│       ├── views/              # Waves, Agents, Output
│       ├── bridge.rs           # Uses scud-core directly
│       ├── harness/            # Claude, OpenCode, Codex
│       └── spec.rs             # Spec building
│
└── descartes-cli/              # Thin CLI (optional, mostly deprecated)
    └── src/
        └── main.rs             # Delegates to scud or launches GUI
```

## What We're NOT Doing

- **Not adding remote execution** (Sprites/Fly.io - future work)
- **Not adding Elixir/Phoenix** (unnecessary complexity)
- **Not rewriting the GUI** (just fixing what's broken)
- **Not changing SCG format** (already works)

---

## Phase 1: Workspace Restructuring

### Overview
Convert Scud to a Cargo workspace and move Descartes in.

### Changes Required

#### 1. Create Workspace Root
**File**: `scud/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "scud-cli",
    "scud-core",
    "descartes",
    "descartes-cli",
]

[workspace.package]
version = "2.0.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
iced = "0.13"

# Internal
scud-core = { path = "scud-core" }
```

#### 2. Extract scud-core
Move shared types from scud-cli to scud-core:

**File**: `scud/scud-core/src/lib.rs`

```rust
pub mod task;
pub mod scg;
pub mod waves;
pub mod storage;

pub use task::{Task, TaskStatus, Phase, Priority};
pub use scg::{parse_scg, write_scg};
pub use waves::compute_waves;
pub use storage::Storage;
```

#### 3. Move Descartes
```bash
# Copy descartes repo contents
cp -r ../descartes/descartes scud/descartes
cp -r ../descartes/descartes-gui/* scud/descartes/  # Merge GUI into main crate

# Or keep separate:
cp -r ../descartes/descartes-gui scud/descartes-gui
```

#### 4. Update Dependencies

**File**: `scud/descartes/Cargo.toml`

```toml
[package]
name = "descartes"
version.workspace = true
edition.workspace = true

[dependencies]
scud-core.workspace = true
iced.workspace = true
tokio.workspace = true
# ... rest
```

### Success Criteria

**Automated:**
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean

**Manual:**
- [ ] `scud --help` works
- [ ] `descartes` launches GUI

---

## Phase 2: Direct Integration (Remove Subprocess Calls)

### Overview
Replace subprocess calls to `scud` CLI with direct library calls via `scud-core`.

### Current (subprocess)
```rust
// descartes/src/main.rs
std::process::Command::new("scud")
    .args(&["swarm", "--tag", tag])
    .status()?;
```

### Target (library)
```rust
// descartes/src/bridge.rs
use scud_core::{Storage, compute_waves, Task};

pub fn load_tasks(tag: &str) -> Result<Vec<Task>> {
    let storage = Storage::new(".scud")?;
    storage.load_tasks_by_tag(tag)
}

pub fn get_waves(tasks: &[Task]) -> Vec<Vec<String>> {
    compute_waves(tasks)
}
```

### Changes Required

#### 1. Expose scud-core API

**File**: `scud/scud-core/src/storage.rs`

```rust
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self>;
    pub fn load_tasks_by_tag(&self, tag: &str) -> Result<Vec<Task>>;
    pub fn save_task(&self, task: &Task) -> Result<()>;
    pub fn mark_complete(&self, task_id: &str) -> Result<()>;
}
```

#### 2. Update Descartes Bridge

**File**: `scud/descartes/src/bridge.rs`

```rust
use scud_core::{Storage, compute_waves, Task, TaskStatus};
use std::sync::mpsc;

pub struct ScudBridge {
    storage: Storage,
    event_tx: mpsc::Sender<ScudEvent>,
}

impl ScudBridge {
    pub fn new(event_tx: mpsc::Sender<ScudEvent>) -> Result<Self> {
        Ok(Self {
            storage: Storage::new(".scud")?,
            event_tx,
        })
    }

    pub fn load_tasks(&self, tag: &str) -> Result<Vec<Task>> {
        let tasks = self.storage.load_tasks_by_tag(tag)?;
        self.event_tx.send(ScudEvent::TasksLoaded(tasks.clone()))?;
        Ok(tasks)
    }

    pub fn compute_waves(&self, tasks: &[Task]) -> Vec<Vec<String>> {
        let waves = scud_core::compute_waves(tasks);
        self.event_tx.send(ScudEvent::WavesComputed(waves.clone()))?;
        waves
    }

    // For swarm execution, still delegate to CLI (it manages tmux)
    pub fn start_swarm(&self, tag: &str, harness: &str) -> Result<()> {
        // This one stays as subprocess - swarm needs terminal management
        std::process::Command::new("scud")
            .args(&["swarm", "--tag", tag, "--harness", harness])
            .spawn()?;
        Ok(())
    }
}
```

### Success Criteria

**Automated:**
- [ ] `cargo test -p descartes` passes
- [ ] No subprocess calls for read operations

**Manual:**
- [ ] GUI loads tasks without spawning `scud` process
- [ ] Wave computation works via library
- [ ] Swarm still works (uses subprocess for terminal mgmt)

---

## Phase 3: Fix Iced GUI Issues

### Overview
Address specific issues identified in the code review. The GUI architecture is sound (message-driven, channel-based ScudBridge, Iced subscriptions) but has implementation gaps.

**Reference**: `thoughts/shared/research/2026-01-22-descartes-scud-gui-code-review.md`

---

### Phase 3A: Fix WavesComputed Type Mismatch

**Problem**: `scud waves --json` returns `Vec<Vec<String>>` (task IDs), but GUI needs `Vec<Vec<TaskInfo>>`.

**Current workaround** (`descartes-gui/src/main.rs:368-386`):
```rust
// This only works if TasksLoaded already ran - fragile implicit dependency
let waves: Vec<Vec<TaskInfo>> = wave_ids
    .into_iter()
    .map(|wave| wave.into_iter()
        .filter_map(|id| self.state.tasks.iter().find(|t| t.id == id).cloned())
        .collect())
    .collect();
```

**Fix**: Change `ScudEvent::WavesComputed` to return full task data.

**File**: `descartes-gui/src/scud_bridge.rs:324-361`

```rust
// Option A: ScudBridge computes waves with full TaskInfo
pub enum ScudEvent {
    // Change from Vec<Vec<String>> to Vec<Vec<TaskInfo>>
    WavesComputed(Vec<Vec<TaskInfo>>),
    // ...
}

async fn compute_waves(&self, tag: &str) {
    // First load tasks
    let tasks = self.load_tasks_internal(tag.into()).await?;
    // Then compute waves and map IDs back to TaskInfo
    let wave_ids = self.get_wave_ids(tag).await?;
    let waves: Vec<Vec<TaskInfo>> = wave_ids
        .into_iter()
        .map(|wave| wave.into_iter()
            .filter_map(|id| tasks.iter().find(|t| t.id == id).cloned())
            .collect())
        .collect();
    let _ = self.event_tx.send(ScudEvent::WavesComputed(waves)).await;
}
```

**Success Criteria**:
- [ ] `WavesComputed` event contains `Vec<Vec<TaskInfo>>`
- [ ] `main.rs:368-386` simplified to direct assignment
- [ ] Waves display correctly even on fresh app launch

---

### Phase 3B: Extract View Modules

**Problem**: All view logic is in `main.rs` (1,469 lines). View modules are empty placeholders.

**Current state**:
- `views/waves.rs:5-6` - "This module is reserved for future extraction"
- `views/agents.rs:5-6` - "This module is reserved for future extraction"
- `views/output.rs:5-6` - "This module is reserved for future extraction"

**Files to refactor**:

| Source Location | Target File | Lines |
|-----------------|-------------|-------|
| `main.rs:552-616` | `views/waves.rs` | view_waves() |
| `main.rs:618-673` | `views/agents.rs` | view_agents() |
| `main.rs:675-711` | `views/output.rs` | view_output() |
| `main.rs:518-550` | `views/header.rs` | view_header() |

**Pattern**: Each view module exports a function:
```rust
// views/waves.rs
use crate::state::{AppState, TaskInfo};
use crate::Message;
use iced::Element;

pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
    // Move view_waves() logic here
}
```

**Success Criteria**:
- [ ] `main.rs` < 500 lines
- [ ] Each view in its own module
- [ ] `cargo build -p descartes-gui` succeeds
- [ ] Views render identically to before

---

### Phase 3C: Apply Theme Constants

**Problem**: `theme.rs` defines color constants that aren't used anywhere.

**File**: `descartes-gui/src/theme.rs:10-37`
```rust
// Currently unused:
pub const ACCENT: Color = Color::from_rgb(0.3, 0.5, 0.9);
pub const SUCCESS: Color = Color::from_rgb(0.3, 0.8, 0.4);
pub const WARNING: Color = Color::from_rgb(0.9, 0.7, 0.2);
pub const ERROR: Color = Color::from_rgb(0.9, 0.3, 0.3);
// ... background and text colors
```

**Apply to**:
- Error banner background → `theme::ERROR`
- Success status → `theme::SUCCESS`
- Output container → `theme::background::SECONDARY`
- Task status text → Appropriate colors
- Navigation buttons → `theme::ACCENT` for active

**Success Criteria**:
- [ ] Remove `#[allow(dead_code)]` from `theme.rs:5`
- [ ] All color literals in views replaced with theme constants
- [ ] `cargo clippy` shows no unused constant warnings

---

### Phase 3D: Wire Swarm Progress Display

**Problem**: Swarm events are parsed but not properly displayed.

**Events defined** (`scud_bridge.rs:27-51`):
- `SwarmStarted { tag, total_waves }` - Logged but no visual indicator
- `WaveStarted { wave, tasks }` - Logged to output only
- `TaskStarted { task_id }` - Sets current_task but no highlight
- `TaskOutput { task_id, text }` - Appended to output
- `TaskCompleted { task_id, success }` - Logged only
- `WaveCompleted { wave }` - Logged only
- `SwarmCompleted { success }` - Sets Idle status

**Required changes**:

1. **Add swarm progress to AppState** (`state.rs`):
```rust
pub struct SwarmProgress {
    pub total_waves: usize,
    pub current_wave: usize,
    pub wave_tasks: Vec<String>,
    pub completed_tasks: HashSet<String>,
}

pub struct AppState {
    // ... existing fields ...
    pub swarm_progress: Option<SwarmProgress>,
}
```

2. **Update event handlers** (`main.rs:387-446`):
```rust
ScudEvent::SwarmStarted { tag, total_waves } => {
    self.state.swarm_progress = Some(SwarmProgress {
        total_waves,
        current_wave: 0,
        wave_tasks: vec![],
        completed_tasks: HashSet::new(),
    });
    // ... existing logging
}
ScudEvent::WaveStarted { wave, tasks } => {
    if let Some(ref mut progress) = self.state.swarm_progress {
        progress.current_wave = wave;
        progress.wave_tasks = tasks.clone();
    }
    // ... existing logging
}
ScudEvent::TaskCompleted { task_id, success } => {
    if let Some(ref mut progress) = self.state.swarm_progress {
        progress.completed_tasks.insert(task_id.clone());
    }
    // ... existing logging
}
```

3. **Show progress in Agents view** (`views/agents.rs`):
```rust
// Add progress bar or wave indicator
if let Some(progress) = &state.swarm_progress {
    text(format!("Wave {}/{}: {}/{} tasks",
        progress.current_wave,
        progress.total_waves,
        progress.completed_tasks.len(),
        progress.wave_tasks.len()
    ))
}
```

**Success Criteria**:
- [ ] Swarm progress visible in Agents view during execution
- [ ] Wave number updates as swarm progresses
- [ ] Task completion count updates in real-time

---

### Phase 3E: Connect Control Channel

**Problem**: `control_tx` for `AgentCommand` (Pause/Resume/Cancel) is always `None`.

**Current state** (`main.rs:52`):
```rust
control_tx: Option<mpsc::Sender<AgentCommand>>,  // Always None
```

**Event handlers that check it** (`main.rs:311-351`):
```rust
Message::PauseAgent => {
    if let Some(ref tx) = self.control_tx {
        let _ = tx.send(AgentCommand::Pause);  // Never executes
    }
    self.state.agent_status = AgentStatus::Paused;  // UI-only update
    Task::none()
}
```

**Fix**: Connect control channel to ScudBridge for swarm control.

1. **Add control commands to ScudBridge** (`scud_bridge.rs`):
```rust
pub enum ScudCommand {
    // ... existing commands ...
    PauseSwarm,
    ResumeSwarm,
    // StopSwarm already exists
}
```

2. **Update GUI initialization** (`main.rs:123-148`):
```rust
fn new() -> (Self, Task<Message>) {
    let (bridge, scud_command_tx, scud_event_rx) = ScudBridge::create();
    // Remove control_tx field, use scud_command_tx for all control
    // ...
}
```

3. **Update control handlers**:
```rust
Message::PauseAgent => {
    if let Some(ref tx) = self.scud_command_tx {
        let tx = tx.clone();
        return Task::perform(
            async move { let _ = tx.send(ScudCommand::PauseSwarm).await; },
            |_| Message::Tick,
        );
    }
    Task::none()
}
```

**Success Criteria**:
- [ ] Remove `control_tx` field
- [ ] Pause button pauses swarm execution
- [ ] Resume button resumes swarm execution
- [ ] Cancel button stops swarm (uses existing StopSwarm)

---

### Phase 3F: Make Defaults Configurable

**Problem**: Hardcoded defaults in `main.rs:630-635`:
```rust
let tag = self.state.active_tag.clone().unwrap_or_else(|| "refactor".to_string());
let harness = "claude-code".to_string();
let round_size = 3;
```

**Fix**: Add configuration to AppState or load from `.descartes/config.toml`.

1. **Add defaults to state** (`state.rs`):
```rust
pub struct SwarmDefaults {
    pub harness: String,
    pub round_size: usize,
    pub default_tag: String,
}

impl Default for SwarmDefaults {
    fn default() -> Self {
        Self {
            harness: "claude-code".to_string(),
            round_size: 3,
            default_tag: "refactor".to_string(),
        }
    }
}
```

2. **Load from config on startup** (`main.rs:new()`):
```rust
let defaults = descartes::Config::load(None)
    .map(|c| SwarmDefaults {
        harness: c.harness.kind.clone(),
        round_size: 3,  // Could add to config
        default_tag: "refactor".to_string(),
    })
    .unwrap_or_default();
```

3. **Add UI controls** (optional, lower priority):
- Dropdown for harness selection
- Input for round size
- Tag selector from available tags

**Success Criteria**:
- [ ] Defaults loaded from config file if available
- [ ] Fallback to sensible defaults if no config
- [ ] (Optional) UI controls for changing defaults

---

### Phase 3 Overall Success Criteria

**Automated:**
- [ ] `cargo build -p descartes-gui` succeeds
- [ ] `cargo test -p descartes-gui` passes (existing tests)
- [ ] `cargo clippy -p descartes-gui -- -D warnings` clean
- [ ] No unused code warnings in `theme.rs`

**Manual:**
- [ ] Open GUI → tasks load automatically
- [ ] Click Refresh → tasks update
- [ ] Start Swarm → see wave progress indicator
- [ ] Pause Swarm → execution pauses
- [ ] Resume Swarm → execution continues
- [ ] Stop Swarm → execution stops
- [ ] Error banner uses theme colors
- [ ] Output view uses theme background

---

## Testing Strategy

### Unit Tests (Rust)
```bash
# All workspace tests
cargo test --workspace

# Specific crate
cargo test -p scud-core
cargo test -p descartes
```

### Integration Tests
- Workspace builds together
- scud-core used by both scud-cli and descartes
- GUI can load and display tasks

### Manual Testing
1. `scud swarm --tag test` - verify CLI still works
2. `descartes` - verify GUI launches
3. Load tasks in GUI, verify display
4. Start swarm from GUI, verify progress updates

---

## Migration Notes

### For Descartes Users
- Same binary name (`descartes`)
- Same GUI functionality
- Faster startup (no subprocess for reads)

### For Scud Users
- No changes to CLI
- Can optionally use `descartes` for GUI

---

## References

- Scud repo: current location
- Descartes repo: `/Users/reuben/projects/descartes`
- Iced docs: https://docs.rs/iced
- Comparison research: `thoughts/shared/research/2026-01-22-loom-scud-descartes-comparison.md`
- **GUI code review**: `thoughts/shared/research/2026-01-22-descartes-scud-gui-code-review.md`
