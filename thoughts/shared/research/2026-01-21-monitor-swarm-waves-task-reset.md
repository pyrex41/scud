---
date: 2026-01-21T22:07:08Z
researcher: Claude
git_commit: 25dad59b41f23bbf772d0e8e8c52f1d3f3416a1b
branch: master
repository: scud
topic: "Monitor/Swarm Wave Visibility and Task Reset Mechanisms"
tags: [research, codebase, monitor, swarm, spawn, ralph, waves, task-status]
status: complete
last_updated: 2026-01-21
last_updated_by: Claude
---

# Research: Monitor/Swarm Wave Visibility and Task Reset Mechanisms

**Date**: 2026-01-21T22:07:08Z
**Researcher**: Claude
**Git Commit**: 25dad59b41f23bbf772d0e8e8c52f1d3f3416a1b
**Branch**: master
**Repository**: scud

## Research Question

The monitor works better with spawn than swarm. How can we have better wave-by-wave visibility during swarm execution? The Ralph command doesn't make sense for refactors (which is what swarm does). How can we manually reset/restart tasks?

## Summary

The research reveals the following key findings:

1. **Monitor-Spawn Integration**: The monitor/TUI was designed for spawn sessions and tracks agents via `.scud/spawn/<session>.json` files. A bridge proxy (`create_and_update_spawn_proxy`) exists to make swarm sessions visible to the monitor, but it only adds tasks to the spawn proxy—it doesn't provide wave-level granularity.

2. **Wave Visibility Gap**: Swarm executes in waves with rounds, tracking state in `.scud/swarm/<session>.json`, but this rich wave/round structure is flattened when bridged to spawn format. The TUI computes waves independently using Kahn's algorithm on task dependencies rather than reading swarm's actual wave execution state.

3. **Ralph Mode Purpose**: Ralph is an autonomous retry loop for individual tasks—it keeps retrying a task until completion or max iterations (50). It's designed for "get it done" scenarios, not for coordinated refactors where swarm's wave-based execution with backpressure validation is more appropriate.

4. **Task Reset Mechanisms**: Tasks can be reset manually via `scud set-status <task_id> pending` or in bulk via `scud set-status --from in-progress --to pending`. The `scud doctor --fix` command auto-resets stale in-progress tasks.

## Detailed Findings

### Monitor Architecture

The monitor system consists of two main components:

**SpawnSession** (`scud-cli/src/commands/spawn/monitor.rs:34-41`):
- Tracks agents in a flat list structure
- Each agent has: `task_id`, `task_title`, `window_name`, `status`, `started_at`, `tag`
- Persisted to `.scud/spawn/<session-name>.json`
- Status values: `Starting`, `Running`, `Completed`, `Failed`

**TUI Application** (`scud-cli/src/commands/spawn/tui/app.rs`):
- Three-panel interface: agents list, waves view, live output
- Computes waves independently via `compute_waves()` using Kahn's algorithm
- Refreshes agent statuses every 2 seconds by checking tmux windows and SCUD task status
- Refreshes live output every 500ms via `tmux capture-pane`

### Swarm Execution Model

**SwarmSession** (`scud-cli/src/commands/swarm/session.rs:208-277`):
- Hierarchical structure: Session → Waves → Rounds → Tasks
- Each wave tracks: `wave_number`, `rounds`, `validation`, `summary`, `start_commit`, `review`, `repairs`
- Each round tracks: `round_number`, `task_ids`, `tags`, `failures`, timestamps
- Rich execution metadata not available in spawn format

**Wave Execution** (`scud-cli/src/commands/swarm/mod.rs:272-516`):
```
Main Loop:
1. Load fresh task state
2. Compute waves from dependency DAG (Kahn's algorithm)
3. Execute first wave in rounds (parallel batches)
4. Run backpressure validation (build/test/lint)
5. Handle review/repair if validation fails
6. Save session, increment wave number
7. Repeat until all tasks complete
```

**Swarm-to-Spawn Bridge** (`scud-cli/src/commands/swarm/mod.rs:551-600`):
- `create_and_update_spawn_proxy()` translates swarm sessions to spawn format
- Called after each round (incremental) and at final completion (full)
- **Limitation**: Flattens wave/round structure to a flat agent list
- Does not preserve wave boundaries or round groupings in spawn proxy

### Wave Visibility in TUI

The TUI computes waves independently, not from swarm's execution state:

**Wave Computation** (`scud-cli/src/commands/spawn/tui/app.rs:696-832`):
1. Filter actionable tasks (excludes Done, Expanded, Cancelled)
2. Build dependency graph with in-degree counts
3. Tasks blocked by in-progress dependencies get +1000 in-degree (effectively blocked)
4. Use Kahn's algorithm to group tasks into waves
5. Assign state per task: Ready, Running, Done, Blocked, InProgress

**WaveTask States** (`scud-cli/src/commands/spawn/tui/app.rs:42-55`):
- `Ready`: Dependencies met, task pending
- `Running`: Agent currently spawned
- `Done`: Task completed
- `Blocked`: Dependencies not satisfied
- `InProgress`: Task marked in-progress but no visible agent

### Ralph Mode

Ralph is an autonomous task completion loop, not a wave orchestrator:

**Purpose**: Keep retrying a single task until it completes or reaches max iterations (50)

**TUI Ralph Mode** (`scud-cli/src/commands/spawn/tui/app.rs:349-397`):
- Toggle via 'R' key
- Auto-spawns ready tasks up to `ralph_max_parallel` (default: 5)
- Checks every 5 seconds for newly ready tasks
- Spawns with `spawn_task_with_ralph()` which adds retry loop wrapper

**Ralph Loop Script** (`scud-cli/src/commands/spawn/terminal.rs:469-532`):
```bash
RALPH_MAX_ITER=50
while [ $RALPH_ITER -lt $RALPH_MAX_ITER ]; do
    # Run agent
    # Check if task status is "done"
    # Break if done or max iterations
    # Sleep 2 seconds
done
```

**Why Ralph Doesn't Fit Refactors**:
- Ralph is designed for autonomous "keep trying until done" scenarios
- No coordination between tasks—each Ralph loop is independent
- No backpressure validation between iterations
- Swarm provides wave-based execution with validation gates and repair loops

### Task Status System

**Status Values** (`scud-cli/src/models/task.rs:5-16`):
- `Pending`: Waiting to start (default)
- `InProgress`: Currently being worked on
- `Done`: Completed successfully
- `Review`: Awaiting review
- `Blocked`: Cannot proceed
- `Deferred`: Postponed
- `Cancelled`: Abandoned
- `Expanded`: Broken into subtasks
- `Failed`: Validation failed

**Set-Status Command** (`scud-cli/src/commands/set_status.rs`):

Three modes of operation:

1. **Single Task**: `scud set-status <task_id> <status>`
2. **Multi-Task**: `scud set-status <status> <task_id> [task_id...]`
3. **Bulk Transition**: `scud set-status --from <status> --to <status>`

No transition guards exist—any status can transition to any other status.

**Doctor Command Reset** (`scud-cli/src/commands/doctor.rs:226-259`):
- Detects stale in-progress tasks via `updated_at` timestamp
- Default threshold: tasks in-progress for more than X hours
- `scud doctor --fix` resets stale tasks to `Pending`
- Custom threshold: `scud doctor --fix --stale-hours 48`

### Restart Agent Function

The TUI has a "restart" function (`scud-cli/src/commands/spawn/tui/app.rs:547-594`) but it only:
1. Sends Ctrl+C to interrupt current process
2. Displays "Agent restarted by user" message

It does **not** actually restart the Claude agent—the tmux window process must be manually restarted.

## Code References

### Monitor/Spawn
- `scud-cli/src/commands/spawn/monitor.rs:34-41` - SpawnSession struct
- `scud-cli/src/commands/spawn/monitor.rs:22-30` - AgentState struct
- `scud-cli/src/commands/spawn/monitor.rs:124-133` - save_session()
- `scud-cli/src/commands/spawn/tui/app.rs:696-832` - Wave computation
- `scud-cli/src/commands/spawn/tui/app.rs:270-297` - Agent status refresh

### Swarm
- `scud-cli/src/commands/swarm/mod.rs:272-516` - Main execution loop
- `scud-cli/src/commands/swarm/mod.rs:551-600` - Swarm-to-spawn bridge
- `scud-cli/src/commands/swarm/mod.rs:622-732` - Wave computation (Kahn's algorithm)
- `scud-cli/src/commands/swarm/session.rs:208-277` - SwarmSession struct
- `scud-cli/src/commands/swarm/session.rs:141-205` - WaveState struct

### Ralph
- `scud-cli/src/commands/spawn/tui/app.rs:349-356` - toggle_ralph_mode()
- `scud-cli/src/commands/spawn/tui/app.rs:358-397` - ralph_auto_spawn()
- `scud-cli/src/commands/spawn/terminal.rs:469-532` - Ralph loop script generation

### Task Status
- `scud-cli/src/models/task.rs:5-16` - TaskStatus enum
- `scud-cli/src/models/task.rs:287-290` - set_status() method
- `scud-cli/src/commands/set_status.rs:56-123` - Bulk transition mode
- `scud-cli/src/commands/doctor.rs:226-259` - Auto-fix stale tasks

## Architecture Documentation

### Current Patterns

**Session File Locations**:
- Spawn sessions: `.scud/spawn/<session-name>.json`
- Swarm sessions: `.scud/swarm/<session-name>.json`
- Session locks: `.scud/swarm/<tag>.lock`

**Status Reconciliation** (TUI determines agent status by combining):
1. Tmux window existence (process running)
2. SCUD task status (logical completion)
3. Priority: Done → Completed; Blocked → Failed; InProgress + window → Running

**Wave Computation**: Both swarm and TUI use Kahn's algorithm independently:
- Swarm: `compute_waves_from_tasks()` in `swarm/mod.rs:622-732`
- TUI: `compute_waves()` in `tui/app.rs:696-832`

### Identified Gaps

1. **Wave Visibility**: Swarm's wave/round execution state is not visible in monitor/TUI
2. **No True Restart**: TUI restart only interrupts, doesn't re-spawn agents
3. **Bridge Flattening**: Swarm-to-spawn bridge loses wave structure

## Related Research

- `thoughts/shared/research/2026-01-20-swarm-monitor-bridge.md` - Swarm-to-spawn bridge architecture
- `thoughts/shared/research/2026-01-20-swarm-monitor-real-time.md` - Real-time monitor updates
- `thoughts/shared/research/2026-01-18-ralph-loop-backpressure-idempotency-analysis.md` - Ralph and backpressure
- `scud-cli/thoughts/shared/research/2026-01-17-swarm-vs-spawn-architecture.md` - Architecture comparison
- `thoughts/shared/plans/2026-01-20-monitor-swarm-integration.md` - Integration planning

## Open Questions

1. Should swarm expose its wave/round state to the TUI directly rather than through the flattened spawn proxy?
2. Would a dedicated swarm TUI view (showing waves/rounds/validation status) be more appropriate than adapting spawn TUI?
3. For task restart, should there be a command that:
   - Kills the tmux window
   - Resets task status to pending
   - Optionally re-spawns the agent
4. Should `scud set-status` gain a `--restart` flag that combines status reset with agent re-spawn?
