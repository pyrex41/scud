---
date: 2026-01-18T21:45:54+00:00
researcher: Claude
git_commit: 7c65b5ad7ac2662cb87b0f39bb09192b8f7a5e93
branch: claude/test-monitor-feature-OJEwO
repository: scud
topic: "Monitor Feature Status - Does it still work with harness and swarm?"
tags: [research, codebase, monitor, spawn, harness, swarm, tui, casey]
status: complete
last_updated: 2026-01-18
last_updated_by: Claude
---

# Research: Monitor Feature Status

**Date**: 2026-01-18T21:45:54+00:00
**Researcher**: Claude
**Git Commit**: 7c65b5ad7ac2662cb87b0f39bb09192b8f7a5e93
**Branch**: claude/test-monitor-feature-OJEwO
**Repository**: scud

## Research Question

Investigate the monitor feature added by Casey. Does it still work with the new harness and swarm command? Look at implementation, tests, and any integration points.

## Summary

**The monitor feature still works and is well-integrated with the harness system.** All 33 related tests pass. However, the **swarm command does NOT integrate with monitor** - it has its own separate session system.

Casey Manos added the monitor feature in two key commits:
1. `2c5c12c` - "Fix parent task spawning and add agents panel scrolling" (Jan 7, 2026)
2. `1e76595` - "Add Ralph mode for autonomous wave execution" (Jan 7, 2026)

## Detailed Findings

### Casey Manos' Contributions

Casey made 5 commits to the codebase, all focused on the spawn/monitor TUI:

| Commit | Description |
|--------|-------------|
| `63e67b2` | fix(reanalyze-deps): correctly parse local ID from namespaced task IDs |
| `2c5c12c` | Fix parent task spawning and add agents panel scrolling |
| `1e76595` | Add Ralph mode for autonomous wave execution |
| `9b523a4` | Fix agents panel scrolling to use actual panel height |
| `ecfa1e6` | Fix Ralph prompt to be explicit about task ID |

### Monitor Feature Architecture

The monitor is a three-panel TUI (Terminal User Interface) for real-time agent tracking:

```
┌─────────────────────────────────────────┐
│ Header: Session name, status indicators │
├─────────────────────────────────────────┤
│ Waves Panel (35%) - Task execution      │
│   Wave 1: [x] task-1, [x] task-2        │
│   Wave 2: [ ] task-3                    │
├─────────────────────────────────────────┤
│ Agents Panel (25%) - Running agents     │
│   ● task-1: Building auth module        │
│   ◐ task-2: Starting...                 │
├─────────────────────────────────────────┤
│ Output Panel (40%) - Terminal output    │
│   (live tmux pane capture)              │
└─────────────────────────────────────────┘
```

**Entry Points:**
- `scud monitor [--session <name>]` - Standalone monitor command
- `scud spawn --monitor` or `scud spawn -m` - Spawn with monitor attached

### Core Implementation Files

| File | Purpose |
|------|---------|
| `scud-cli/src/commands/spawn/monitor.rs` | Data structures (AgentStatus, SpawnSession), persistence |
| `scud-cli/src/commands/spawn/mod.rs` | Spawn command with `run_monitor()` at line 332 |
| `scud-cli/src/commands/spawn/tui/app.rs` | TUI application state and logic |
| `scud-cli/src/commands/spawn/tui/ui.rs` | TUI rendering (three panels) |
| `scud-cli/src/commands/spawn/tui/mod.rs` | TUI event loop |
| `scud-cli/src/commands/spawn/terminal.rs` | Harness integration, tmux spawning |

### Harness Integration - WORKS

The monitor fully integrates with the harness system:

**Harness enum** (`terminal.rs:13-19`):
- `Claude` - Claude Code CLI
- `OpenCode` - OpenCode CLI

**Binary discovery** (`terminal.rs:72-158`):
- Caches binary paths using `OnceLock`
- Checks `which`, then falls back to common paths (`/opt/homebrew/bin`, `/usr/local/bin`, `$HOME/.local/bin`)

**Terminal spawning** (`terminal.rs:212-324`):
- Creates tmux windows with pattern `task-{task_id}`
- Writes prompts to `/tmp/scud-prompt-{task_id}.txt`
- Sets `SCUD_TASK_ID` environment variable for hooks
- Supports both Claude and OpenCode harnesses

**Ralph mode spawning** (`terminal.rs:326-542`):
- Uses pattern `ralph-{task_id}` for window names
- Creates bash loop script for autonomous execution
- Passes `RALPH_PROMISE` and `RALPH_MAX_ITER` environment variables

### Swarm Integration - DOES NOT EXIST

**Swarm has its own separate session system** and does NOT use the monitor feature:

| Aspect | Spawn Monitor | Swarm |
|--------|--------------|-------|
| Storage | `.scud/spawn/{session}.json` | `.scud/swarm/{session}.json` |
| Data model | Flat agent list | Hierarchical waves/rounds/tasks |
| Locking | None | Exclusive file lock per tag |
| TUI | Yes (`--monitor` flag) | No |
| Use case | Fire-and-forget parallel | Sequential waves with validation |

Swarm DOES use some spawn infrastructure:
- `spawn::agent` - For prompt generation
- `spawn::hooks` - For installing Claude Code hooks
- `spawn::terminal` - For spawning tmux windows

But swarm does NOT import or use:
- `spawn::monitor`
- `spawn::tui`

### Test Status - ALL PASSING

**Monitor unit tests** (4 tests):
```
test commands::spawn::monitor::tests::test_add_agent ... ok
test commands::spawn::monitor::tests::test_spawn_session_new ... ok
test commands::spawn::monitor::tests::test_spawn_stats ... ok
test commands::spawn::monitor::tests::test_update_agent_status ... ok
```

**Spawn tests** (20 tests):
```
test commands::spawn::tests::test_is_task_ready_basic ... ok
test commands::spawn::tests::test_is_task_ready_blocked_by_deps ... ok
test commands::spawn::tests::test_is_task_ready_in_progress ... ok
test commands::spawn::hooks::tests::test_hooks_installed_detects_our_hook ... ok
test commands::spawn::hooks::tests::test_install_hooks_creates_settings ... ok
test commands::spawn::hooks::tests::test_uninstall_hooks ... ok
(+ 14 more)
```

**E2E/User story tests** (9 tests):
```
test user_stories::multi_agent::test_us15_spawn_creates_sessions_for_ready_tasks ... ok
test user_stories::multi_agent::test_us15_spawn_different_terminal_types ... ok
test user_stories::multi_agent::test_us15_spawn_respects_limit ... ok
test user_stories::multi_agent::test_us17_spawn_claim_marks_tasks_in_progress ... ok
test user_stories::multi_agent::test_us17_spawn_only_pending_tasks ... ok
(+ 4 more)
```

### Key Features

**Ralph Mode** (added by Casey in `1e76595`):
- Toggle with `R` key in TUI
- Auto-spawns ready tasks up to max parallel (5)
- Each agent runs in bash loop checking task completion
- Up to 50 iterations per task
- Uses completion promise detection

**Agent Status Tracking**:
- `Starting` - Agent process initializing
- `Running` - Actively executing
- `Completed` - Finished successfully
- `Failed` - Encountered error

**Session Persistence**:
- JSON files in `.scud/spawn/`
- Tracks: session_name, tag, terminal type, working_dir, agents
- Survives CLI restarts

## Code References

- `scud-cli/src/commands/spawn/monitor.rs:12-19` - AgentStatus enum
- `scud-cli/src/commands/spawn/monitor.rs:33-42` - SpawnSession struct
- `scud-cli/src/commands/spawn/mod.rs:332-356` - run_monitor() function
- `scud-cli/src/commands/spawn/terminal.rs:13-19` - Harness enum
- `scud-cli/src/commands/spawn/terminal.rs:212-231` - spawn_terminal_with_harness_and_model()
- `scud-cli/src/commands/spawn/terminal.rs:326-542` - Ralph loop spawning
- `scud-cli/src/commands/spawn/tui/app.rs:341-397` - Ralph auto-spawn logic
- `scud-cli/src/commands/swarm/session.rs:208-277` - SwarmSession (separate system)

## Architecture Documentation

### Data Flow

```
┌─────────────┐
│ scud spawn  │
│  --monitor  │
└──────┬──────┘
       │ creates
       v
┌─────────────────────┐
│   SpawnSession      │───────> .scud/spawn/{session}.json
│   + agents list     │
└──────┬──────────────┘
       │ spawns
       v
┌─────────────────────┐
│   terminal.rs       │
│   - find harness    │
│   - spawn tmux      │
│   - set SCUD_TASK_ID│
└──────┬──────────────┘
       │ launches
       v
┌─────────────────────┐
│   TUI Monitor       │
│   - polls tmux      │
│   - captures output │
│   - updates status  │
└─────────────────────┘
```

### Refresh Cycles

- Session/status refresh: every 2 seconds
- Live output refresh: every 500ms
- Ralph check: every 5 seconds

## Related Research

- `thoughts/shared/research/2026-01-17-terminal-multiplexer-detached-sessions.md` - Terminal session research
- `scud-cli/thoughts/shared/research/2026-01-17-swarm-vs-spawn-architecture.md` - Architecture comparison

## Open Questions

1. **Should swarm integrate with monitor?** Currently swarm has synchronous wave-by-wave execution which doesn't benefit from a monitor TUI, but future async swarm could use it.

2. **Test coverage for TUI?** The user story tests (`US-16: Spawn Session Monitoring`) are marked but not all implemented in the test file.

3. **Ralph mode reliability?** The 50-iteration limit and completion promise detection may need tuning based on real-world usage.
