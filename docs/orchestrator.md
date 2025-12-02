# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete. An orchestrator spawns agents for ready tasks and loops until work is done.

This guide shows how to build orchestrator patterns that spawn multiple Claude Code agents in parallel, each working on a different task.

---

## Quick Start

```bash
# 1. Install hooks first (critical!)
scud hooks install

# 2. Basic orchestrator loop
while true; do
    TASK=$(scud next --tag myproject)
    if [ -z "$TASK" ]; then
        echo "No ready tasks. Exiting."
        break
    fi

    TASK_ID=$(echo "$TASK" | grep -o "Task [0-9]*" | awk '{print $2}')
    echo "Starting task $TASK_ID"

    SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
done

wait
echo "All tasks complete"
```

---

## How Hooks Ensure Completion

The orchestrator pattern relies on Claude Code hooks to automatically mark tasks complete:

1. **Install hooks** - Run `scud hooks install` to create `.claude/settings.local.json`
2. **Set task ID** - Pass `SCUD_TASK_ID=<id>` as environment variable to Claude session
3. **Work happens** - Agent implements the task
4. **Session ends** - Claude Code fires the Stop hook
5. **Hook marks complete** - Hook calls `scud _hook-complete` internally
6. **Task updated** - If `SCUD_TASK_ID` is set, that task is marked Done and unlocked

This prevents the ~15% of cases where agents forget to mark tasks complete.

---

## Commands for Orchestration

### Get Next Ready Task

```bash
scud next --tag myproject
```

Output (when task is ready):
```
Task 3 is ready (depends on: task-1, task-2)
Title: Implement user authentication
Dependencies: task-1 (done), task-2 (done)
Complexity: 8
```

Output (when no tasks ready):
```
No tasks ready. Check dependencies or all tasks may be complete.
```

### Monitor Active Sessions

```bash
scud whois --tag myproject
```

Output:
```
Active task assignments:

Task 3: alice (claimed 5m ago)
Task 4: bob (claimed 2m ago)
Task 5: charlie (claimed 1m ago)
```

### Check for Stale Locks

```bash
scud doctor --tag myproject
```

Output:
```
Checking tasks in tag: myproject

Issues found:
  - Task 7: Stale lock (claimed by alice 25h ago)

Run with --fix to auto-release stale locks
```

Fix stale locks:
```bash
scud doctor --tag myproject --fix
```

### View Parallel Waves

```bash
scud waves --tag myproject
```

Output:
```
Wave 1 (3 tasks):
  - Task 1: Setup database schema
  - Task 2: Create API endpoints
  - Task 3: Design UI mockups

Wave 2 (2 tasks):
  - Task 4: Implement auth middleware (depends on: task-1, task-2)
  - Task 5: Build login form (depends on: task-3)

Wave 3 (1 task):
  - Task 6: Integration tests (depends on: task-4, task-5)
```

---

## Parallel Spawning Examples

### Example 1: Simple Parallel Loop

Spawn up to 4 parallel agents:

```bash
#!/bin/bash
# parallel-simple.sh

scud hooks install

MAX_PARALLEL=4
ACTIVE=0

while true; do
    # Find ready tasks
    TASK=$(scud next --tag myproject)

    if [ -z "$TASK" ]; then
        if [ $ACTIVE -eq 0 ]; then
            echo "All tasks complete"
            break
        else
            echo "Waiting for active tasks..."
            sleep 5
            continue
        fi
    fi

    # Wait if at max parallel
    while [ $ACTIVE -ge $MAX_PARALLEL ]; do
        sleep 2
        # Count active jobs
        ACTIVE=$(jobs -r | wc -l)
    done

    # Spawn agent
    TASK_ID=$(echo "$TASK" | grep -o "Task [0-9]*" | awk '{print $2}')
    echo "Starting task $TASK_ID (active: $ACTIVE)"

    SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
    ACTIVE=$((ACTIVE + 1))
done

wait
```

### Example 2: Claim-Based Orchestrator

Use task claiming for more control:

```bash
#!/bin/bash
# parallel-claim.sh

scud hooks install

MAX_PARALLEL=4
AGENT_NAME="orchestrator-$$"

while true; do
    # Get next ready task
    TASK=$(scud next --tag myproject)

    if [ -z "$TASK" ]; then
        # Check if any jobs still running
        if [ $(jobs -r | wc -l) -eq 0 ]; then
            echo "All tasks complete"
            break
        fi
        sleep 5
        continue
    fi

    # Wait if at capacity
    while [ $(jobs -r | wc -l) -ge $MAX_PARALLEL ]; do
        sleep 2
    done

    # Extract task ID
    TASK_ID=$(echo "$TASK" | grep -o "Task [0-9]*" | awk '{print $2}')

    # Claim task
    scud claim "$TASK_ID" --name "$AGENT_NAME-$TASK_ID" --tag myproject

    # Spawn agent with task ID
    echo "Starting task $TASK_ID"
    SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
done

wait
echo "All spawned tasks complete"
```

### Example 3: Wave-Based Execution

Execute entire waves in parallel:

```bash
#!/bin/bash
# parallel-waves.sh

scud hooks install

TAG="myproject"

# Get wave count
WAVES=$(scud waves --tag $TAG | grep "^Wave" | wc -l)

for WAVE in $(seq 1 $WAVES); do
    echo "Starting Wave $WAVE..."

    # Get all ready tasks
    READY_TASKS=$(scud list --tag $TAG --status pending | grep "^Task" | awk '{print $2}')

    if [ -z "$READY_TASKS" ]; then
        echo "No more ready tasks"
        break
    fi

    # Spawn agents for all ready tasks
    for TASK_ID in $READY_TASKS; do
        echo "  - Starting task $TASK_ID"
        SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
    done

    # Wait for wave to complete
    wait
    echo "Wave $WAVE complete"
done

echo "All waves complete"
```

---

## Monitoring Progress

### Real-time Statistics

```bash
watch -n 2 "scud stats --tag myproject"
```

Output:
```
Every 2.0s: scud stats --tag myproject

Tag: myproject
Total tasks: 10
  Pending: 2
  In Progress: 3
  Done: 5

Completion: 50% (5/10)
```

### Session Dashboard

```bash
watch -n 2 "scud whois --tag myproject && echo && scud stats --tag myproject"
```

---

## Best Practices

### 1. Always Install Hooks First

```bash
scud hooks install
```

Without hooks, agents may forget to mark tasks complete, breaking the DAG.

### 2. Set SCUD_TASK_ID Environment Variable

```bash
SCUD_TASK_ID=5 claude "Implement task 5"
```

This tells the hook which task to mark complete when the session ends.

### 3. Use Claiming for Team Coordination

If multiple orchestrators run simultaneously:

```bash
scud claim <task-id> --name <unique-name>
```

This prevents two agents from working on the same task.

### 4. Monitor with `whois` and `doctor`

```bash
# See active work
scud whois --tag myproject

# Find stale locks
scud doctor --tag myproject --stale-hours 2
```

### 5. Clean Up Stale Locks

If an agent crashes without releasing:

```bash
scud doctor --tag myproject --fix
```

Or manually:

```bash
scud release <task-id> --force
```

---

## Troubleshooting

### Task Not Marked Complete

**Symptom:** Agent finishes but task stays "in-progress"

**Causes:**
1. Hooks not installed (`scud hooks install`)
2. `SCUD_TASK_ID` not set when spawning agent
3. Hook file corrupted (check `.claude/settings.local.json`)

**Fix:**
```bash
# Check hook status
scud hooks status

# Reinstall if needed
scud hooks install

# Manually mark complete
scud set-status <id> done
```

### No Tasks Ready

**Symptom:** `scud next` says "No tasks ready" but tasks exist

**Causes:**
1. All ready tasks are claimed/locked
2. Dependencies not satisfied
3. All tasks complete

**Fix:**
```bash
# Check for claimed tasks
scud whois --tag myproject

# Check for stale locks
scud doctor --tag myproject

# View dependency graph
scud waves --tag myproject

# Check completion
scud stats --tag myproject
```

### Stale Locks

**Symptom:** Tasks locked by crashed agents

**Fix:**
```bash
# Find stale locks (older than 2 hours)
scud doctor --tag myproject --stale-hours 2

# Auto-fix
scud doctor --tag myproject --stale-hours 2 --fix

# Manual release
scud release <task-id> --force
```

---

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `SCUD_TASK_ID` | Task ID for hook completion | `SCUD_TASK_ID=5 claude "work"` |
| `ANTHROPIC_API_KEY` | API key for AI commands | `export ANTHROPIC_API_KEY=sk-ant-...` |

---

## Advanced Patterns

### Conditional Spawning

Only spawn if task meets criteria:

```bash
while true; do
    TASK=$(scud next --tag myproject)
    [ -z "$TASK" ] && break

    TASK_ID=$(echo "$TASK" | grep -o "Task [0-9]*" | awk '{print $2}')
    COMPLEXITY=$(scud show $TASK_ID | grep "Complexity:" | awk '{print $2}')

    # Only spawn for low-complexity tasks
    if [ "$COMPLEXITY" -lt 10 ]; then
        SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
    else
        echo "Skipping high-complexity task $TASK_ID"
    fi
done
wait
```

### Priority-Based Execution

Execute high-priority tasks first:

```bash
# Get all ready tasks sorted by complexity (low to high)
TASKS=$(scud list --tag myproject --status pending | sort -k3 -n)

for TASK_LINE in $TASKS; do
    TASK_ID=$(echo "$TASK_LINE" | awk '{print $2}')
    SCUD_TASK_ID=$TASK_ID claude "Implement task $TASK_ID" &
done

wait
```

### Multi-Tag Orchestrator

Work across multiple tags:

```bash
TAGS=("auth" "api" "ui")

for TAG in "${TAGS[@]}"; do
    echo "Processing tag: $TAG"

    while true; do
        TASK=$(scud next --tag $TAG)
        [ -z "$TASK" ] && break

        TASK_ID=$(echo "$TASK" | grep -o "Task [0-9]*" | awk '{print $2}')
        SCUD_TASK_ID=$TASK_ID claude "Implement $TAG task $TASK_ID" &
    done
done

wait
```

---

## Performance Tips

1. **Limit parallelism** - More than 4-5 agents can cause rate limiting
2. **Use waves** - Execute entire dependency levels at once
3. **Monitor actively** - Use `watch` with `stats` and `whois`
4. **Clean stale locks** - Run `doctor` periodically
5. **Profile complexity** - Skip or defer high-complexity tasks

---

## Reference

### Key Commands

```bash
scud hooks install          # Enable automatic completion
scud next --tag <tag>       # Find next ready task
scud claim <id> --name <n>  # Lock task
scud release <id>           # Unlock task
scud whois --tag <tag>      # Show active work
scud doctor --tag <tag>     # Check for issues
scud waves --tag <tag>      # Show parallel waves
scud stats --tag <tag>      # Show progress
```

### Hook Mechanism

1. Hooks installed via `scud hooks install`
2. Creates `.claude/settings.local.json` with Stop hook
3. Hook calls `scud _hook-complete` on every Claude session end
4. If `SCUD_TASK_ID` env var is set, that task is marked Done
5. Task lock is automatically released

---

## See Also

- [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- [Parallel Features](features/PARALLEL_FEATURES.md) - Task locking details
- [Complete Guide](guides/COMPLETE_GUIDE.md) - Full documentation
