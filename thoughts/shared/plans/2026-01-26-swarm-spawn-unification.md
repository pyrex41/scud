---
date: 2026-01-26
author: Claude Opus 4.5
status: draft
tags: [plan, swarm, spawn, tui, orchestration, unification]
prerequisite_research: thoughts/shared/research/2026-01-26-swarm-introspection-orchestration-state.md
---

# Plan: Unify Spawn + Swarm into a Single Orchestration System

## Problem Statement

SCUD has two parallel agent execution systems that evolved independently:

1. **`scud spawn`** - Simple one-shot spawning with optional TUI monitor, ralph mode, interactive task selection
2. **`scud swarm`** - Wave-based orchestration with validation, repair, review, research, salvo worktrees

They share underlying code (`spawn/agent`, `spawn/terminal`, `spawn/hooks`, `spawn/monitor`) but have different user experiences, different monitoring stories, and different feature sets. The result:

- **Confusing surface area**: 7 related commands (`spawn`, `monitor`, `sessions`, `swarm`, `restart`, `ralph`, `run`)
- **Duplicated concepts**: Both have session management, agent spawning, task selection
- **Split monitoring**: The TUI lives under spawn but swarm is the primary execution path. A lossy JSON proxy bridges them.
- **Visibility black hole**: During swarm wave execution, users see nothing between "Spawned: task:1" and wave completion. No heartbeats, no progress, no indication of life.
- **Feature fragmentation**: Ralph mode is in spawn TUI but not in swarm. Beads mode is in swarm but has no TUI. Wave validation is in swarm but not spawn.

## Design Principles

1. **One command to rule them all**: `scud swarm` is the entry point. Spawn, Ralph, Monitor, Sessions become flags/subcommands of swarm or are retired.
2. **TUI-first**: The TUI launches by default when running swarm. Headless mode is opt-in (`--no-tui` or `--headless`).
3. **Simple process model**: Start with foreground process + background thread. Leverage tmux for detachment (`--detach`). Reserve full daemon architecture for when simpler approaches prove insufficient.
4. **Visibility by default**: Every agent gets heartbeat tracking. The wait loop emits progress. The TUI shows real-time status.
5. **Progressive disclosure**: Simple `scud swarm --tag X` just works. Power features (repair, review, salvo) are opt-in flags.

## Architecture Overview

```
scud swarm --tag backend
    |
    ├── [default]   TUI foreground + orchestrator background thread
    ├── [--headless] Orchestrator foreground, print-statement output
    ├── [--detach]   Orchestrator in own tmux window, return to shell
    └── [--attach]   TUI only, reads state from existing session

    v
[Orchestrator] ──writes──> SQLite events (all modes)
    |                       Session JSON (.scud/swarm/*.json)
    |
    +── Wave/Beads/Ralph execution loop
    |       |
    |       +── Spawn agents in tmux
    |       +── Poll completion (5s) WITH progress output
    |       +── Periodic orphan detection (30s)
    |       +── Stale task timeout (configurable, default 30min)
    |       +── Validate (backpressure)
    |       +── Repair loop (if needed)
    |       +── Review (if enabled)
    |
    +── TUI (foreground, default)
            |
            +── Reads SQLite events + session JSON + tmux panes
            +── Three panels: Waves | Agents | Output
            +── Live heartbeat indicators
            +── Interactive: spawn, restart, attach, ralph toggle
```

## Phases

### Phase 1: Swarm Visibility (the "black hole" fix)

**Goal**: Make swarm wave mode actually tell you what's happening.

No TUI changes yet - just fix the headless output so it's not silent during execution.

#### 1a. Live progress during `wait_for_round_completion()` ✅

Currently the wait loop (`swarm/mod.rs:1272-1305`) is a silent 5-second poll. Change it to:

```
  Round 1/2 - 2 task(s)
    Spawned: task:1 | Build auth module [claude] session:3
    Spawned: task:2 | Add login page [claude] session:4
    Waiting... [2 running] ⣾ 15s
    Waiting... [1 running: task:2] ⣽ 20s
    ✓ task:1 completed (18s)
    ✓ task:2 completed (25s)
  Round complete (25s)
```

Implementation:
- In `wait_for_round_completion()`, track per-task start times
- On each poll cycle, print a carriage-return (`\r`) status line showing: running count, elapsed time, spinner
- When a task transitions from InProgress to Done/Failed, print a completion line
- Track which tasks completed since last poll

Files: `scud-cli/src/commands/swarm/mod.rs` (modify `wait_for_round_completion()`)

#### 1b. SQLite event parity for wave mode ✅

Currently only beads mode writes SQLite events. Add event emission to wave mode so retrospectives work for all execution modes.

Events to emit:
- `Spawned` when agent tmux window is created
- `Completed` when task status transitions to Done
- `Failed` when task status transitions to Blocked/Failed
- `WaveStarted` / `WaveCompleted` (new event kinds)
- `ValidationStarted` / `ValidationPassed` / `ValidationFailed`
- `RepairStarted` / `RepairCompleted`

Implementation:
- Create an `EventWriter` in `run()` at the start of swarm execution
- Pass it through the wave loop, `execute_round()`, `wait_for_round_completion()`, validation, repair
- Emit events at each lifecycle point

Files:
- `scud-cli/src/commands/swarm/events.rs` (add new EventKind variants)
- `scud-cli/src/commands/swarm/mod.rs` (wire EventWriter into wave execution)

#### 1c. Heartbeat / last-activity tracking ✅

Add a lightweight heartbeat mechanism so we can tell if an agent is alive vs stuck:

- When spawning each agent, record `spawned_at` timestamp in session JSON
- During `wait_for_round_completion()`, check each agent's tmux pane for output activity using `tmux capture-pane` and comparing to last known content hash
- Store `last_activity_at` per agent in the session JSON
- In headless mode, flag agents with no activity for >60s as "possibly stuck"
- In the session JSON and SQLite, this enables post-hoc "how long was this agent idle?"

Files:
- `scud-cli/src/commands/swarm/session.rs` (add `spawned_at`, `last_activity_at` to agent tracking)
- `scud-cli/src/commands/swarm/mod.rs` (heartbeat check in wait loop)

#### 1d. Stale task timeout in wait loops ✅

The `wait_for_round_completion()` loop (`swarm/mod.rs:1272-1305`) polls forever with no timeout. If an agent dies without updating its task status (e.g., tmux window killed, hook fails), the orchestrator hangs indefinitely. Similarly, the "waiting for in-progress tasks" loop at startup (`mod.rs:426-436`) has no timeout.

Fix both loops:

- Add a configurable stale threshold (default: 30 minutes)
- On each poll cycle, check if any InProgress task has exceeded the threshold
- Cross-reference with tmux window existence: if task is InProgress but tmux window is gone, the agent died without reporting
- When a stale task is detected:
  - In headless mode: print warning, reset task to Pending, continue
  - In TUI mode: highlight the task in red, let user decide (reset/skip/abort)
- Add `--stale-timeout <minutes>` flag to swarm command

Files:
- `scud-cli/src/commands/swarm/mod.rs` (modify `wait_for_round_completion()` and startup wait loop)

#### 1e. Periodic orphan detection during execution ✅

Currently orphan detection (`swarm/mod.rs:273-351`) only runs once at startup. If a tmux window dies mid-wave (user kills it, OOM, crash), the task stays InProgress forever and blocks all dependents.

Add periodic orphan scanning to the wait loop:

- Every 30 seconds during `wait_for_round_completion()`, check if each InProgress task's tmux window still exists via `tmux_window_exists_for_task()`
- If a window is gone but task is still InProgress: the agent died without completing
- Mark the task as Failed and emit a `Failed` SQLite event with reason "agent window disappeared"
- Log a warning: `⚠ task:3 agent died (tmux window gone), marking failed`
- This allows the wave to complete (failed tasks don't block the wait loop) and the repair loop to pick up the failure

Files:
- `scud-cli/src/commands/swarm/mod.rs` (add orphan check to `wait_for_round_completion()`)

### Phase 2: TUI as Default Swarm Interface

**Goal**: When you run `scud swarm`, you get the TUI. The orchestration loop runs in the background (same process, separate thread).

#### 2a. Move TUI from spawn to shared location

The TUI code currently lives at `commands/spawn/tui/`. It's already capable of monitoring swarm sessions (via `swarm_mode: bool`). Move it to a shared location and make it the primary monitoring interface.

- Move `commands/spawn/tui/` to `commands/tui/` (or `tui/` at crate root)
- Update all imports
- The TUI keeps reading from the same data sources (session JSON, SQLite, tmux panes)

Files: All files under `scud-cli/src/commands/spawn/tui/` -> new location

#### 2b. Swarm launches TUI by default

When `scud swarm --tag X` runs:
1. Start the orchestration loop in a background thread
2. Launch the TUI in the foreground (main thread)
3. The TUI reads session state that the orchestrator writes
4. On TUI exit (q), the orchestrator keeps running (agents are in tmux anyway)
5. `--no-tui` / `--headless` flag to get the current print-statement behavior

The orchestrator thread communicates to the TUI via:
- Session JSON (already written per-wave)
- SQLite events (from Phase 1b)
- Shared atomic state for "current wave number", "orchestrator status"

Implementation:
- In `swarm/mod.rs::run()`, spawn the orchestration loop in `std::thread::spawn`
- The main thread starts the TUI with `swarm_mode: true`
- Use `Arc<AtomicBool>` for "orchestrator running" flag
- TUI already has a refresh loop that reads session data

Files:
- `scud-cli/src/commands/swarm/mod.rs` (thread spawn, TUI launch)
- TUI `app.rs` (read orchestrator status)

#### 2c. Enhanced TUI for swarm monitoring

Upgrade the TUI to show swarm-specific information:

- **Header bar**: Show orchestrator status (Running Wave 2/5 | Validating | Repairing | Complete)
- **Progress bar**: Wave N/M, Tasks done/total
- **Agent heartbeat indicators**: Green dot = active (recent output), Yellow = idle (>30s), Red = no window
- **Per-agent elapsed time**: Show how long each agent has been running
- **Event log panel** (optional toggle): Show recent SQLite events as a scrollable log
- **Keyboard shortcuts**: `a` to attach to selected agent's tmux (opens new terminal), `r` to restart selected agent

Files:
- `tui/header.rs` (swarm status bar)
- `tui/agents.rs` (heartbeat indicators, elapsed time)
- `tui/app.rs` (event log data, attach/restart actions)

#### 2d. Detach mode (`--detach`)

A lightweight "daemon-like" option that requires zero new infrastructure: run the orchestrator in its own tmux window.

`scud swarm --tag X --detach`:
1. Creates a tmux window (in the swarm session) running `scud swarm --tag X --headless`
2. Prints: `Swarm orchestrator detached in tmux session 'swarm-backend' window 'ctrl'`
3. Prints: `Monitor with: scud swarm --attach swarm-backend`
4. Returns immediately to the user's shell

This gives the orchestrator full process independence (survives terminal closure, SSH disconnect) without any daemon infrastructure. The user already has tmux — we just use it for the orchestrator too, not just the agents.

To reattach: `scud swarm --attach <session>` launches the TUI pointed at that session.

Implementation:
- In `swarm/mod.rs::run()`, if `--detach`: spawn `scud swarm --headless` in tmux, exit
- The detached swarm is just a normal headless swarm running in tmux
- All the same resilience applies: SQLite events, session JSON, heartbeats

Files:
- `scud-cli/src/commands/swarm/mod.rs` (detach logic)
- `scud-cli/src/main.rs` (add `--detach` flag)

### Phase 3: Command Consolidation

**Goal**: Reduce the 7 execution commands to 2-3.

#### 3a. Deprecate `scud spawn`

`scud spawn` becomes an alias for `scud swarm --mode interactive` (or similar). In interactive mode:
- TUI launches immediately
- No automatic wave execution
- User manually selects and spawns tasks via the TUI (Space to select, `s` to spawn)
- Ralph mode toggle (`R`) available
- This is exactly what `scud spawn --monitor` does today

Implementation:
- Add `--interactive` / `-i` flag to swarm
- When interactive: skip the orchestration thread, just launch TUI in spawn mode
- `scud spawn` prints deprecation notice and forwards to `scud swarm --interactive`

#### 3b. Fold `scud monitor` into `scud swarm`

`scud monitor` becomes `scud swarm --attach <session>`:
- Launches TUI pointed at an existing session
- No orchestration thread
- Read-only monitoring of a running swarm

Or keep `scud monitor` as a convenience alias.

#### 3c. Fold `scud ralph` into `scud swarm --mode ralph`

Ralph mode becomes a swarm execution mode alongside wave and beads:
- `scud swarm --tag X --swarm-mode ralph`
- Sequential execution with retry loops
- Gets all the same benefits: TUI, events, heartbeats

#### 3d. Fold `scud restart` into swarm TUI

The TUI's `x` (restart) key already sends Ctrl+C. Enhance it to:
- Reset task to Pending
- Kill the tmux window
- Re-spawn with fresh prompt
- This replaces the need for a separate `restart` command

Keep `scud restart <task-id>` as a CLI convenience but it just calls the same logic.

#### 3e. Fold `scud sessions` into `scud swarm sessions`

Make it a subcommand: `scud swarm sessions` lists all swarm/spawn sessions.

#### 3f. Keep `scud run` separate

`scud run` is for ad-hoc prompts, not task execution. It stays as-is.

### Phase 4: Cleanup

#### 4a. Consolidate session types

Currently there are two session types:
- `SpawnSession` (spawn/monitor.rs) - agent states
- `SwarmSession` (swarm/session.rs) - waves, rounds, validation

Merge them into a single `Session` type that supports both:
- Wave execution metadata (waves, rounds, validation)
- Agent state tracking (status, heartbeat, window)
- The spawn proxy bridge goes away because there's one unified session

#### 4b. Consolidate monitor module

Move agent status detection from `spawn/monitor.rs` into the unified session module. The TUI and headless mode both use the same status detection logic.

#### 4c. Remove dead code

- Remove `commands/spawn/mod.rs` `run()` function (replaced by swarm interactive mode)
- Remove `commands/ralph.rs` (replaced by swarm ralph mode)
- Remove spawn proxy bridge code from `swarm/mod.rs`
- Clean up unused imports

## CLI Surface Area: Before and After

### Before (7 execution commands)
```
scud spawn [--monitor] [--claim] [--attach]    # One-shot spawn + optional TUI
scud monitor [--session] [--swarm]             # TUI for existing session
scud sessions                                   # List sessions
scud swarm --tag X [--swarm-mode beads]        # Wave/beads orchestration (headless)
scud restart <task-id>                          # Reset + respawn single task
scud ralph --tag X                              # Sequential retry loop
scud run "prompt"                               # Ad-hoc agent
```

### After (2 execution commands + 1 subcommand)
```
scud swarm --tag X                              # Wave orchestration + TUI (default)
scud swarm --tag X --headless                   # Wave orchestration, no TUI
scud swarm --tag X --detach                     # Orchestrator in tmux, return to shell
scud swarm --tag X --interactive                # Manual spawning via TUI (replaces spawn)
scud swarm --tag X --swarm-mode beads           # Beads continuous mode + TUI
scud swarm --tag X --swarm-mode ralph           # Sequential retry mode + TUI
scud swarm --attach <session>                   # Monitor existing session (replaces monitor)
scud swarm sessions                             # List sessions (replaces sessions command)
scud run "prompt"                               # Ad-hoc agent (unchanged)
```

Deprecated (with forwarding):
```
scud spawn    -> scud swarm --interactive
scud monitor  -> scud swarm --attach
scud sessions -> scud swarm sessions
scud ralph    -> scud swarm --swarm-mode ralph
scud restart  -> TUI 'x' key, or scud swarm restart <task-id>
```

## Implementation Order

**Phase 1** (Visibility + Resilience) is the highest-value, lowest-risk change. It fixes both the worst UX problem (the black hole) and two real bugs (infinite hangs on stale tasks, undetected mid-execution orphans). No restructuring needed.

**Phase 2** (TUI-first + Detach) is the biggest architectural change but builds on existing TUI code that already supports swarm mode. The `--detach` flag is low-effort since we already have tmux infrastructure.

**Phase 3** (Consolidation) is mostly CLI routing changes and deprecation notices. Can be done incrementally.

**Phase 4** (Cleanup) is internal refactoring. Do it last when everything works.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| TUI + orchestrator in same process: crash in one kills both | Agents run in tmux and survive. `scud swarm --attach` reconnects. Orchestrator state saved to disk per-wave. `--detach` runs orchestrator in tmux for full independence. |
| Thread communication complexity | Minimal shared state: just an AtomicBool for "running" + file-based communication (same as today) |
| Breaking existing workflows | Deprecation aliases forward old commands to new ones. No hard breaks in Phase 1-2. |
| TUI doesn't work in all terminals | `--headless` flag provides fallback. Phase 1 improves headless mode too. |
| SQLite write contention (orchestrator + events) | Single writer (orchestrator thread). TUI is read-only. Same as current beads mode. |
| Agent dies without updating status | Phase 1d adds stale timeout + tmux window check. Phase 1e adds periodic orphan detection. Failed agents get picked up by repair loop. |
| Orchestrator dies mid-wave | Agents complete independently (hooks write status to disk). On restart, swarm resumes from next pending wave. `--detach` minimizes this risk. |

## Process Lifecycle

### Why Not a Daemon (Yet)

The system has strong natural resilience that reduces the need for a daemon:

1. **Agents are independent**: They run in tmux windows and survive orchestrator death. Claude Code's stop hook writes task status directly to disk — no orchestrator involvement needed.
2. **Locks auto-release**: Session locks use OS-level `flock` which the kernel releases on process termination, even on `kill -9`.
3. **State is durable**: Session JSON and task status are written to disk at wave boundaries. On restart, the swarm naturally resumes from the first pending wave.
4. **`--detach` covers the main use case**: Running the orchestrator in tmux gives process independence without daemon complexity (no PID files, no double-forking, no signal handling, no IPC protocol).

### What a Daemon Would Add

A dedicated daemon process would solve two problems that `--detach` doesn't:

1. **Multiple CLI clients**: Several terminals could query swarm status, attach/detach the TUI, or issue commands — all talking to the same daemon via Unix socket. Today, `--attach` can only read state from files; it can't send commands to the orchestrator.
2. **Programmatic control**: External tools (CI, editors, web dashboards) could control the swarm via an IPC API without shelling out to `scud` commands.

### When to Revisit

Consider a daemon if any of these become pain points:

- Users frequently want to control a running swarm from a different terminal (not just monitor it)
- Integration with external tools requires a stable API endpoint
- The file-polling approach (SQLite + JSON) becomes a bottleneck for real-time updates
- Multi-swarm coordination is needed (orchestrating swarms across multiple tags/repos)

The architecture in this plan (SQLite events, session JSON, unified session type) creates the right foundation — a daemon could be layered on top later by adding a Unix socket listener that reads/writes the same SQLite + session state.

## Scope Boundaries

- **No WebSocket/socket IPC** (in this plan): File-based communication (SQLite + JSON) is sufficient. A daemon with Unix socket IPC is a future option if needed.
- **No live streaming of agent output**: The TUI polls tmux panes (500ms). This is good enough. Real streaming would require piping agent stdout.
- **No changes to agent spawning**: Agents still run in tmux windows with the same hooks. The improvements are all on the orchestrator/monitoring side.
