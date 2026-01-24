# Continuous Worker Model: Beads-Inspired Task Distribution

## Date: 2026-01-24

## Problem Statement

The current swarm implementation uses **wave-based execution**:
1. Compute all tasks with in-degree = 0 (no pending dependencies)
2. Execute entire wave in parallel
3. **Wait for ALL tasks in wave to complete**
4. Compute next wave
5. Repeat

This creates bottlenecks where fast workers sit idle waiting for the slowest task in the wave.

### Current Bottlenecks (from swarm/mod.rs analysis)

1. **Wave Completion Waiting** (lines 313-328): When a wave finishes but some tasks still in-progress, orchestration waits 10 seconds before rechecking

2. **Round Completion Polling** (lines 1147-1180): Polling every 5 seconds means up to 5-second latency before detecting task completion

3. **Sequential Wave Execution**: Must finish wave N before starting wave N+1, even if some wave N+1 tasks have no dependencies on slow wave N tasks

## Desired State: Continuous Worker Model

Inspired by Steve Yegge's Beads and Gastown:

### The GUPP Principle
**"If there is work on your Hook, YOU MUST RUN IT."**

Workers continuously check for ready work and immediately execute. No waiting for wave completion.

### How Beads Does It

```
Agent Loop:
1. bd ready              # Query for ready tasks (deps satisfied)
2. Select highest priority task
3. Update status to in-progress
4. Execute work
5. Close issue upon completion
6. GOTO 1 (immediately)
```

The `bd ready` command returns tasks where ALL dependencies are complete, regardless of which "wave" they conceptually belong to.

## Proposed SCUD Implementation

### 1. New Command: `scud ready`

Returns all tasks that are ready to execute (pending + dependencies met):

```bash
$ scud ready --tag myproject --limit 5 --json
{
  "tasks": [
    {"id": "1", "title": "Setup database schema", "priority": "high"},
    {"id": "3", "title": "Create user model", "priority": "medium"},
    {"id": "7", "title": "Add logging", "priority": "low"}
  ]
}
```

Key differences from `scud next`:
- Returns **multiple** tasks (configurable limit)
- Supports JSON output for orchestrators
- Can filter by priority, complexity
- Does NOT claim/lock - just queries

### 2. Continuous Worker Loop

Instead of wave-based orchestration:

```bash
#!/bin/bash
# Continuous worker loop

while true; do
    # Query for ready work
    TASK=$(scud ready --tag myproject --limit 1 --json | jq -r '.tasks[0] // empty')

    if [ -z "$TASK" ]; then
        # No work available - check if any in-progress
        IN_PROGRESS=$(scud list --tag myproject --status in-progress --count)
        if [ "$IN_PROGRESS" -eq 0 ]; then
            echo "All work complete!"
            break
        fi
        # Work exists but blocked - sleep briefly
        sleep 2
        continue
    fi

    TASK_ID=$(echo $TASK | jq -r .id)

    # Claim and execute immediately
    scud set-status $TASK_ID in-progress --tag myproject

    # Spawn agent (or execute inline)
    SCUD_TASK_ID=$TASK_ID claude "Implement task: $(scud show $TASK_ID --tag myproject)"

    # Agent completion handled by hooks
done
```

### 3. Swarm Refactor: `swarm --continuous`

New execution mode that replaces wave batching:

```rust
// Pseudocode for continuous mode
async fn run_swarm_continuous(config: SwarmConfig) -> Result<()> {
    let max_concurrent = config.concurrency.unwrap_or(5);
    let active_tasks: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    loop {
        // Load current task state
        let storage = Storage::new()?;
        let all_phases = storage.load_tasks()?;

        // Get ready tasks (deps met, not in-progress, not locked)
        let ready = get_ready_tasks(&all_phases, &config.tags);

        // How many slots available?
        let active_count = active_tasks.lock().unwrap().len();
        let available_slots = max_concurrent - active_count;

        if available_slots == 0 {
            // At capacity - wait for completion signal
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        if ready.is_empty() && active_count == 0 {
            // No work and nothing in progress - done!
            break;
        }

        if ready.is_empty() {
            // Blocked - wait for in-progress to complete
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        // Spawn workers for ready tasks (up to available slots)
        for task in ready.iter().take(available_slots) {
            let task_id = task.id.clone();
            let active = active_tasks.clone();

            // Mark as in-progress
            storage.update_task_status(&task.tag, &task_id, TaskStatus::InProgress)?;

            // Add to active set
            active.lock().unwrap().insert(task_id.clone());

            // Spawn worker (tmux/subprocess)
            tokio::spawn(async move {
                execute_task(&task_id).await;
                // Remove from active when done
                active.lock().unwrap().remove(&task_id);
            });
        }
    }

    Ok(())
}

fn get_ready_tasks(phases: &HashMap<String, Phase>, tags: &[String]) -> Vec<TaskInfo> {
    let mut ready = Vec::new();

    // Flatten all tasks for cross-tag dependency checking
    let all_tasks: Vec<&Task> = phases.values()
        .flat_map(|p| &p.tasks)
        .collect();

    for tag in tags {
        if let Some(phase) = phases.get(tag) {
            for task in &phase.tasks {
                if task.status == TaskStatus::Pending
                    && !task.is_expanded()
                    && task.has_dependencies_met_refs(&all_tasks)
                    && !task.is_locked()
                {
                    ready.push(TaskInfo { task: task.clone(), tag: tag.clone() });
                }
            }
        }
    }

    // Sort by priority, then complexity
    ready.sort_by(|a, b| {
        a.task.priority.cmp(&b.task.priority)
            .then(a.task.complexity.cmp(&b.task.complexity))
    });

    ready
}
```

### 4. Event-Driven Completion Detection

Replace 5-second polling with event-driven completion:

**Option A: File Watch (inotify/fsevents)**
- Watch `.scud/tasks/tasks.scg` for changes
- When file changes, immediately check for newly completed tasks
- Much lower latency than polling

**Option B: Completion Signal File**
- Workers write to `.scud/completed/<task-id>` on completion
- Orchestrator watches directory for new files
- Immediate notification

**Option C: Unix Socket / Named Pipe**
- Workers send completion message to orchestrator
- Zero latency notification
- More complex but fastest

### 5. Worker Self-Orchestration

For maximum efficiency, workers can self-orchestrate:

```bash
# Worker startup script (injected by spawner)
while true; do
    # Check if there's more work
    NEXT=$(scud ready --tag $SCUD_TAG --limit 1 --json | jq -r '.tasks[0] // empty')

    if [ -z "$NEXT" ]; then
        exit 0  # No more work for this worker
    fi

    TASK_ID=$(echo $NEXT | jq -r .id)

    # Claim the task
    scud set-status $TASK_ID in-progress --tag $SCUD_TAG

    # Execute via Claude
    SCUD_TASK_ID=$TASK_ID claude "Work on task $TASK_ID"

    # Loop continues - grab next task
done
```

This eliminates orchestrator round-trip entirely. Workers keep working until exhaustion.

## Comparison: Wave vs Continuous

| Aspect | Wave-Based (Current) | Continuous (Proposed) |
|--------|---------------------|----------------------|
| Worker idle time | High (wait for slowest) | Minimal (immediate next) |
| Latency to start new task | 5-10 seconds | < 1 second |
| Coordination overhead | High (wave computation) | Low (simple ready query) |
| Complexity | Medium | Lower |
| DAG ordering guarantee | Yes (by wave) | Yes (by dependency check) |
| Backpressure handling | Per-wave | Per-task or continuous |

## Implementation Plan

### Phase 1: `scud ready` Command
- [ ] Add `ready` command to CLI
- [ ] Support `--limit`, `--json`, `--tag` flags
- [ ] Query tasks with deps met, not locked, pending status
- [ ] Sort by priority then complexity

### Phase 2: Continuous Swarm Mode
- [ ] Add `--continuous` flag to swarm
- [ ] Implement continuous loop instead of wave batching
- [ ] Use async task spawning with semaphore for concurrency
- [ ] Replace polling with shorter interval or event-driven

### Phase 3: Worker Self-Loop
- [ ] Modify spawn harness to include self-loop
- [ ] Workers query for next task on completion
- [ ] Workers exit only when no work available
- [ ] Handle validation/backpressure in self-loop

### Phase 4: Event-Driven Completion
- [ ] Implement file-watch for task state changes
- [ ] Or: completion signal files
- [ ] Reduce latency from 5s to < 100ms

## Open Questions

1. **Validation/Backpressure**: Current swarm validates after each wave. In continuous mode, when do we validate? Options:
   - After N tasks complete
   - After specific "checkpoint" tasks
   - Continuously in background
   - Per-task (expensive)

2. **Repair Loop**: If validation fails, how do we pause new task spawning while repairs happen?

3. **Review Phase**: Current swarm has optional review after waves. How does this work in continuous mode?

4. **Concurrency Limits**: How do we handle per-tag vs global concurrency limits?

## References

- [Beads GitHub](https://github.com/steveyegge/beads) - `bd ready` command
- [Gastown GitHub](https://github.com/steveyegge/gastown) - GUPP principle
- Current swarm: `scud-cli/src/commands/swarm/mod.rs`
- Previous beads refactor: `thoughts/shared/plans/2025-12-01-scud-v2-beads-inspired-refactor.md`
