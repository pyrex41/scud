---
date: 2026-01-29T10:00:00-08:00
researcher: Claude
git_commit: edcaaeb43e2e78c889e2be018c37c607116e861d
branch: master
repository: scud
topic: "How to use descartes-gui for monitoring SCUD swarms"
tags: [research, descartes-gui, swarm, monitoring, orchestration]
status: complete
last_updated: 2026-01-29
last_updated_by: Claude
---

# Research: How to Use descartes-gui for Monitoring SCUD Swarms

**Date**: 2026-01-29
**Researcher**: Claude
**Git Commit**: edcaaeb43e2e78c889e2be018c37c607116e861d
**Branch**: master
**Repository**: scud

## Research Question

How do I use descartes-gui to monitor a running SCUD swarm?

## Summary

**descartes-gui** is a standalone desktop GUI application (built with Iced) that provides visibility and control over SCUD task execution. It can:

1. Display tasks organized by parallel execution waves
2. Start individual tasks or full swarms
3. Pause/Resume/Cancel running agents
4. Show live streaming output from agents

**Key insight**: descartes-gui does NOT connect to a running swarm directly. Instead, it:
- Reads task state from `.scud/tasks/tasks.scg` via scud-core library calls
- Starts NEW swarm execution via `scud swarm` subprocess
- Monitors swarm progress through JSON event streaming (`--json-events` flag)

## How to Use descartes-gui

### Building and Running

```bash
# From the scud repository root
cd descartes-gui

# Build release binary
cargo build --release

# Run the GUI
./target/release/descartes-gui
# Or simply:
cargo run --release
```

The binary exists at: `/Users/reuben/projects/scud/descartes-gui/target/release/descartes-gui`

### Prerequisites

1. **SCUD configured**: Run `scud warmup` to ensure tasks are loaded
2. **Environment variables**:
   - `ANTHROPIC_API_KEY` - For Claude models
   - `XAI_API_KEY` - For xAI models (optional)
   - `RUST_LOG=descartes_gui=debug` - For verbose logging

### GUI Layout

```
┌─────────────────────────────────────────────────────────────┐
│                      Descartes GUI                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                     │
│  │  Waves  │  │ Agents  │  │ Output  │  ← View Tabs        │
│  └─────────┘  └─────────┘  └─────────┘                     │
├─────────────────────────────────────────────────────────────┤
│  Wave 1:  [Task A] [Start]  [Task B] [Start]               │
│  Wave 2:  [Task C] [Start]  (depends on A, B)              │
│  ...                                                        │
│                                                             │
│  [Refresh]                                                  │
├─────────────────────────────────────────────────────────────┤
│  Status: Idle/Running/Paused │ Current: Task X             │
│  [Pause] [Resume] [Cancel] [Start Swarm]                   │
└─────────────────────────────────────────────────────────────┘
```

### Three Views

1. **Waves View** - Shows tasks grouped by execution wave (based on DAG dependencies)
   - Click **Start** on any task to run it individually
   - Click **Refresh** to reload task state from disk

2. **Agents View** - Swarm control panel
   - **Start Swarm** button - Launches `scud swarm --tag <active-tag> --json-events`
   - **Pause/Resume/Cancel** buttons for controlling running swarm
   - Shows current harness and round size settings

3. **Output View** - Live streaming output from agents
   - Shows task output, wave progress, validation results
   - **Clear** button to reset output buffer

### Workflow: Monitoring an Existing Swarm

**Important limitation**: descartes-gui cannot attach to an already-running swarm started from the terminal. The GUI starts its own swarm subprocess and monitors it through stdout.

To monitor a swarm with descartes-gui:

1. **Don't** start the swarm from terminal first
2. Open descartes-gui
3. Select your tag (if needed)
4. Click **Start Swarm** in the Agents view
5. Watch progress in the Output view

### Workflow: Starting a New Swarm

1. Run `descartes-gui`
2. Go to **Waves** view to see available tasks
3. Go to **Agents** view
4. Click **Start Swarm**
5. Monitor progress in **Output** view
6. Use **Pause**/**Resume**/**Cancel** as needed

### Configuration

descartes-gui reads swarm defaults from `.scud/config.toml`:
- Default harness (terminal type)
- Default round size (concurrent agents)
- Default tag

These can be set in your SCUD config:
```toml
[swarm]
harness = "tmux"
round_size = 5
```

## Alternative: Terminal-Based Monitoring

For monitoring swarms started from the terminal, use these tools instead:

### TUI Monitor (scud spawn -m)

```bash
# Spawn agents with built-in TUI
scud spawn -m --limit 5 --tag myproject

# Or monitor an existing session
scud monitor --session scud-myproject
```

### Command-Line Monitoring

```bash
# Watch progress
scud stats --tag myproject

# See parallel waves
scud waves --tag myproject

# View retrospective after completion
scud swarm retro
```

### SQLite Queries

```bash
# Recent events
sqlite3 .scud/scud.db "SELECT timestamp, task_id, kind FROM events ORDER BY timestamp DESC LIMIT 20"

# Session timeline
scud swarm retro <session-id>
```

## Architecture Details

### ScudBridge (descartes-gui/src/scud_bridge.rs)

The bridge provides two communication channels:
- **Command channel**: GUI → Bridge (start swarm, pause, resume, stop)
- **Event channel**: Bridge → GUI (task updates, output, completion)

Task operations use **direct scud-core library calls**:
- `LoadTasks` - Reads from Storage
- `ComputeWaves` - Uses `scud_core::compute_waves()`
- `CompleteTask` / `BlockTask` - Updates task status

Swarm operations use **subprocess spawning**:
- `StartSwarm` - Spawns `scud swarm --tag <tag> --harness <harness> --json-events`
- JSON events are parsed from stdout and converted to ScudEvents

### Event Types

The GUI receives these events from the swarm:
- `SwarmStarted { tag, total_waves }`
- `WaveStarted { wave, tasks }`
- `TaskStarted { task_id }`
- `TaskOutput { task_id, text }`
- `TaskCompleted { task_id, success }`
- `ValidationStarted` / `ValidationCompleted`
- `WaveCompleted { wave }`
- `SwarmCompleted { success }`

## Code References

- Entry point: `descartes-gui/src/main.rs:36-46`
- ScudBridge: `descartes-gui/src/scud_bridge.rs:192-278`
- Swarm execution: `descartes-gui/src/scud_bridge.rs:419-491`
- Views: `descartes-gui/src/views/waves.rs`, `agents.rs`, `output.rs`

## Related Documentation

- `/Users/reuben/projects/scud/docs/orchestrator.md` - Full orchestration documentation
- `/Users/reuben/projects/scud/descartes-gui/README.md` - GUI-specific readme

## Open Questions

1. **Attach to existing swarm**: Currently not possible - would require the swarm to write events to a file/socket that the GUI could read
2. **Multiple swarms**: The GUI tracks a single swarm subprocess; running multiple tags requires multiple GUI instances or using terminal-based tools
