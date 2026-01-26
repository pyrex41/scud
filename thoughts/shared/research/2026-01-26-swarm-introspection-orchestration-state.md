---
date: 2026-01-26T18:47:27+0000
researcher: Claude Opus 4.5
git_commit: 071787290eafe898957d611d821889a813fcc325
branch: master
repository: pyrex41/scud
topic: "Swarm introspection, agent lifecycle visibility, error handling, and restart-ability"
tags: [research, codebase, swarm, orchestration, monitoring, tui, agents, recovery]
status: complete
last_updated: 2026-01-26
last_updated_by: Claude Opus 4.5
---

# Research: Swarm Introspection, Agent Lifecycle Visibility, and Orchestration State

**Date**: 2026-01-26T18:47:27+0000
**Researcher**: Claude Opus 4.5
**Git Commit**: `0717872`
**Branch**: master
**Repository**: pyrex41/scud

## Research Question

How does the current swarm system expose agent status, progress, errors, and execution state to the user? What mechanisms exist for monitoring running agents, detecting failures, restarting work, and understanding where the swarm is at during execution?

## Summary

SCUD has two separate execution paths with different visibility characteristics:

1. **Swarm command** (`scud swarm`) - The primary orchestrator. Runs waves of agents in tmux windows. User-facing output is **print-statement-based**: static headers, one-line spawn confirmations, and polling-based completion detection. There is no live dashboard, no spinners, no heartbeats, and no way to see what agents are doing without manually attaching to tmux.

2. **Spawn/Monitor TUI** (`scud spawn --monitor`) - A ratatui-based interactive terminal UI with three panels (waves, agents, output). Refreshes agent status every 2 seconds and captures tmux pane output every 500ms. This is the closest thing to a live dashboard, but it's wired to the **spawn** command, not the **swarm** command. The swarm command writes a "spawn proxy" JSON file that the TUI can read, but the two systems are loosely coupled.

Agent completion is detected by **polling task status** (agents self-report via hooks) and **checking tmux window existence**. There are no heartbeats, no progress events, and no structured status updates during execution. The gap between "agent spawned" and "agent done" is a black box.

---

## Detailed Findings

### 1. Swarm Command User-Facing Output

**File**: `scud-cli/src/commands/swarm/mod.rs`

The swarm command prints static text at key lifecycle points. There are no spinners, progress bars, or dynamic updates.

#### What the user sees during a swarm run:

**Startup header** (lines 172-238):
```
SCUD Swarm Mode
==================================================
Tag:                 backend
Round size:          5
Research:            enabled
Validation:          enabled
Mode:                tmux (waves)
Harness:             claude
Review:              disabled
Repair:              up to 3 attempts
```

**Per-wave header** (lines 439-446):
```
Wave 1 - 3 task(s)
----------------------------------------
```

**Per-round header** (lines 462-469):
```
  Round 1/2 - 2 task(s)
```

**Per-agent spawn confirmation** (lines 1017-1025):
```
    Spawned: task:1 | Task title [claude] session:3
```

**Then silence.** The user sees "Waiting for round completion..." and nothing else until the round finishes.

**Round completion wait** (`wait_for_round_completion()`, lines 1272-1305):
- Infinite loop with 5-second sleep
- For each task: reloads storage, checks if status is still `InProgress`
- No output during the wait
- No indication of which agents are still running
- No progress updates

**Validation output** (lines 531-591):
```
  Validate: Running backpressure checks...
    All checks passed
```
or
```
    ! Some checks failed:
      - cargo build
```

**Final summary** (lines 691-706):
```
Swarm Session Summary
========================================
  Waves completed: 3
  Tasks executed: 15
  Spawn proxy updated for monitor/TUI
```

#### Key observation: No mid-execution visibility

Between the spawn confirmation and round completion, the user has **zero visibility** into:
- Which agents are still running
- What any agent is currently doing
- Whether an agent is stuck or progressing
- How long each agent has been running
- Which tasks completed vs which are still in-progress

### 2. Beads Mode Output

**File**: `scud-cli/src/commands/swarm/beads.rs`

Beads mode is slightly more informative because it polls continuously:

```
Beads Execution Mode
==================================================
  Mode: Continuous ready-task polling
  Max concurrent: 5
  Poll interval: 3000ms
```

**During execution** (lines 318-449):
```
  Spawned: task:1 | Task title [session:3]
  3 task(s) in progress, waiting...          (updates in-place via \r)
  Completed: task:1 (5000ms)
  Spawned: task:2 | Build login form [session:4]
```

Beads mode provides per-task completion notifications with duration, which wave mode does not. However, it still has no information about what agents are *doing* between spawn and completion.

### 3. Spawn TUI (Monitor) - The Existing Dashboard

**Files**: `scud-cli/src/commands/spawn/tui/` (11 files)

The spawn TUI is a ratatui-based three-panel interface:

#### Panels

**Waves Panel** (`tui/waves.rs`):
- Shows all execution waves with tasks grouped by wave
- Each task shows state: Ready / Running / Done / Blocked / InProgress
- Tasks can be selected for spawning with Space, batch-selected with `a`
- Spawned with `s` key

**Agents Panel** (`tui/agents.rs`):
- Shows spawned agents with status: Starting / Running / Completed / Failed
- Selected agent's output shown in output panel

**Output Panel** (`tui/output.rs`):
- Captures last 100 lines of tmux pane content for selected agent
- Refreshes every 500ms via `tmux capture-pane -t <session:window> -p -S -100`
- Scrollable with PageUp/PageDown, auto-scroll follows new output

#### Refresh Mechanism (`tui/app.rs`)

| What | Interval | Method |
|------|----------|--------|
| Agent status | 2 seconds | `refresh_agent_statuses()` - polls tmux windows + task status |
| Terminal output | 500ms | `refresh_live_output()` - captures tmux pane content |
| Ralph auto-spawn | 5 seconds | `ralph_auto_spawn()` - finds and spawns ready tasks |
| TUI event poll | 100ms | Crossterm event polling for keyboard input |

#### Agent Status Detection (`app.rs:332-359`)

Status is computed from two sources:
1. **Task status** from SCUD storage (the `.scg` file)
2. **Tmux window existence** via `tmux list-windows`

Logic:
- Task `Done` -> `Completed`
- Task `Blocked` -> `Failed`
- Task `InProgress` + window exists -> `Running`
- Task `InProgress` + no window -> `Completed` (assumed finished)
- No window -> `Completed`
- Window exists -> `Running`

#### Key Bindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between panels |
| `j`/`k` or arrows | Navigate within panel |
| `Space` | Toggle task selection (Waves panel) |
| `s` | Spawn selected tasks |
| `Enter` | Toggle fullscreen for current panel |
| `i` | Enter input mode (send commands to agent tmux pane) |
| `x` | Interrupt agent (send Ctrl+C to tmux pane) |
| `r` | Force refresh |
| `R` | Toggle Ralph mode (auto-spawn ready tasks) |
| `d`/`p`/`b` | Mark task Done/Pending/Blocked |
| `W` | Start swarm execution |
| `?` | Toggle help overlay |

### 4. The Swarm-to-TUI Bridge

The swarm command and spawn TUI are **separate processes**. They communicate via a JSON file:

**Spawn Proxy** (`swarm/mod.rs:680-689`):
- Swarm writes a `SpawnSession` JSON at `.scud/spawn/{session_name}.json`
- Contains agent states for all spawned tasks
- Updated after each round and at session end
- The TUI can read this file to see swarm state

**Gap**: The swarm command creates the proxy file, but:
- It only updates between rounds, not during agent execution
- The TUI must be launched separately (`scud spawn --monitor --session <name>`)
- There's no prompt or guidance from the swarm about how to monitor it
- The session name isn't prominently displayed

### 5. Agent Completion Detection

The system has **no heartbeat mechanism**. Completion is detected through three strategies depending on mode:

#### Tmux Mode (Wave + Beads)
- **Primary**: Agents self-report completion via Claude Code Stop hook:
  ```bash
  if [ -n "$SCUD_TASK_ID" ]; then
    scud set-status "$SCUD_TASK_ID" done 2>/dev/null || true
  fi
  ```
  Defined in `spawn/hooks.rs:73-86`
- **Secondary**: TUI checks tmux window existence
- **Polling interval**: 5 seconds in swarm wave mode, 3 seconds in beads mode, 2 seconds in TUI

#### Extensions Mode
- Direct subprocess monitoring via tokio task handles
- `runner.wait_all().await` blocks until all processes exit
- Exit code determines success/failure

#### Server Mode (OpenCode)
- OpenCode server sends SSE events for agent lifecycle
- `orchestrator.wait_all().await` waits for server to report completion

### 6. Error Handling and Failure Detection

#### Spawn Failures (`swarm/mod.rs:1036-1039`)
- If tmux window creation fails, error is printed and task added to `round_state.failures`
- Execution continues with remaining tasks
- Failed task remains in whatever state it was in

#### Validation Failures (`swarm/mod.rs:531-591`)
- Backpressure commands run after each wave
- On failure, two paths:
  - **No repair**: All wave tasks marked `Failed`
  - **Repair enabled**: Enters repair loop

#### Repair Loop (`swarm/mod.rs:1527-1710`)
1. **Attribution**: Uses `git blame` + error output to identify responsible task(s)
2. **Confidence levels**: High (single task), Medium (multiple), Low (unknown)
3. **Repair agent**: Spawns "repairer" agent in tmux with error context
4. **Wait**: Polls for `.scud/repair-complete-{task_id}` marker file (30min timeout, 5s poll)
5. **Re-validate**: Runs backpressure again
6. **Retry**: Up to `max_repair_attempts` (default 3)
7. **Exhausted**: Marks responsible tasks as `Failed`

#### Orphan Detection (`swarm/mod.rs:273-351`)
- At swarm startup only, checks for tasks with `InProgress` status but no tmux window
- Presents interactive prompt: Reset / Kill+Restart / Skip / Abort
- Does NOT detect orphans during execution

#### Doctor Command (`commands/doctor.rs`)
- Offline diagnostic tool, not integrated into swarm
- Detects stale `InProgress` tasks by timestamp (configurable threshold)
- Detects dependency issues (missing, cancelled, blocked)
- Auto-fix capability: resets stale tasks to `Pending`
- Does NOT check tmux sessions or running processes

### 7. Restart Mechanisms

#### Restart Command (`commands/restart.rs`)
- `scud restart <task-id>` resets a single task to `Pending` and re-spawns
- Resolves agent config from task's `agent_type` field
- Spawns new tmux window
- Updates task to `InProgress`
- Optionally attaches to tmux session

#### Ralph Loop (`commands/ralph.rs`)
- Sequential task execution with fresh context per task
- Each task gets a new agent instance in a new tmux window
- Built-in validation + repair loop per task
- Auto-retries with repair agents on validation failure
- Ralph mode also available in TUI via `R` key toggle

#### Manual Reset
- `scud set-status <id> pending` resets any task
- `scud doctor --fix` bulk-resets stale tasks

### 8. Session and Event Persistence

#### Session JSON (`swarm/session.rs`)
- Path: `.scud/swarm/{session_name}.json`
- Contains: `SwarmSession` with waves, rounds, task IDs, validation results, repair attempts
- Written after each wave completes
- Can be loaded later for retrospective analysis

#### SQLite Events (`db/events.rs`)
- Path: `.scud/scud.db`
- Events: Spawned, Started, Completed, Failed, ToolCall, ToolResult, FileRead, FileWrite, Unblocked
- Currently only written by beads mode (`EventWriter`)
- Wave mode does NOT write events to SQLite
- `RetrospectiveTimeline` can reconstruct per-task timelines from events

#### Spawn Session JSON (`spawn/monitor.rs`)
- Path: `.scud/spawn/{session_name}.json`
- Contains: `SpawnSession` with agent states (Starting/Running/Completed/Failed)
- Read by TUI for monitoring
- Written by swarm as "spawn proxy" between rounds

### 9. Tmux Session Structure

#### Session Naming
- Swarm creates tmux session named `swarm-{tag}` (or custom `--session` name)
- Each agent gets a window: `task-{task_id}`
- Ralph agents get: `ralph-{task_id}`
- Repair agents get: `repair-{task_id}`
- Control window: `ctrl` (first window, used for monitoring)

#### Window Discovery
- `tmux list-windows -t <session> -F "#{window_index}:#{window_name}"` (`terminal.rs:635-649`)
- Fuzzy matching: `starts_with` in either direction
- No centralized registry of which window is which agent

#### Attach Commands
- After swarm launch, user must know: `tmux attach -t swarm-{tag}`
- Within session, must navigate windows manually (Ctrl+B, n/p)
- No guidance about which window has which agent
- TUI provides this mapping if launched separately

---

## Architecture Documentation

### Execution Mode Comparison

| Feature | Wave Mode | Beads Mode | Extensions Mode | Server Mode |
|---------|-----------|------------|-----------------|-------------|
| Execution | Batched waves | Continuous polling | Async subprocess | OpenCode API |
| Terminal | tmux windows | tmux windows | No terminal | No terminal |
| Completion detect | Status polling (5s) | Status polling (3s) | Process exit | Server events |
| Live output | None (print stmts) | Carriage-return updates | None | None |
| Validation | Per-wave | Not integrated | Per-wave | Per-wave |
| Repair | Per-wave | Not integrated | Per-wave | Per-wave |
| Event logging | No (proxy only) | SQLite events | Via runner | Via orchestrator |
| TUI support | Via spawn proxy | Not integrated | Not applicable | Not applicable |

### State Flow

```
User runs: scud swarm --tag backend
    |
    v
[Startup] -> Print config header -> Check tmux -> Acquire lock
    |
    v
[Orphan check] -> Interactive prompt if orphans found
    |
    v
[Wave loop]
    |
    +-> Load tasks, compute waves (Kahn's algorithm)
    |
    +-> For each round in wave:
    |       +-> Print round header
    |       +-> Spawn agents in tmux (500ms between each)
    |       +-> Print "Waiting for round completion..."
    |       +-> Poll every 5s until all tasks not InProgress
    |       +-> (no output during wait)
    |
    +-> Run validation (if enabled)
    |       +-> On failure: repair loop or mark failed
    |
    +-> Run review (if enabled)
    |       +-> Spawn reviewer, wait for marker file
    |
    +-> Save session JSON
    +-> Write spawn proxy JSON
    +-> Increment wave, repeat
    |
    v
[Summary] -> Print wave count, task count
```

### Status Detection Chain

```
Agent completes work in tmux
    |
    v
Claude Code exits -> Stop hook fires
    |
    v
Hook runs: scud set-status $SCUD_TASK_ID done
    |
    v
Storage updated: task status = Done in .scg file
    |
    v
Swarm polls storage (5s interval)
    |
    v
Detects task no longer InProgress -> Round complete
```

### File-Based Communication

```
.scud/
  scud.db                          <- SQLite: events (beads only)
  swarm/
    {session}.json                  <- SwarmSession: waves, rounds, validation
    {tag}.lock                      <- Exclusive session lock (released on exit)
  spawn/
    {session}.json                  <- SpawnSession: agent states (TUI reads this)
  repair-complete-{task_id}         <- Marker file: repairer writes, swarm polls
  review-complete-{wave_number}     <- Marker file: reviewer writes, swarm polls
```

---

## Code References

### Swarm Orchestration
- `scud-cli/src/commands/swarm/mod.rs:71-88` - `run()` entry point with 14+ parameters
- `scud-cli/src/commands/swarm/mod.rs:172-238` - Configuration header display
- `scud-cli/src/commands/swarm/mod.rs:273-351` - Orphan task detection and handling
- `scud-cli/src/commands/swarm/mod.rs:404-677` - Main wave execution loop
- `scud-cli/src/commands/swarm/mod.rs:984-1046` - `execute_round()` tmux agent spawning
- `scud-cli/src/commands/swarm/mod.rs:1272-1305` - `wait_for_round_completion()` polling loop
- `scud-cli/src/commands/swarm/mod.rs:1527-1710` - `run_repair_loop()` failure attribution and repair

### Beads Mode
- `scud-cli/src/commands/swarm/beads.rs:249-489` - `run_beads_loop()` continuous execution
- `scud-cli/src/commands/swarm/beads.rs:67-122` - `get_ready_tasks()` with priority sorting
- `scud-cli/src/commands/swarm/beads.rs:199-213` - `claim_task()` atomic status update

### TUI System
- `scud-cli/src/commands/spawn/tui/app.rs:105-170` - `App` struct with all state fields
- `scud-cli/src/commands/spawn/tui/app.rs:225-260` - `refresh()` session data reload
- `scud-cli/src/commands/spawn/tui/app.rs:263-329` - `refresh_live_output()` tmux pane capture
- `scud-cli/src/commands/spawn/tui/app.rs:332-359` - `refresh_agent_statuses()` status computation
- `scud-cli/src/commands/spawn/tui/mod.rs:136-342` - Event loop with key bindings

### Agent Lifecycle
- `scud-cli/src/commands/spawn/terminal.rs:265-378` - `spawn_tmux()` window creation
- `scud-cli/src/commands/spawn/terminal.rs:423-592` - `spawn_tmux_ralph()` retry loop
- `scud-cli/src/commands/spawn/hooks.rs:73-86` - Stop hook for completion detection
- `scud-cli/src/commands/spawn/monitor.rs:14-30` - `AgentStatus` and `AgentState` structs

### Session Persistence
- `scud-cli/src/commands/swarm/session.rs:140-205` - `WaveState` struct
- `scud-cli/src/commands/swarm/session.rs:207-277` - `SwarmSession` struct
- `scud-cli/src/commands/swarm/session.rs:339-379` - `acquire_session_lock()`
- `scud-cli/src/commands/swarm/session.rs:387-396` - `save_session()`

### Events and Recovery
- `scud-cli/src/commands/swarm/events.rs:14-71` - `EventKind` enum (12 variants)
- `scud-cli/src/commands/swarm/events.rs:138-196` - `EventWriter`
- `scud-cli/src/commands/swarm/events.rs:230-325` - `RetrospectiveTimeline`
- `scud-cli/src/commands/restart.rs:18` - Restart command entry
- `scud-cli/src/commands/ralph.rs:156` - Ralph loop entry
- `scud-cli/src/commands/doctor.rs:94` - Doctor diagnostics entry
- `scud-cli/src/backpressure.rs:226-284` - `run_validation()` command execution

---

## Related Research

- `thoughts/shared/research/2026-01-20-swarm-monitor-real-time.md` - Real-time swarm monitoring research
- `thoughts/shared/research/2026-01-20-swarm-monitor-bridge.md` - Swarm-monitor bridge patterns
- `thoughts/shared/research/2026-01-21-monitor-swarm-waves-task-reset.md` - Monitor-swarm waves and task reset
- `thoughts/shared/plans/2026-01-21-monitor-status-update-swarm-start.md` - Monitor and status update plans
- `thoughts/shared/plans/2026-01-20-monitor-swarm-integration.md` - Monitor-swarm integration plan
- `thoughts/shared/plans/2026-01-25-orchestration-sqlite-salvos.md` - SQLite events and salvo worktrees

---

## Open Questions

1. **Event parity**: Wave mode does not write SQLite events (only beads mode does). Should wave mode also emit events for retrospective analysis?
2. **TUI integration**: The spawn TUI reads a "spawn proxy" JSON, but the swarm only writes it between rounds. Should the swarm update the proxy more frequently (per-spawn)?
3. **Beads + TUI**: Beads mode has no TUI integration at all. The TUI's Ralph mode is the closest equivalent but runs through a different code path.
4. **Server/Extensions modes**: These modes have no tmux windows, so the TUI's window-based monitoring doesn't apply. What monitoring story do these modes need?
5. **Cross-process communication**: Currently file-based (JSON, marker files). Would a socket, pipe, or SQLite-based approach enable richer real-time updates?
