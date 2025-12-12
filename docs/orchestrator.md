# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete. An orchestrator spawns agents for ready tasks and loops until work is done.

This guide shows how to build orchestrator patterns that spawn multiple Claude Code agents in parallel, each working on a different task.

---

## Quick Start

```bash
# Basic orchestrator loop
while true; do
    TASK=$(scud next --tag myproject)
    if [ -z "$TASK" ]; then
        echo "No ready tasks. Exiting."
        break
    fi

    TASK_ID=$(echo "$TASK" | grep -o "ID: [^ ]*" | awk '{print $2}')
    echo "Starting task $TASK_ID"

    # Claim and work on task
    scud claim "$TASK_ID" --name "agent-$$"
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

### Get Multiple Ready Tasks

```bash
scud next-batch --tag myproject --count 5
```

Returns up to 5 ready tasks at once - useful for orchestrators.

### Claim a Task

```bash
scud claim <task-id> --name <agent-name>
```

Locks the task so other agents don't work on it.

### Release a Task

```bash
scud release <task-id>
```

Releases the lock (use after completing or abandoning).

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
    TASK=$(scud next --tag $TAG)

    if [ -z "$TASK" ]; then
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

    # Extract task ID and claim
    TASK_ID=$(echo "$TASK" | grep -o "ID: [^ ]*" | awk '{print $2}')
    scud claim "$TASK_ID" --name "agent-$$-$TASK_ID"
    scud set-status "$TASK_ID" in-progress

    echo "Starting task $TASK_ID"
    (
        claude "Implement task $TASK_ID. When done, run: scud set-status $TASK_ID done"
        scud release "$TASK_ID"
    ) &
done

wait
```

### Example 2: Claim-Based Orchestrator with Auto-Complete

Use task claiming with explicit completion:

```bash
#!/bin/bash
# parallel-claim.sh

MAX_PARALLEL=4
AGENT_NAME="orchestrator-$$"
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
    scud claim "$TASK_ID" --name "$AGENT_NAME-$TASK_ID"
    scud set-status "$TASK_ID" in-progress

    echo "Starting task $TASK_ID"
    (
        # Run claude and mark done when complete
        claude "Complete task $TASK_ID. When finished, the task will be marked done."
        scud set-status "$TASK_ID" done
        scud release "$TASK_ID"
    ) &
done

wait
echo "All spawned tasks complete"
```

### Example 3: Wave-Based Execution

Execute entire waves in parallel:

```bash
#!/bin/bash
# parallel-waves.sh

TAG="myproject"

while true; do
    # Get all ready tasks
    READY_TASKS=$(scud next-batch --tag $TAG --count 10 | grep "ID:" | awk '{print $2}')

    if [ -z "$READY_TASKS" ]; then
        echo "No more ready tasks"
        break
    fi

    echo "Starting wave with $(echo "$READY_TASKS" | wc -w) tasks..."

    # Spawn agents for all ready tasks
    for TASK_ID in $READY_TASKS; do
        scud claim "$TASK_ID" --name "wave-$$"
        scud set-status "$TASK_ID" in-progress
        echo "  - Starting task $TASK_ID"
        (
            claude "Implement task $TASK_ID"
            scud set-status "$TASK_ID" done
            scud release "$TASK_ID"
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
scud serve
```

Opens a web dashboard with visual task board and dependency graph.

### Session Dashboard

```bash
watch -n 2 "scud whois --tag myproject && echo && scud stats --tag myproject"
```

---

## Best Practices

### 1. Use Claiming for Coordination

Always claim tasks before working on them:

```bash
scud claim <task-id> --name <unique-name>
```

This prevents two agents from working on the same task.

### 2. Mark Tasks Complete Explicitly

After completing work:

```bash
scud set-status <task-id> done
scud release <task-id>
```

### 3. Monitor with `whois` and `doctor`

```bash
# See active work
scud whois --tag myproject

# Find stale locks
scud doctor --tag myproject --stale-hours 2
```

### 4. Clean Up Stale Locks

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
1. Agent didn't run `scud set-status <id> done`
2. Script crashed before completion command

**Fix:**
```bash
# Manually mark complete
scud set-status <id> done
scud release <id>
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
| `SCUD_TASK_ID` | Current task for context | `export SCUD_TASK_ID=5` |
| `XAI_API_KEY` | API key for AI commands | `export XAI_API_KEY=xai-...` |

---

## Advanced Patterns

### Conditional Spawning

Only spawn if task meets criteria:

```bash
while true; do
    TASK=$(scud next --tag myproject)
    [ -z "$TASK" ] && break

    TASK_ID=$(echo "$TASK" | grep -o "ID: [^ ]*" | awk '{print $2}')
    COMPLEXITY=$(scud show $TASK_ID | grep "Complexity:" | awk '{print $2}')

    # Only spawn for low-complexity tasks
    if [ "$COMPLEXITY" -lt 10 ]; then
        scud claim "$TASK_ID" --name "agent-$$"
        scud set-status "$TASK_ID" in-progress
        (
            claude "Implement task $TASK_ID"
            scud set-status "$TASK_ID" done
            scud release "$TASK_ID"
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
        TASK=$(scud next --tag $TAG)
        [ -z "$TASK" ] && break

        TASK_ID=$(echo "$TASK" | grep -o "ID: [^ ]*" | awk '{print $2}')
        scud claim "$TASK_ID" --name "multi-$$"
        scud set-status "$TASK_ID" in-progress
        (
            claude "Implement $TAG task $TASK_ID"
            scud set-status "$TASK_ID" done
            scud release "$TASK_ID"
        ) &
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
scud next --tag <tag>       # Find next ready task
scud next-batch --count N   # Get multiple ready tasks
scud claim <id> --name <n>  # Lock task
scud release <id>           # Unlock task
scud set-status <id> done   # Mark complete
scud whois --tag <tag>      # Show active work
scud doctor --tag <tag>     # Check for issues
scud waves --tag <tag>      # Show parallel waves
scud stats --tag <tag>      # Show progress
scud serve                  # Web dashboard
```

---

## See Also

- [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- [Parallel Features](features/PARALLEL_FEATURES.md) - Task locking details
