# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete. SCUD provides two approaches for parallel agent orchestration:

- **`scud spawn`** - Launch individual agents with TUI monitoring
- **`scud swarm`** - Automated wave or beads execution with SQLite event logging, salvo worktrees, and transcript capture

---

## Swarm Execution (Recommended)

The `scud swarm` command is SCUD's primary orchestration tool. It automatically:

1. Provisions an isolated git **salvo worktree** for the tag (unless `--no-worktree`)
2. Finds ready tasks from the DAG
3. Spawns Claude Code agents in tmux windows
4. Monitors agent progress with heartbeat detection
5. Logs all events to **SQLite** for queryable retrospectives
6. Captures Claude Code **transcripts** in real-time
7. Validates results between waves (wave mode)
8. Syncs task status changes back to main on completion

### Wave Mode (Default)

Batches ready tasks into waves, runs them in parallel, validates results, then proceeds to next wave.

```bash
# Run swarm with wave-based execution
scud swarm --tag myproject

# Customize wave size
scud swarm --tag myproject --round-size 5

# Skip validation between waves
scud swarm --tag myproject --no-validate

# Dry run - show execution plan without spawning
scud swarm --tag myproject --dry-run
```

**Wave lifecycle:**
1. Compute wave from DAG (tasks whose dependencies are all done)
2. Spawn agents for each task in the wave
3. Monitor progress with live display (heartbeat, completion polling)
4. Wait for all agents in the wave to complete
5. Run validation commands (build, test, lint) if configured
6. If validation fails, enter repair loop (re-run failed tasks)
7. Advance to next wave
8. Repeat until all tasks complete

### Beads Mode

Continuous polling for ready tasks. Spawns agents immediately when dependencies complete---no waiting for batch boundaries. Inspired by the [Beads project](https://github.com/steveyegge/beads).

```bash
# Run swarm with continuous polling
scud swarm --tag myproject --swarm-mode beads

# Limit concurrent agents
scud swarm --tag myproject --swarm-mode beads --round-size 3
```

**When to use which:**
- **Wave mode**: When you need validation between batches, structured checkpoints, or repair loops
- **Beads mode**: When you have many small tasks and want fluid execution without artificial boundaries

### Swarm Options

| Option | Description |
|--------|-------------|
| `--tag <tag>` | Run tasks from specific tag |
| `--all-tags` | Run tasks from all tags |
| `--swarm-mode <mode>` | `wave` (default) or `beads` |
| `--round-size <n>` | Max concurrent agents (default: 5) |
| `--no-validate` | Skip backpressure validation between waves |
| `--harness <type>` | Terminal: `tmux`, `claude-code` |
| `--no-worktree` | Skip automatic salvo worktree creation (run in-place) |
| `--salvo-dir <path>` | Custom directory for salvo worktree |
| `--stale-timeout <secs>` | Seconds before an unresponsive agent is considered stale (default: 300) |
| `--dry-run` | Show execution plan without spawning agents |

---

## Salvo Worktrees

When `scud swarm --tag <tag>` is invoked, SCUD automatically provisions an isolated git worktree for that tag. This allows multiple swarms on different tags to run in parallel without file conflicts.

### How It Works

1. SCUD checks if a worktree already exists for the tag (via SQLite lookup)
2. If not, creates a git worktree at `../<project-name>.salvo.<tag>/` on branch `salvo/<tag>`
3. Generates a **filtered task file**: full detail for the target tag, collapsed stubs for other tags
4. Copies `.scud/config.toml`, guidance files, and sets the active tag
5. Runs the swarm inside the worktree directory
6. On swarm completion, auto-syncs task status changes back to main

### Convention Path

Given a project at `/home/user/myproject` with tag `backend`:
- Worktree created at `/home/user/myproject.salvo.backend/`
- Git branch: `salvo/backend`
- Override with `--salvo-dir /custom/path`

### Filtered Task Files

The worktree gets a focused view of tasks:

```
# Target tag: full detail with all tasks, edges, metadata
@meta { name "Backend API" }
@nodes
api:1 | Create user endpoint | P | 5 | H
api:2 | Add authentication | P | 8 | H
@edges
api:2 -> api:1

---

# SCUD Graph v1
# Phase: frontend
# [Collapsed - 12 tasks, work in main branch]
@meta { name "Frontend UI" }
@nodes
# Tasks hidden. Run `scud salvo sync` to merge changes.
```

Agents working in the worktree only see tasks relevant to their tag, reducing noise and preventing accidental edits to other tags' tasks.

### Parallel Swarms

Run multiple tags simultaneously in separate terminals:

```bash
# Terminal 1
scud swarm --tag backend
# Creates ../myproject.salvo.backend/

# Terminal 2
scud swarm --tag frontend
# Creates ../myproject.salvo.frontend/

# No conflicts - each runs in its own worktree
```

### Worktree Reuse

On subsequent runs, existing worktrees are reused:

```bash
# First run - creates worktree
$ scud swarm --tag backend
Created salvo worktree for 'backend' at ../myproject.salvo.backend/

# Second run - reuses existing worktree
$ scud swarm --tag backend
Using existing salvo worktree at ../myproject.salvo.backend/
```

The filtered task file is refreshed with the latest state from main on each reuse, while preserving any in-progress status changes from the worktree.

### Opt-Out

```bash
# Skip worktree creation, run in current directory (previous behavior)
scud swarm --tag backend --no-worktree
```

### Salvo Management Commands

```bash
# List all salvo worktrees with paths, branches, and sync status
scud salvo list

# Manually sync worktree task status back to main
scud salvo sync <tag>

# Remove worktree, git branch, and database record
scud salvo remove <tag>
```

---

## SQLite Event Storage

All swarm events are stored in a SQLite database at `.scud/scud.db` with WAL mode enabled for concurrent access. This replaces the previous JSONL event files.

### Database Schema

The database contains 9 tables:

| Table | Purpose |
|-------|---------|
| `sessions` | Swarm session records (tag, mode, timing) |
| `agent_runs` | Per-task execution attempts (wave, round, model, duration) |
| `events` | Lifecycle events (spawn, complete, fail, tool calls, file ops) |
| `transcript_messages` | Claude Code conversation messages |
| `tool_calls` | Tool invocations extracted from transcripts |
| `tool_results` | Tool results extracted from transcripts |
| `validation_runs` | Post-wave validation results |
| `validation_commands` | Individual validation command results |
| `salvo_worktrees` | Active salvo worktree registry |

### Event Kinds

Events capture the full agent lifecycle:

| Event | Description |
|-------|-------------|
| `spawned` | Agent process created |
| `started` | Agent began working |
| `completed` | Agent finished (success/failure + duration) |
| `failed` | Agent failed with reason |
| `tool_call` | Tool invocation (tool name + input summary) |
| `tool_result` | Tool result (tool name + success + duration) |
| `file_read` | File read operation |
| `file_write` | File write operation (+ lines changed) |
| `dependency_met` | Task dependency satisfied |
| `unblocked` | Task unblocked by another task's completion |
| `output` | Agent output line |
| `wave_started` | Wave began (wave number + task count) |
| `wave_completed` | Wave finished (wave number + duration) |
| `validation_passed` | All validation commands passed |
| `validation_failed` | One or more validation commands failed |
| `repair_started` | Repair loop began (attempt number + task IDs) |
| `repair_completed` | Repair loop finished (attempt + success) |

### Querying Events Directly

```bash
# Recent events
sqlite3 .scud/scud.db "SELECT timestamp, task_id, kind FROM events ORDER BY timestamp DESC LIMIT 20"

# Events for a specific session
sqlite3 .scud/scud.db "SELECT kind, COUNT(*) FROM events WHERE session_id = 'myproject-wave-20260126' GROUP BY kind"

# Task durations
sqlite3 .scud/scud.db "SELECT task_id, duration_ms/1000.0 as secs FROM events WHERE kind = 'completed' ORDER BY duration_ms DESC"

# File write activity
sqlite3 .scud/scud.db "SELECT file_path, COUNT(*) as writes FROM events WHERE kind = 'file_write' GROUP BY file_path ORDER BY writes DESC LIMIT 10"
```

### Retrospective Analysis

```bash
# List available sessions
scud swarm retro

# View session timeline
scud swarm retro <session-id>
```

The retrospective shows a chronological timeline of all events, with per-task summaries including duration, tool calls, and file operations.

---

## Transcript Capture

SCUD automatically imports Claude Code conversation transcripts during swarm execution. Transcripts are parsed from `~/.claude/projects/<project>/*.jsonl` and stored in SQLite.

### Automatic Capture During Swarm

When a swarm starts, a background file watcher (using FSEvents on macOS) monitors the Claude project directory for new transcript files. As agents write their conversation logs, SCUD imports them in real-time.

### Transcript Commands

```bash
# Search across all transcript content
scud transcript search "authentication"
scud transcript search "error"

# Show aggregate statistics
scud transcript stats
# Output:
#   Sessions: 118
#   Messages: 11,879
#   Tool calls: 3,735

# List recent transcript sessions with message counts
scud transcript list

# View a specific session transcript
scud transcript view --session <session-id>

# Bulk import all project transcripts (useful for initial setup)
scud transcript import
```

### Querying Transcripts Directly

```bash
# Count messages per role
sqlite3 .scud/scud.db "SELECT role, COUNT(*) FROM transcript_messages GROUP BY role"

# Most-used tools
sqlite3 .scud/scud.db "SELECT tool_name, COUNT(*) as uses FROM tool_calls GROUP BY tool_name ORDER BY uses DESC LIMIT 10"

# Total tokens used (from sessions with structured content)
sqlite3 .scud/scud.db "SELECT SUM(input_tokens) as input, SUM(output_tokens) as output FROM transcript_messages WHERE input_tokens IS NOT NULL"

# Find sessions where a specific file was read
sqlite3 .scud/scud.db "SELECT DISTINCT claude_session_id FROM tool_calls WHERE tool_name = 'Read' AND input_json LIKE '%auth%'"
```

---

## Live Progress Monitoring

Wave mode provides live progress display during execution:

### Heartbeat Detection

SCUD monitors each agent for activity. If an agent stops producing output for longer than the stale timeout, it is flagged as potentially orphaned.

```bash
# Set stale timeout to 10 minutes (default: 5 minutes)
scud swarm --tag myproject --stale-timeout 600
```

### Orphan Detection

If an agent's tmux window disappears (crashed, killed, or detached), SCUD detects the orphaned task and reports it. Orphaned tasks can be:
- Re-queued for the next wave
- Manually investigated via `scud transcript view`
- Marked as failed with `scud set-status <id> blocked`

### Progress Display

During wave execution, the console shows:
- Current wave number and total tasks
- Per-agent status (running, completed, failed)
- Elapsed time per task
- Validation results between waves

---

## Batch Repair

When backpressure validation fails after a wave, SCUD spawns a single "batch repair"
agent instead of one agent per failing task. This agent:

- Receives context about ALL responsible tasks at once
- Can analyze related failures together
- Iterates internally to fix issues systematically
- Signals completion via `.scud/batch-repair-complete` marker file

This approach is more efficient than spawning N agents for N failing tasks, since the batch
repair agent can see the full picture and fix root causes that may affect multiple tasks.

---

## Agent Health Checking

During wave execution, SCUD monitors agent health to prevent tasks from being stuck as `InProgress`:

### Orphan Detection

Every 30 seconds, SCUD checks if each running task's tmux window still exists. If a window
disappears (agent crashed, killed, or tmux session terminated), the task is marked `Failed`.

### Idle Detection

When an agent has been idle for too long (configurable, default 5 minutes), SCUD checks if
the tmux pane shows a shell prompt (indicating the agent process has exited). If both
conditions are true:
- Agent has been idle beyond the threshold
- Pane shows a shell prompt (not an active process)

The task is automatically marked `Failed` with an appropriate event emitted.

Configure the idle timeout:
```bash
scud swarm --tag myproject --idle-timeout-minutes 10
```

### Stale Timeout

The optional `--stale-timeout-minutes` flag provides an additional safeguard. Tasks that
exceed the timeout with no tmux window are reset to `Pending`, allowing them to be
retried in a subsequent wave.

```bash
scud swarm --tag myproject --stale-timeout-minutes 30
```

### How This Prevents Stuck Tasks

Without these health checks, tasks can remain `InProgress` indefinitely when:
- An agent process crashes but leaves the tmux window open
- A tmux session is terminated unexpectedly
- An agent hangs and stops producing output

The combination of orphan detection, idle detection, and stale timeout ensures that
tasks eventually get marked as `Failed` and can be retried or investigated.

---

## Quick Start: scud spawn

The simplest way to run parallel agents (without swarm orchestration):

```bash
# Spawn agents for ready tasks with TUI monitor
scud spawn -m --limit 5

# Spawn for a specific tag
scud spawn --tag myproject -m --limit 3

# Spawn across all tags
scud spawn --all-tags -m --limit 10
```

The `-m` flag opens a TUI monitor where you can:
- Watch agents work in real-time (live terminal output)
- Switch between agents with `j/k` keys
- Enter fullscreen mode with `Enter`
- Send input to agents with `i`
- Stop/restart agents with `x`

### What happens when you spawn:

1. SCUD finds ready tasks (pending with dependencies met)
2. Installs Claude Code hooks for auto-completion (first run only)
3. Spawns terminal windows with Claude Code agents
4. Each agent receives task context and works autonomously
5. When agents finish, hooks automatically mark tasks as done

---

## Spawn Command Reference

```bash
scud spawn [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--tag <tag>` | Spawn tasks from specific tag (default: active tag) |
| `--all-tags` | Spawn tasks from all tags |
| `--limit <n>` | Maximum number of agents to spawn (default: 5) |
| `-m, --monitor` | Open TUI monitor after spawning |
| `-c, --claim` | Mark spawned tasks as in-progress |
| `--terminal <type>` | Terminal: auto, tmux, kitty, wezterm, iterm2 |
| `--session <name>` | Custom session name |
| `--attach` | Attach to tmux session after spawn |
| `--dry-run` | Show what would be spawned without spawning |

### Examples

```bash
# Preview what will be spawned
scud spawn --dry-run

# Spawn 3 agents for auth tasks
scud spawn --tag auth --limit 3 -m

# Spawn in kitty terminal
scud spawn --terminal kitty --limit 2

# Spawn and claim tasks (mark as in-progress)
scud spawn --claim --limit 3 -m

# Name the session for later attachment
scud spawn --session my-feature -m
```

---

## TUI Monitor

The TUI provides a split-view interface for monitoring agents:

```
+-----------------------------------------------------------------+
| scud-myproject    . Starting 0  . Running 3  . Done 2  . Failed 0|
+----------------------+------------------------------------------+
|  Agents              |  Live Output                             |
|                      |                                          |
| > * 1: Setup auth    |  Exploring codebase...                   |
|   * 2: Add login     |  Found auth module at src/auth/          |
|   v 3: Create models |  Implementing JWT token validation...    |
|                      |                                          |
+----------------------+------------------------------------------+
| j/k Navigate  .  Enter Fullscreen  .  i Input  .  x Stop  .  q  |
+-----------------------------------------------------------------+
```

### Key Bindings

| Key | Split View | Fullscreen |
|-----|------------|------------|
| `j/k` | Switch agents | Switch agents |
| Up/Down | Switch agents | Scroll output |
| `Enter` | Fullscreen | Exit fullscreen |
| `i` | Input mode | Input mode |
| `x` | Stop agent | Stop agent |
| `g/G` | Top/Bottom | Top/Bottom |
| `?` | Help | Help |
| `q` | Quit | Quit |

### Monitor an existing session

```bash
# List sessions
scud monitor

# Monitor specific session
scud monitor --session scud-myproject
```

---

## Hook Integration

SCUD automatically installs Claude Code hooks for bulletproof task completion:

**What the hooks do:**
- Set `SCUD_TASK_ID` environment variable for each agent
- When Claude finishes (Stop event), hook runs `scud set-status <id> done`
- Tasks complete even if agent forgets to run the command

**Hook location:** `.claude/settings.local.json`

```json
{
  "hooks": {
    "Stop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "bash -c 'if [ -n \"$SCUD_TASK_ID\" ]; then scud set-status \"$SCUD_TASK_ID\" done; fi'"
      }]
    }]
  }
}
```

---

## Data Storage

### SQLite Database

All orchestration data is stored in `.scud/scud.db`:

```
.scud/scud.db
├── sessions          # Swarm session records
├── agent_runs        # Per-task execution attempts
├── events            # Lifecycle events (18 kinds)
├── transcript_messages # Claude Code conversation logs
├── tool_calls        # Tool invocations from transcripts
├── tool_results      # Tool results from transcripts
├── validation_runs   # Post-wave validation results
├── validation_commands # Individual validation command results
├── salvo_worktrees   # Active worktree registry
└── schema_version    # For future migrations
```

**Configuration:** WAL mode is enabled for concurrent reads during swarm execution. The database is created on first use and schema is auto-initialized.

### Salvo Worktree Layout

```
../<project>.salvo.<tag>/
├── .git              # Git worktree link (file, not directory)
├── .scud/
│   ├── tasks/tasks.scg   # Filtered: full detail for tag, stubs for others
│   ├── config.toml       # Copied from main
│   ├── active-tag        # Set to target tag
│   ├── guidance/         # Copied from main
│   └── swarm/            # Lock files (worktree-scoped)
└── <project files>       # Full checkout on salvo/<tag> branch
```

---

## Monitoring Progress

### From the TUI

The TUI header shows real-time status counts:
- Gray dot: Starting/Waiting
- Green dot: Running
- Blue dot: Done
- Red dot: Failed

### From the command line

```bash
# Overall stats
scud stats --tag myproject

# Who's working on what
scud whois --tag myproject

# Parallel execution waves
scud waves --tag myproject

# Transcript statistics
scud transcript stats

# Search agent conversations
scud transcript search "error handling"

# List salvo worktrees
scud salvo list
```

---

## Advanced: Manual Orchestration

For custom orchestration patterns, you can build your own loops using these commands:

### Get Next Ready Task

```bash
scud next --tag myproject
```

### Get Multiple Ready Tasks

```bash
scud next-batch --tag myproject --limit 5
```

### Start Work on a Task

```bash
scud set-status <task-id> in-progress
# ... do work ...
scud set-status <task-id> done
```

### Example: Custom Bash Orchestrator

```bash
#!/bin/bash
# custom-orchestrator.sh

MAX_PARALLEL=4
TAG="myproject"

while true; do
    TASK=$(scud next --tag $TAG)

    if [ -z "$TASK" ]; then
        if [ $(jobs -r | wc -l) -eq 0 ]; then
            echo "All tasks complete"
            break
        fi
        sleep 5
        continue
    fi

    while [ $(jobs -r | wc -l) -ge $MAX_PARALLEL ]; do
        sleep 2
    done

    TASK_ID=$(echo "$TASK" | grep -o "ID: [^ ]*" | awk '{print $2}')

    (
        export SCUD_TASK_ID=$TASK_ID
        claude "Implement task $TASK_ID"
    ) &
done

wait
```

---

## Troubleshooting

### Agents not completing tasks

**Symptom:** Task stays "in-progress" after agent finishes

**Cause:** Hooks not installed or `SCUD_TASK_ID` not set

**Fix:**
```bash
# Check if hooks are installed
cat .claude/settings.local.json | grep SCUD_TASK_ID

# Manually mark complete
scud set-status <task-id> done
```

### No tasks ready to spawn

**Symptom:** `scud spawn` says "No ready tasks"

**Causes:**
1. All tasks are done
2. Dependencies not satisfied
3. Tasks are blocked

**Fix:**
```bash
scud stats --tag myproject   # Check completion
scud waves --tag myproject   # Check dependencies
scud list --status blocked   # Check blocked tasks
```

### Stale agents detected

**Symptom:** Swarm reports agents as stale or orphaned

**Causes:**
1. Agent crashed or was killed
2. tmux window closed
3. Agent hung on a long operation

**Fix:**
```bash
# Check transcript for the agent's last activity
scud transcript search "<task-id>"

# Increase stale timeout for long-running tasks
scud swarm --tag myproject --stale-timeout 900

# Manually mark task for retry
scud set-status <task-id> pending
```

### Salvo worktree issues

**Symptom:** Worktree creation fails

**Causes:**
1. Git branch `salvo/<tag>` already exists from a previous run
2. Target directory already exists
3. Uncommitted changes in main

**Fix:**
```bash
# Remove stale worktree record
scud salvo remove <tag>

# Clean up orphaned git branch
git branch -D salvo/<tag>

# Re-run swarm
scud swarm --tag <tag>
```

### TUI shows wrong status

**Fix:** Press `r` to force refresh, or restart the monitor.

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `SCUD_TASK_ID` | Set by spawn for hook integration |

---

## Best Practices

### Swarm Execution

1. **Start with dry-run** - Preview the execution plan with `--dry-run` before spawning agents
2. **Use default worktrees** - Let SCUD create salvo worktrees for tag isolation; use `--no-worktree` only for quick single-tag runs
3. **Set appropriate stale timeout** - Increase `--stale-timeout` for tags with complex, long-running tasks
4. **Import transcripts** - Run `scud transcript import` to seed the database with historical conversations
5. **Check retrospectives** - Use `scud swarm retro` after each swarm to review performance and identify issues
6. **Clean up worktrees** - Remove finished salvo worktrees with `scud salvo remove` to save disk space

### Spawn Monitoring

1. **Use spawn with monitor** - `scud spawn -m` gives you visibility
2. **Start small** - Begin with `--limit 2-3` agents
3. **Use dry-run first** - Preview with `--dry-run`
4. **Check waves** - Understand dependencies with `scud waves`
5. **Monitor progress** - Watch the TUI or run `scud stats`

---

## Reference

### Key Commands

```bash
# Swarm orchestration
scud swarm --tag <tag>              # Run swarm with auto-worktree
scud swarm --tag <tag> --dry-run    # Preview execution plan
scud swarm --no-worktree            # Run in-place
scud swarm retro [session]          # View session retrospective

# Salvo worktree management
scud salvo list                     # List all worktrees
scud salvo sync <tag>               # Sync status back to main
scud salvo remove <tag>             # Clean up worktree

# Transcript analysis
scud transcript search <query>      # Search conversations
scud transcript stats               # Aggregate statistics
scud transcript list                # List sessions
scud transcript import              # Bulk import

# Spawn (individual agents)
scud spawn -m --limit 5             # Spawn with monitor
scud monitor                        # Monitor existing session

# Progress
scud stats --tag <tag>              # Show progress
scud waves --tag <tag>              # Show parallel waves
scud whois --tag <tag>              # Show active work
scud doctor --tag <tag>             # Check for issues
```

---

## See Also

- [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- [Parallel Features](features/PARALLEL_FEATURES.md) - Task locking details
