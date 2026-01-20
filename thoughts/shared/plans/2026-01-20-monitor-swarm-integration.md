# Monitor + Swarm Integration Implementation Plan

## Overview

Make `scud monitor` work with swarm sessions by having swarm also write a SpawnSession file to `.scud/spawn/`. This allows the existing monitor infrastructure to discover and display swarm-spawned agents.

## Current State Analysis

### The Problem
- `scud monitor` looks for sessions in `.scud/spawn/` using `monitor::list_sessions()`
- `scud swarm` writes sessions to `.scud/swarm/` using `session::save_session()`
- Different data structures: SpawnSession (flat agent list) vs SwarmSession (wave-based hierarchy)
- When running swarm, `scud monitor` says "no sessions found"

### Key Files
- `scud-cli/src/commands/spawn/monitor.rs`: SpawnSession struct, save/load/list functions
- `scud-cli/src/commands/swarm/mod.rs`: Main swarm logic, calls session::save_session()
- `scud-cli/src/commands/swarm/session.rs`: SwarmSession struct

### Solution Approach (Option A)
Have swarm maintain BOTH:
1. SwarmSession in `.scud/swarm/` (for wave tracking, validation state)
2. SpawnSession in `.scud/spawn/` (for monitor compatibility)

When swarm spawns a task, update both session files.

## Desired End State

1. Start swarm: `scud swarm --tag test`
2. In another terminal: `scud monitor --session swarm-test` shows running agents
3. SpawnSession updated as agents start, complete, or fail
4. Existing SwarmSession behavior unchanged

### Verification:
- `cargo test` passes
- Monitor displays swarm-spawned agents correctly

## What We're NOT Doing

- Not replacing SwarmSession with SpawnSession (swarm needs wave tracking)
- Not modifying the monitor to read from multiple locations
- Not adding bidirectional sync (SpawnSession is write-only from swarm's perspective)

## Implementation Approach

1. Import spawn::monitor module in swarm
2. Create and maintain SpawnSession alongside SwarmSession
3. Update SpawnSession when agents are spawned or complete

## Phase 1: Add SpawnSession Creation to Swarm

### Overview
Create a SpawnSession when swarm starts, alongside the SwarmSession.

### Changes Required:

#### 1.1 Add spawn::monitor import to swarm/mod.rs

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add import at top of file (around line 33-38)

```rust
use crate::commands::spawn::monitor as spawn_monitor;
```

#### 1.2 Create SpawnSession when swarm starts

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: After creating SwarmSession (find `SwarmSession::new`), create SpawnSession

Find the line where SwarmSession is created (should be in the main run loop setup) and add:

```rust
// Create spawn session for monitor compatibility
let mut spawn_session = spawn_monitor::SpawnSession::new(
    &session_name,
    &phase_tag,
    "tmux",
    working_dir.to_str().unwrap_or("."),
);
spawn_monitor::save_session(project_root.as_ref(), &spawn_session)?;
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds

#### Manual Verification:
- [ ] Start swarm, check `.scud/spawn/swarm-<tag>.json` exists

---

## Phase 2: Update SpawnSession When Agents Spawn

### Overview
When swarm spawns an agent, add it to the SpawnSession.

### Changes Required:

#### 2.1 Pass spawn_session to spawn_round function

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Modify `spawn_round` function signature and calls

Add `spawn_session: &mut spawn_monitor::SpawnSession` parameter to `spawn_round`:

```rust
fn spawn_round(
    storage: &Storage,
    project_root: Option<&PathBuf>,
    tasks: &[TaskInfo],
    harness: Harness,
    working_dir: &str,
    session_name: &str,
    spawn_session: &mut spawn_monitor::SpawnSession,  // NEW
) -> Result<RoundState> {
```

#### 2.2 Update spawn_session when agent spawns successfully

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: In spawn_round, after successful spawn (around line 607-631)

After the `println!` confirming spawn, add:

```rust
// Update spawn session for monitor
spawn_session.add_agent(&info.task.id, &info.task.title, &info.tag);
spawn_session.update_agent_status(&info.task.id, spawn_monitor::AgentStatus::Running);
spawn_monitor::save_session(project_root, spawn_session)?;
```

#### 2.3 Update all spawn_round call sites

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Pass spawn_session to all spawn_round calls

Find each call to `spawn_round` and add the spawn_session argument.

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds

#### Manual Verification:
- [ ] Start swarm with tasks, agents appear in `.scud/spawn/swarm-<tag>.json`
- [ ] `scud monitor --session swarm-<tag>` shows running agents

---

## Phase 3: Update SpawnSession on Task Completion

### Overview
When swarm detects task completion/failure, update the SpawnSession agent status.

### Changes Required:

#### 3.1 Pass spawn_session to wait_for_round_completion

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Modify function signature and add status updates

```rust
fn wait_for_round_completion(
    storage: &Storage,
    project_root: Option<&PathBuf>,
    tasks: &[TaskInfo],
    spawn_session: &mut spawn_monitor::SpawnSession,  // NEW
) -> Result<()> {
```

Inside the function, after detecting task status change:

```rust
// When task is no longer in-progress, update spawn session
for task_id in &task_ids {
    if let Some(tag) = task_tags.get(task_id) {
        if let Ok(phase) = storage.load_group(tag) {
            if let Some(task) = phase.get_task(task_id) {
                let new_status = match task.status {
                    TaskStatus::Done => spawn_monitor::AgentStatus::Completed,
                    TaskStatus::Failed => spawn_monitor::AgentStatus::Failed,
                    TaskStatus::InProgress => spawn_monitor::AgentStatus::Running,
                    _ => continue,
                };
                spawn_session.update_agent_status(task_id, new_status);
            }
        }
    }
}
spawn_monitor::save_session(project_root, spawn_session)?;
```

#### 3.2 Update wait_for_round_completion call sites

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Pass spawn_session and project_root to all calls

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes

#### Manual Verification:
- [ ] Start swarm, complete a task, status updates in monitor
- [ ] Failed tasks show as "failed" in monitor
- [ ] Completed tasks show as "completed" in monitor

---

## Phase 4: Cleanup SpawnSession on Swarm Exit

### Overview
Mark the spawn session as complete when swarm finishes.

### Changes Required:

#### 4.1 Delete or mark spawn session on swarm exit

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: At the end of the main run function, before returning

```rust
// Clean up spawn session (optional: could keep it for history)
spawn_monitor::delete_session(project_root.as_ref(), &session_name)?;
```

Or alternatively, leave it for historical reference (sessions are small).

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds

#### Manual Verification:
- [ ] After swarm completes, spawn session is cleaned up (or shows completed state)

---

## Testing Strategy

### Manual Testing Steps:
1. Start swarm with a few tasks: `scud swarm --tag test`
2. In another terminal: `scud monitor --session swarm-test`
3. Verify agents appear as they spawn
4. Verify status updates as tasks complete
5. Verify cleanup on swarm exit

### Edge Cases:
- Swarm crash mid-execution (spawn session left behind)
- Multiple swarm sessions with same tag (should be prevented by lock)

## References

- Swarm main: `scud-cli/src/commands/swarm/mod.rs`
- SpawnSession: `scud-cli/src/commands/spawn/monitor.rs`
- SwarmSession: `scud-cli/src/commands/swarm/session.rs`
