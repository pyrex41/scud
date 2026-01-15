# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete. The `scud spawn` command launches parallel Claude Code agents to work on ready tasks automatically.

---

## Quick Start: scud spawn

The simplest way to run parallel agents:

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
┌─────────────────────────────────────────────────────────────────┐
│ scud-myproject    ◉ Starting 0  ◉ Running 3  ◉ Done 2  ◉ Failed 0│
├──────────────────────┬──────────────────────────────────────────┤
│  Agents              │  Live Output                             │
│                      │                                          │
│ ▸ ● 1: Setup auth    │  Exploring codebase...                   │
│   ● 2: Add login     │  Found auth module at src/auth/          │
│   ✓ 3: Create models │  Implementing JWT token validation...    │
│                      │                                          │
├──────────────────────┴──────────────────────────────────────────┤
│ j/k Navigate  ·  Enter Fullscreen  ·  i Input  ·  x Stop  ·  q  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Bindings

| Key | Split View | Fullscreen |
|-----|------------|------------|
| `j/k` | Switch agents | Switch agents |
| `↑/↓` | Switch agents | Scroll output |
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

### TUI shows wrong status

**Fix:** Press `r` to force refresh, or restart the monitor.

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `SCUD_TASK_ID` | Set by spawn for hook integration |

---

## Best Practices

1. **Use spawn with monitor** - `scud spawn -m` gives you visibility
2. **Start small** - Begin with `--limit 2-3` agents
3. **Use dry-run first** - Preview with `--dry-run`
4. **Check waves** - Understand dependencies with `scud waves`
5. **Monitor progress** - Watch the TUI or run `scud stats`

---

## Reference

### Key Commands

```bash
scud spawn -m --limit 5     # Spawn with monitor
scud monitor                # Monitor existing session
scud stats --tag <tag>      # Show progress
scud waves --tag <tag>      # Show parallel waves
scud whois --tag <tag>      # Show active work
scud doctor --tag <tag>     # Check for issues
```

---

## See Also

- [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- [Parallel Features](features/PARALLEL_FEATURES.md) - Task locking details
