# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete. An orchestrator spawns agents for ready tasks and loops until work is done.

This guide shows how to build orchestrator patterns that spawn multiple Claude Code agents in parallel, each working on a different task.

---

## Quick Start

```bash
# Basic orchestrator loop
while true; do
    TASK=$(scud next --spawn --tag myproject)
    if [ -z "$TASK" ] || [ "$TASK" = "null" ]; then
        echo "No ready tasks. Exiting."
        break
    fi

    TASK_ID=$(echo "$TASK" | jq -r '.id')
    echo "Starting task $TASK_ID"

    scud set-status "$TASK_ID" in-progress

    claude "Implement task $TASK_ID then mark it done with: scud set-status $TASK_ID done" &
done

wait
echo "All tasks complete"
```

---

## Commands for Orchestration

### Get Next Ready Task

```bash
scud next --tag myproject
```

Output (when task is ready):
```
Next ready task:
  ID: 3
  Title: Implement user authentication
  Complexity: 8
  Dependencies: 1 (done), 2 (done)
```

Output (when no tasks ready):
```
No tasks ready. Check dependencies or all tasks may be complete.
```

### Get Next Task as JSON (for scripts)

```bash
scud next --spawn --tag myproject
```

Returns JSON for easy parsing in scripts:
```json
{"id":"3","title":"Implement user authentication","complexity":8}
```

### Get Multiple Ready Tasks

```bash
scud next-batch --tag myproject --limit 5
```

Returns up to 5 ready tasks at once as JSON - useful for orchestrators.

### Assign a Task

```bash
scud assign <task-id> <name>
```

Records who is working on a task for visibility.

### Monitor Active Work

```bash
scud who-is --tag myproject
```

Output:
```
Active task assignments:

alice:
  [auth] auth:3 - Implement JWT tokens

bob:
  [api] api:2 - Create REST endpoints
```

### Check for Stale Tasks

```bash
scud doctor --tag myproject
```

Output:
```
Checking tasks in tag: myproject

Issues found:
  - Task 7: In-progress for 25h (stale)

Run with --fix to auto-reset stale tasks
```

Fix stale tasks:
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
  - Task 4: Implement auth middleware (depends on: 1, 2)
  - Task 5: Build login form (depends on: 3)

Wave 3 (1 task):
  - Task 6: Integration tests (depends on: 4, 5)
```

---

## Parallel Spawning Examples

### Example 1: Simple Parallel Loop

Spawn up to 4 parallel agents:

```bash
#!/bin/bash
# parallel-simple.sh

MAX_PARALLEL=4
TAG="myproject"

while true; do
    # Find ready tasks
    TASK=$(scud next --spawn --tag $TAG)

    if [ -z "$TASK" ] || [ "$TASK" = "null" ]; then
        if [ $(jobs -r | wc -l) -eq 0 ]; then
            echo "All tasks complete"
            break
        else
            echo "Waiting for active tasks..."
            sleep 5
            continue
        fi
    fi

    # Wait if at max parallel
    while [ $(jobs -r | wc -l) -ge $MAX_PARALLEL ]; do
        sleep 2
    done

    # Extract task ID and start work
    TASK_ID=$(echo "$TASK" | jq -r '.id')
    scud assign "$TASK_ID" "agent-$$-$TASK_ID"
    scud set-status "$TASK_ID" in-progress

    echo "Starting task $TASK_ID"
    (
        claude "Implement task $TASK_ID. When done, run: scud set-status $TASK_ID done"
    ) &
done

wait
```

### Example 2: Wave-Based Execution

Execute entire waves in parallel:

```bash
#!/bin/bash
# parallel-waves.sh

TAG="myproject"

while true; do
    # Get all ready tasks
    READY_TASKS=$(scud next-batch --tag $TAG --limit 10 | jq -r '.[].id')

    if [ -z "$READY_TASKS" ]; then
        echo "No more ready tasks"
        break
    fi

    echo "Starting wave with $(echo "$READY_TASKS" | wc -w) tasks..."

    # Spawn agents for all ready tasks
    for TASK_ID in $READY_TASKS; do
        scud assign "$TASK_ID" "wave-$$"
        scud set-status "$TASK_ID" in-progress
        echo "  - Starting task $TASK_ID"
        (
            claude "Implement task $TASK_ID"
            scud set-status "$TASK_ID" done
        ) &
    done

    # Wait for wave to complete
    wait
    echo "Wave complete"
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

### Web Dashboard

```bash
scud view
```

Opens a web dashboard with visual task board and dependency graph.

### Session Dashboard

```bash
watch -n 2 "scud who-is --tag myproject && echo && scud stats --tag myproject"
```

---

## Best Practices

### 1. Use Assignment for Coordination

Track who is working on what:

```bash
scud assign <task-id> <agent-name>
```

This helps with visibility but doesn't block other agents.

### 2. Mark Tasks Complete Explicitly

After completing work:

```bash
scud set-status <task-id> done
```

### 3. Monitor with `who-is` and `doctor`

```bash
# See active work
scud who-is --tag myproject

# Find stale tasks
scud doctor --tag myproject --stale-hours 2
```

### 4. Clean Up Stale Tasks

If a task is stuck in-progress:

```bash
scud doctor --tag myproject --fix
```

Or manually reset:

```bash
scud set-status <task-id> pending
```

---

## Troubleshooting

### Task Not Marked Complete

**Symptom:** Agent finishes but task stays "in-progress"

**Causes:**
1. Agent didn't run `scud set-status <id> done`
2. Script crashed before completion command

**Fix:**
```bash
# Manually mark complete
scud set-status <id> done
```

### No Tasks Ready

**Symptom:** `scud next` says "No tasks ready" but tasks exist

**Causes:**
1. Dependencies not satisfied
2. All tasks complete
3. Tasks are blocked or deferred

**Fix:**
```bash
# View dependency graph
scud waves --tag myproject

# Check completion
scud stats --tag myproject

# Check for issues
scud doctor --tag myproject
```

### Stale In-Progress Tasks

**Symptom:** Tasks stuck in-progress after agent crash

**Fix:**
```bash
# Find stale tasks (older than 2 hours)
scud doctor --tag myproject --stale-hours 2

# Auto-fix (reset to pending)
scud doctor --tag myproject --stale-hours 2 --fix

# Manual reset
scud set-status <task-id> pending
```

---

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `SCUD_TASK_ID` | Current task for context | `export SCUD_TASK_ID=5` |
| `XAI_API_KEY` | API key for AI commands | `export XAI_API_KEY=xai-...` |

---

## Advanced Patterns

### Conditional Spawning

Only spawn if task meets criteria:

```bash
while true; do
    TASK=$(scud next --spawn --tag myproject)
    [ -z "$TASK" ] || [ "$TASK" = "null" ] && break

    TASK_ID=$(echo "$TASK" | jq -r '.id')
    COMPLEXITY=$(echo "$TASK" | jq -r '.complexity')

    # Only spawn for low-complexity tasks
    if [ "$COMPLEXITY" -lt 10 ]; then
        scud assign "$TASK_ID" "agent-$$"
        scud set-status "$TASK_ID" in-progress
        (
            claude "Implement task $TASK_ID"
            scud set-status "$TASK_ID" done
        ) &
    else
        echo "Skipping high-complexity task $TASK_ID"
    fi
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
        TASK=$(scud next --spawn --tag $TAG)
        [ -z "$TASK" ] || [ "$TASK" = "null" ] && break

        TASK_ID=$(echo "$TASK" | jq -r '.id')
        scud assign "$TASK_ID" "multi-$$"
        scud set-status "$TASK_ID" in-progress
        (
            claude "Implement $TAG task $TASK_ID"
            scud set-status "$TASK_ID" done
        ) &
    done
done

wait
```

---

## Performance Tips

1. **Limit parallelism** - More than 4-5 agents can cause rate limiting
2. **Use waves** - Execute entire dependency levels at once
3. **Monitor actively** - Use `watch` with `stats` and `who-is`
4. **Clean stale tasks** - Run `doctor` periodically
5. **Profile complexity** - Skip or defer high-complexity tasks

---

## Reference

### Key Commands

```bash
scud next --tag <tag>         # Find next ready task
scud next --spawn --tag <tag> # Get next task as JSON
scud next-batch --limit N     # Get multiple ready tasks
scud assign <id> <name>       # Track who's working on task
scud set-status <id> done     # Mark complete
scud who-is --tag <tag>       # Show assignments
scud doctor --tag <tag>       # Check for issues
scud waves --tag <tag>        # Show parallel waves
scud stats --tag <tag>        # Show progress
scud view                     # Web dashboard
```

---

## See Also

- [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
