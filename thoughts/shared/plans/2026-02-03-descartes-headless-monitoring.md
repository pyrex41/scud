# Descartes GUI: Headless Swarm Monitoring Implementation Plan

## Overview

Add a dedicated "Monitor" view to the Descartes (Iced) GUI that displays real-time status and streaming output from multiple headless swarm sessions running in parallel. This replaces the single-task output model with a multi-session dashboard similar to the ratatui TUI monitor but implemented in Iced.

## Current State Analysis

### What Already Exists

The Descartes GUI already has significant headless infrastructure:

1. **ScudBridge** (`descartes-gui/src/scud_bridge.rs`) already:
   - Has `RunTaskHeadless` command (line 135)
   - Creates `StreamStore` sessions and streams events (lines 866-1100)
   - Converts `StreamEventKind` to `ScudEvent` variants: `HeadlessStarted`, `ToolStart`, `ToolResult`, `SessionAssigned` (lines 72-92)
   - Has `stream_store()`, `get_headless_output()`, `is_headless_active()` helper methods (lines 1102-1118)

2. **StreamStore** (`scud-cli/src/commands/spawn/headless/store.rs`):
   - Thread-safe multi-session storage with `Arc<RwLock<HashMap>>`
   - Per-session event storage, output line rendering, status tracking
   - Methods: `all_tasks()`, `active_tasks()`, `get_status()`, `get_output()`, `get_all_output()`, `session_stats()`
   - Memory-bounded (10k lines, 50k events per session)

3. **GUI State** (`descartes-gui/src/state.rs`):
   - `AppState` has a single `output_buffer: String` and `current_task: Option<String>` — only tracks one task at a time
   - No per-task output storage in the GUI state

4. **Streaming View Component** (`descartes-gui/src/components/streaming_view.rs`):
   - `OutputLine` type with `OutputLineType` for colored output (Normal, Error, Success, System)
   - `StreamingViewState` with auto-scroll and fullscreen toggles
   - `OutputBuffer` for collecting lines with max-line limits
   - `view()` and `view_strings()` render functions ready to use

5. **Existing Views**: Waves, Agents, Output — three nav tabs in header

### What's Missing

- **No multi-session monitoring view**: Current GUI tracks a single agent with one `output_buffer` string
- **No per-task output buffers**: Events from all headless tasks dump into the same `output_buffer`
- **No task selector for switching feeds**: Can't pick which task's output to watch
- **No session status dashboard**: No visual summary of Starting/Running/Completed/Failed across tasks
- **Swarm doesn't use headless mode**: `StartSwarm` command shells out to `scud swarm --json-events` (subprocess approach), not the headless runner infrastructure

### Key Discoveries

- The `ScudBridge` already has `StreamStore` but the GUI doesn't access it at all — events get converted to flat `ScudEvent` variants and the per-task structured data is lost
- The ratatui TUI (`scud-cli/src/commands/spawn/tui/app.rs`) has a clean multi-panel pattern: waves panel + agents panel + output panel with tab-switching between panels
- The existing `streaming_view` component is well-designed and can be reused for per-task output display
- The `StartSwarm` → `run_swarm()` path in ScudBridge doesn't create `StreamStore` sessions — it's a completely separate path from `RunTaskHeadless`

## Desired End State

After implementation:

1. A new "Monitor" tab in the Descartes header navigation bar
2. The Monitor view shows a split layout:
   - **Left panel**: Task list with per-task status indicators (Starting/Running/Completed/Failed) and event/line counts
   - **Right panel**: Streaming output from the selected task, using the `streaming_view` component
3. When a swarm runs in headless mode, each task creates a `StreamStore` session and the Monitor view updates in real-time
4. Clicking a task in the left panel switches the right panel's output feed
5. Status badges use color coding: Starting=yellow, Running=blue, Completed=green, Failed=red

### How to Verify

- Start Descartes GUI and navigate to the Monitor tab — should show "No headless sessions" placeholder
- Start a headless swarm (via Agents view or CLI) — Monitor tab should populate with per-task entries
- Click between tasks in the left panel — right panel output should switch
- Tasks should transition through Starting → Running → Completed/Failed with visual indicators
- `cargo test -p descartes-gui` passes all tests including new Monitor view tests

## What We're NOT Doing

- **Not replacing the existing Output view** — it remains for non-headless output
- **Not converting StartSwarm to use headless runners** — that's a separate future task; for now, the Monitor view works with `RunTaskHeadless` and future swarm-headless integration
- **Not adding log persistence/replay** — the Monitor view shows live sessions only
- **Not adding task filtering/search** — simple list is sufficient for now
- **Not adding terminal emulation** — raw text output with color coding is sufficient

## Implementation Approach

Expose the `StreamStore` data to the GUI through new `ScudEvent` variants for session lifecycle, and maintain per-task state in `AppState`. Add a new `Monitor` view that renders a split-panel dashboard.

## Phase 1: State Model Extensions

### Overview
Extend `AppState` and `ScudEvent`/`Message` to support multiple headless sessions with per-task output.

### Changes Required

#### 1. Extend AppState
**File**: `descartes-gui/src/state.rs`
**Changes**: Add headless session tracking structures

```rust
use std::collections::HashMap;

/// Status of a headless session (mirrors StreamStore's SessionStatus)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadlessSessionStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Per-task headless session info for the GUI
#[derive(Debug, Clone)]
pub struct HeadlessSessionInfo {
    pub task_id: String,
    pub task_title: String,
    pub harness: String,
    pub status: HeadlessSessionStatus,
    pub event_count: usize,
    pub line_count: usize,
    pub output_lines: Vec<String>,
}

// Add to AppState:
pub struct AppState {
    // ... existing fields ...
    /// Headless sessions for monitoring (task_id → session info)
    pub headless_sessions: HashMap<String, HeadlessSessionInfo>,
    /// Currently selected task in the monitor view
    pub monitor_selected_task: Option<String>,
}
```

#### 2. Add Message variants
**File**: `descartes-gui/src/main.rs`
**Changes**: Add messages for monitor interaction

```rust
pub enum Message {
    // ... existing variants ...

    // Monitor view
    MonitorSelectTask(String),
    MonitorClearCompleted,
}
```

#### 3. Add ViewMode::Monitor
**File**: `descartes-gui/src/views/header.rs`
**Changes**: Add Monitor to the ViewMode enum and nav bar

```rust
pub enum ViewMode {
    Waves,
    Agents,
    Output,
    Monitor, // NEW
}
```

Add a "Monitor" button to the header nav bar after "Output".

### Success Criteria

#### Automated Verification:
- [ ] `cargo check -p descartes-gui` compiles without errors
- [ ] `cargo test -p descartes-gui` — existing tests still pass
- [ ] New state types are accessible from views

#### Manual Verification:
- [ ] "Monitor" tab appears in the header navigation
- [ ] Clicking "Monitor" tab switches to the view (placeholder content for now)

---

## Phase 2: ScudEvent Handling for Headless Sessions

### Overview
Update the `Message::ScudEvent` handler in `main.rs` to populate `headless_sessions` from existing headless event variants. No new ScudBridge changes needed — the events already exist.

### Changes Required

#### 1. Update ScudEvent handler
**File**: `descartes-gui/src/main.rs`
**Changes**: In the `ScudEvent` match arm, populate `headless_sessions`

For `ScudEvent::HeadlessStarted { task_id, harness }`:
```rust
// Look up task title from loaded tasks
let title = self.state.tasks.iter()
    .find(|t| t.id == task_id)
    .map(|t| t.title.clone())
    .unwrap_or_else(|| task_id.clone());

self.state.headless_sessions.insert(task_id.clone(), HeadlessSessionInfo {
    task_id: task_id.clone(),
    task_title: title,
    harness,
    status: HeadlessSessionStatus::Starting,
    event_count: 0,
    line_count: 0,
    output_lines: Vec::new(),
});

// Auto-select first task in monitor
if self.state.monitor_selected_task.is_none() {
    self.state.monitor_selected_task = Some(task_id);
}
```

For `ScudEvent::TaskOutput { task_id, text }`:
```rust
// Update headless session if it exists
if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
    session.output_lines.push(text.clone());
    session.line_count = session.output_lines.len();
    session.event_count += 1;
    if session.status == HeadlessSessionStatus::Starting {
        session.status = HeadlessSessionStatus::Running;
    }
}
// Also append to output_buffer as before
```

For `ScudEvent::ToolStart { task_id, tool_name, input_summary, .. }`:
```rust
if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
    session.output_lines.push(format!(">> {} {}", tool_name, input_summary));
    session.line_count = session.output_lines.len();
    session.event_count += 1;
    if session.status == HeadlessSessionStatus::Starting {
        session.status = HeadlessSessionStatus::Running;
    }
}
```

For `ScudEvent::ToolResult { task_id, tool_name, success, .. }`:
```rust
if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
    let status_str = if success { "ok" } else { "failed" };
    session.output_lines.push(format!("<< {} {}", tool_name, status_str));
    session.line_count = session.output_lines.len();
    session.event_count += 1;
}
```

For `ScudEvent::TaskCompleted { task_id, success }`:
```rust
if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
    session.status = if success {
        HeadlessSessionStatus::Completed
    } else {
        HeadlessSessionStatus::Failed
    };
}
```

For `ScudEvent::SessionAssigned { task_id, .. }`:
```rust
if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
    session.status = HeadlessSessionStatus::Running;
}
```

#### 2. Handle Monitor messages
**File**: `descartes-gui/src/main.rs`

```rust
Message::MonitorSelectTask(task_id) => {
    self.state.monitor_selected_task = Some(task_id);
    Task::none()
}
Message::MonitorClearCompleted => {
    self.state.headless_sessions.retain(|_, s| {
        s.status != HeadlessSessionStatus::Completed
            && s.status != HeadlessSessionStatus::Failed
    });
    Task::none()
}
```

### Success Criteria

#### Automated Verification:
- [ ] `cargo check -p descartes-gui` compiles
- [ ] `cargo test -p descartes-gui` — existing tests pass
- [ ] New test: sending `ScudEvent::HeadlessStarted` populates `headless_sessions`
- [ ] New test: sending `ScudEvent::TaskCompleted` updates session status

#### Manual Verification:
- [ ] Start a headless task via ScudBridge — `headless_sessions` populates (verified via debug logging)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 3: Monitor View

### Overview
Create the `views/monitor.rs` view module with a split-panel layout: task list (left) + streaming output (right).

### Changes Required

#### 1. Create monitor view
**File**: `descartes-gui/src/views/monitor.rs` (NEW)
**Changes**: Split-panel monitoring view

```rust
//! Headless swarm monitoring view
//!
//! Split panel: task list (left) + streaming output (right)

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

use crate::state::{HeadlessSessionInfo, HeadlessSessionStatus};
use crate::theme;
use crate::Message;

pub fn view<'a>(
    sessions: &'a std::collections::HashMap<String, HeadlessSessionInfo>,
    selected_task: &Option<String>,
) -> Element<'a, Message> {
    // ... implementation
}
```

Layout structure:
- **Left panel (30% width)**: Scrollable list of tasks with status badges
  - Each row: `[status_icon] task_title (events: N, lines: N)`
  - Click row → `Message::MonitorSelectTask(task_id)`
  - Selected row highlighted
  - Bottom: "Clear Completed" button → `Message::MonitorClearCompleted`
- **Right panel (70% width)**: Uses `streaming_view::view_strings()` with the selected task's `output_lines`
  - Header shows task title and harness name
  - Scrollable output with auto-scroll

Status badge colors:
- Starting: theme muted/yellow
- Running: theme accent/blue
- Completed: theme success/green
- Failed: theme error/red

Empty state: "No headless sessions. Start a swarm or run a task in headless mode to see output here."

#### 2. Register monitor view
**File**: `descartes-gui/src/views/mod.rs`
**Changes**: Add `pub mod monitor;`

#### 3. Wire into main view
**File**: `descartes-gui/src/main.rs`
**Changes**: Add `ViewMode::Monitor` arm to the `view()` method's match block

```rust
ViewMode::Monitor => views::monitor::view(
    &self.state.headless_sessions,
    &self.state.monitor_selected_task,
),
```

### Success Criteria

#### Automated Verification:
- [ ] `cargo check -p descartes-gui` compiles
- [ ] `cargo test -p descartes-gui` — all tests pass
- [ ] New test: render monitor view with empty sessions → shows placeholder text
- [ ] New test: render monitor view with sessions → shows task list
- [ ] New test: `MonitorSelectTask` updates `monitor_selected_task`

#### Manual Verification:
- [ ] Navigate to Monitor tab — shows empty state message
- [ ] Start headless task — Monitor tab populates with task entry
- [ ] Click task in list — right panel shows that task's output
- [ ] Status badges show correct colors for each state

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 4: Headless Swarm Integration

### Overview
Wire the swarm's `StartSwarm` command to use headless runners per-task (instead of subprocess), so each swarm task creates a `StreamStore` session visible in the Monitor view.

### Changes Required

#### 1. Add RunSwarmHeadless command
**File**: `descartes-gui/src/scud_bridge.rs`
**Changes**: Add a new command variant and handler

```rust
pub enum ScudCommand {
    // ... existing ...
    /// Start swarm execution using headless runners (for monitoring)
    StartSwarmHeadless {
        tag: String,
        harness: String,
        round_size: usize,
    },
}
```

Handler `run_swarm_headless()`:
1. Load tasks and compute waves using scud-core (same as existing `run_swarm`)
2. For each wave, spawn up to `round_size` tasks in parallel using `run_task_headless()`
3. Wait for wave completion before moving to next wave
4. Emit `SwarmStarted`, `WaveStarted`, `WaveCompleted`, `SwarmCompleted` events

#### 2. Add Message variant and wire to Agents view
**File**: `descartes-gui/src/main.rs`
**Changes**: Add `StartSwarmHeadless` message variant

**File**: `descartes-gui/src/views/agents.rs`
**Changes**: Add "Start Headless Swarm" button next to existing "Start Swarm"

### Success Criteria

#### Automated Verification:
- [ ] `cargo check -p descartes-gui` compiles
- [ ] `cargo test -p descartes-gui` — all tests pass

#### Manual Verification:
- [ ] Click "Start Headless Swarm" in Agents view
- [ ] Monitor tab shows per-task entries appearing as tasks start
- [ ] Each task transitions Starting → Running → Completed/Failed
- [ ] Output is visible for each task in the Monitor view
- [ ] Multiple tasks run in parallel within a wave

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Testing Strategy

### Unit Tests
- State transitions: HeadlessSessionInfo status updates from ScudEvent sequences
- Message handling: MonitorSelectTask, MonitorClearCompleted
- View rendering: empty state, single session, multiple sessions
- Session cleanup: ClearCompleted removes finished sessions

### Integration Tests (iced_test)
- Click "Monitor" tab → view switches
- Simulate headless events → monitor populates
- Click task in list → selection changes
- Click "Clear Completed" → finished sessions removed

### Manual Testing Steps
1. Start Descartes GUI with a project that has tasks
2. Navigate to Monitor tab — verify empty state
3. Start a headless swarm from Agents view
4. Watch tasks appear in Monitor tab with real-time status
5. Click between tasks — verify output switching
6. Wait for completion — verify status badges update
7. Click "Clear Completed" — verify cleanup

## Performance Considerations

- Output lines per session are bounded by MAX_OUTPUT_LINES (10,000) in StreamStore
- The GUI only renders the selected task's output, not all tasks simultaneously
- `headless_sessions` HashMap operations are O(1) for updates
- Consider adding line truncation in the monitor view if individual lines are very long

## References

- Headless streaming infrastructure: `scud-cli/src/commands/spawn/headless/store.rs`
- ScudBridge headless support: `descartes-gui/src/scud_bridge.rs:866-1118`
- Streaming view component: `descartes-gui/src/components/streaming_view.rs`
- TUI monitor pattern: `scud-cli/src/commands/spawn/tui/app.rs`
- Existing headless plan: `thoughts/shared/plans/2026-02-03-headless-streaming-mode.md`
