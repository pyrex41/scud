---
date: 2026-01-18T16:10:38+0000
researcher: reuben
git_commit: 91e49b9
branch: master
repository: pyrex41/scud
topic: "Ralph Loop Pattern Analysis: Backpressure, Idempotency, and Durability Comparison"
tags: [research, ralph-loop, backpressure, idempotency, durable-execution, scud, descartes]
status: complete
last_updated: 2026-01-18
last_updated_by: reuben
---

# Research: Ralph Loop Pattern Analysis - Backpressure, Idempotency, and Durability

**Date**: 2026-01-18T16:10:38+0000
**Researcher**: reuben
**Git Commit**: 91e49b9
**Branch**: master
**Repository**: pyrex41/scud

## Research Question

Compare the SCUD/Descartes implementation against the Ralph Wiggum loop and durable execution concepts from ghuntley.com/loop, ghuntley.com/ralph, and hatchet.run/blog/durable-execution. Focus on:
1. Backpressure implementation and formalization
2. Idempotency and mid-execution recovery
3. Whether SQLite or similar durable storage would add value

## Summary

The SCUD system implements a sophisticated version of the Ralph Wiggum loop pattern with several notable enhancements over the pure "bash while loop" approach. The implementation includes:

- **Backpressure**: Formalized as a validation/repair loop with git-based attribution
- **Fresh Context Per Task**: Each task gets isolated context, preventing accumulation
- **DAG-Driven Execution**: Kahn's algorithm for dependency ordering
- **Wave-Based Execution**: Parallel execution within dependency waves

However, several gaps exist compared to true durable execution patterns:
- **No true idempotency**: Duplicate execution is possible in race conditions
- **Limited resumability**: Crashed sessions leave orphaned in-progress tasks requiring manual cleanup
- **File-based state**: No atomic multi-file transactions, potential for corruption on interrupt

## Detailed Findings

### 1. The Ralph Wiggum Loop Pattern

#### Blog Concept (ghuntley.com/ralph)

The pure Ralph pattern is:
```bash
while :; do cat PROMPT.md | claude-code ; done
```

Key properties:
- Single task per loop iteration
- "Deterministically bad in an undeterministic world" - reliable failures, fixable via prompt tuning
- Fresh context each iteration (no accumulated conversation)
- Monolithic single-process architecture

#### SCUD Implementation

SCUD implements two flavors of the Ralph loop:

**1. Individual Task Ralph Loop** (`scud-cli/src/commands/spawn/terminal.rs:369-542`)
```bash
# Generated bash script
while true; do
  RALPH_ITER=$((RALPH_ITER + 1))
  claude "$(cat $PROMPT_FILE)" --dangerously-skip-permissions

  status=$(scud show $SCUD_TASK_ID | grep -i "status:" | awk '{print $2}')
  if [[ "$status" == "done" ]]; then
    break
  fi

  if [[ $RALPH_ITER -ge $RALPH_MAX_ITER ]]; then
    echo "Max iterations reached"
    break
  fi

  sleep 2
done
```

**2. Wave-Based Swarm Execution** (`scud-cli/src/commands/swarm/mod.rs:193-426`)
- Computes dependency waves using Kahn's algorithm
- Processes tasks in rounds within each wave
- Runs validation (backpressure) between waves
- Spawns repair agents on validation failure

**Key Enhancement**: SCUD adds **task status tracking** to the pure Ralph loop. The agent is expected to run `scud set-status <id> done` when complete, enabling deterministic termination rather than relying solely on iteration count.

### 2. Backpressure Implementation

#### Current State

Backpressure is formalized in SCUD as a validation/repair system:

**Configuration** (`.scud/config.toml`):
```toml
[swarm.backpressure]
commands = ["cargo build", "cargo test", "cargo clippy -- -D warnings"]
stop_on_failure = true
timeout_secs = 300
```

**Auto-Detection** (`scud-cli/src/backpressure.rs:147-194`):
- Rust: `Cargo.toml` → `cargo build`, `cargo test`
- Node.js: `package.json` → npm scripts (build, test, lint)
- Python: `pyproject.toml` → `pytest`
- Go: `go.mod` → `go build`, `go test`

**Execution Flow**:
1. **Wave completes** → All tasks in wave executed
2. **Validation triggered** → Run backpressure commands sequentially
3. **All pass** → Mark wave tasks as Done
4. **Failure detected** → Attribution phase begins

**Attribution Algorithm** (`scud-cli/src/attribution.rs:176-247`):
1. Parse error output for `file:line` references
2. Git blame each error location to find commit
3. Extract task ID from commit message (`[TASK-ID]` prefix expected)
4. Assign confidence: High (1 task), Medium (N tasks), Low (unknown)

**Repair Loop** (`scud-cli/src/commands/swarm/mod.rs:900-1082`):
1. Mark non-responsible tasks as Done (cleared)
2. For each responsible task (up to max_attempts=3):
   - Generate repair prompt with error context and changed files
   - Spawn repairer agent (claude/opus by default)
   - Wait for marker file (30min timeout)
   - Re-run validation
   - If passes: mark Done, break
   - If fails: continue to next attempt

**Gaps in Backpressure**:
- No incremental validation (full test suite each time)
- Attribution relies on clean commit messages with task IDs
- No rollback capability - failed repairs leave code in unknown state
- Repair agents don't have access to full error history

### 3. Idempotency Analysis

#### Durable Execution Concept (hatchet.run)

True durable execution requires:
- **Event sourcing**: Append-only log of all workflow actions
- **Deterministic replay**: Re-running workflow replays from log, skips completed steps
- **Idempotency keys**: Each subtask dispatched exactly once via unique key
- **No side effects in orchestrator**: All effects pushed to discrete tasks

#### SCUD's Approach

**Partial Idempotency**:
- Task status persisted to disk (`.scud/tasks/tasks.scg`)
- Wave state tracked in session files (`.scud/swarm/{session}.json`)
- Git commit SHA recorded per wave for attribution

**What's Missing**:

1. **No Event Sourcing**
   - Task status is current-state, not event-derived
   - Session files overwritten (not append-only)
   - No replay capability

2. **No Idempotency Keys**
   - Tasks selected by status filter, not idempotency key
   - Race condition: Two agents can both see task as `pending` and claim it
   - File lock prevents data corruption but not duplicate execution

3. **No Automatic Resume**
   - Crashed agent leaves task as `in-progress` indefinitely
   - `scud doctor --fix` required to reset stale tasks
   - No checkpoint to resume from

**Race Condition Example**:
```
Agent A: load_group() → sees task-1 as pending
Agent B: load_group() → sees task-1 as pending
Agent A: spawns agent, update_group(in-progress)
Agent B: spawns agent, update_group(in-progress) ← overwrites
Result: Two agents working on task-1
```

### 4. State Persistence Comparison

#### Current Persistence Model

**File-Based Storage**:
- Tasks: `.scud/tasks/tasks.scg` (SCG format)
- Sessions: `.scud/swarm/{session}.json`, `.scud/spawn/{session}.json`
- Active tag: `.scud/active-tag`
- Current task: `.scud/current-task`

**Locking Mechanism** (`scud-cli/src/storage/mod.rs:52-108`):
- File-level exclusive locks via `fs2` crate
- Exponential backoff retry (10 attempts, 10ms→1000ms)
- Read-modify-write cycle atomic within single phase update

**Atomic Update Pattern** (`scud-cli/src/storage/mod.rs:405-460`):
```rust
// Open file
file = OpenOptions::new().read(true).write(true).create(true).open(path)?;
// Acquire exclusive lock
acquire_lock_with_retry(&file, 10)?;
// Read current content while holding lock
let content = std::fs::read_to_string(&path)?;
// Parse, modify, re-serialize
let phases = parse_multi_scg(&content)?;
phases.insert(tag, modified_phase);
let output = serialize_multi_scg(&phases)?;
// Truncate and write
file.seek(SeekFrom::Start(0))?;
file.set_len(0)?;
file.write_all(output.as_bytes())?;
file.flush()?;
// Lock released on drop
```

**Gaps**:
- No write-ahead logging (WAL)
- Interrupt between truncate and write = data loss
- No multi-file transactions (task state and session state independent)
- Session files use plain `fs::write()` - no locking

#### SQLite Consideration

**Benefits SQLite Would Provide**:

1. **ACID Transactions**
   - Multi-table updates atomic
   - No partial writes on interrupt
   - Built-in rollback on failure

2. **WAL Mode**
   - Readers don't block writers
   - Crash recovery to consistent state
   - Higher concurrency than file locking

3. **Query Capability**
   - Find tasks by status, dependency, agent type efficiently
   - Aggregate queries for stats without parsing all tasks
   - Index on frequently-queried fields

4. **Event Sourcing Enablement**
   - Easy to add events table for append-only log
   - Foreign keys maintain referential integrity
   - Triggers for derived state

**Costs of SQLite**:

1. **Additional Dependency**
   - rusqlite crate + native library
   - Compilation complexity on some platforms

2. **Format Complexity**
   - Binary format (not human-readable like SCG)
   - Need export/import tools for debugging
   - Harder to manually edit if needed

3. **Migration Effort**
   - Significant refactor of storage layer
   - Need migration path for existing `.scg` files

4. **Overkill for Single-User**
   - File locking sufficient if truly single-agent
   - SQLite's concurrency benefits mainly for multi-agent

**Recommendation**: Consider SQLite for the **session/transcript** layer rather than task storage:
- Tasks remain in SCG (human-readable, LLM-friendly)
- Transcripts move to SQLite (append-only, queryable)
- Session state moves to SQLite (atomic multi-record updates)

### 5. Context Handoff (Descartes)

Descartes implements a context management system that aligns with the "fresh context" principle:

**Token Monitoring** (`descartes/descartes/src/context_handoff.rs:16-27`):
- Default: 200K token context window
- Threshold: 60% (triggers at 120K tokens)
- Simple estimation: 4 chars ≈ 1 token

**Handoff Flow** (`descartes/descartes/src/swarm_executor.rs:583-738`):
1. Monitor records tokens for prompt + each response chunk
2. When threshold reached, break response collection
3. Generate summary of progress (status, files modified, recent actions)
4. Create fresh agent session with summary + original task spec
5. Reset token counter, continue execution

**Summary Generation** (`context_handoff.rs:139-223`):
- Detects completion via `DONE]` or `completed successfully`
- Extracts file modifications via `modified:`, `created:` patterns
- Preserves last 3 substantive lines of output
- Filters separator lines and short lines

**State Lost in Handoff**:
- Full conversation history
- Detailed reasoning and intermediate steps
- Tool call arguments and results
- Only summary text preserved

### 6. Comparison Matrix

| Aspect | Ralph (Pure) | SCUD | Hatchet (Durable) |
|--------|--------------|------|-------------------|
| **Iteration Model** | Infinite bash loop | Wave-based with DAG | Step-based workflow |
| **Termination** | Max iterations | Task status | Workflow completion |
| **Context** | Fresh each iteration | Fresh each task | Fresh each step |
| **Backpressure** | None | Validation + Attribution + Repair | Retry policies |
| **Resumability** | None | Manual (`scud doctor`) | Automatic from checkpoint |
| **Idempotency** | None | Partial (status-based) | Full (event log) |
| **State Storage** | Filesystem | SCG files + JSON sessions | Event log + state |
| **Concurrency** | Single agent | File-locked, races possible | Idempotency keys |

### 7. Gap Analysis and Recommendations

#### Critical Gaps

1. **Duplicate Execution Risk**
   - Current: File lock only prevents corruption, not duplicate claim
   - Impact: Two agents can work same task → wasted compute, conflicts
   - Fix: Add claim timestamp + agent ID, reject claim if already claimed recently

2. **No Crash Resume**
   - Current: `in-progress` tasks orphaned on crash
   - Impact: Manual intervention required, work potentially lost
   - Fix: Heartbeat mechanism or timeout-based auto-reset

3. **Session State Not Atomic**
   - Current: `fs::write()` for session files, no locking
   - Impact: Corrupted session file on interrupt
   - Fix: Use same locking pattern as task storage, or move to SQLite

#### Enhancement Opportunities

1. **Formalize Backpressure as First-Class Concept**
   - Make backpressure commands discoverable (not just in config)
   - Add per-task backpressure overrides
   - Support incremental validation (only run relevant tests)

2. **Add Event Log for Durability**
   - Append-only event log alongside current state
   - Events: TaskClaimed, TaskStarted, ToolExecuted, TaskCompleted, ValidationRun
   - Enable replay for debugging and resume

3. **Implement True Idempotency**
   - Generate idempotency key per task execution attempt
   - Check for existing attempt before spawning
   - Store attempt ID in transcript/session

4. **Structured Transcript Storage**
   - Current: SCG format, token-efficient but not queryable
   - Consider: SQLite for transcripts with full-text search
   - Enables: "Find all executions that used tool X" queries

#### Intentional Simplifications

Some gaps may be intentional design decisions:

1. **No Event Sourcing** - Complexity cost outweighs benefit for single-developer use
2. **Manual Doctor** - Humans should review orphaned tasks, not auto-reset
3. **File-Based Storage** - Human-readable, git-friendly, easy to debug
4. **Status-Based Claiming** - Simple, works for non-concurrent use cases

## Architecture Documentation

### Current Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                          SCUD Swarm                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. Load Tasks from .scud/tasks/tasks.scg                      │
│                 ↓                                               │
│  2. Compute Waves (Kahn's Algorithm)                           │
│     [Wave 1: no deps] → [Wave 2: deps on W1] → ...            │
│                 ↓                                               │
│  3. For Each Wave:                                              │
│     ┌─────────────────────────────────────────┐                │
│     │  a. Divide into Rounds (round_size)     │                │
│     │  b. For Each Round:                     │                │
│     │     - Spawn agents for each task        │                │
│     │     - Set status → in-progress          │                │
│     │     - Wait for round completion         │                │
│     │  c. Run Backpressure Validation         │                │
│     │     - If pass: mark tasks → done        │                │
│     │     - If fail: Attribution → Repair     │                │
│     └─────────────────────────────────────────┘                │
│                 ↓                                               │
│  4. All Waves Complete → Exit                                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Backpressure Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     Backpressure Validation                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Wave Completes                                                 │
│        ↓                                                        │
│  Run Validation Commands                                        │
│  (cargo build, cargo test, etc.)                               │
│        ↓                                                        │
│  ┌─────────────┐     ┌─────────────────────────────┐           │
│  │  All Pass   │ YES │ Mark all wave tasks → done  │           │
│  └──────┬──────┘     └─────────────────────────────┘           │
│         │ NO                                                    │
│         ↓                                                       │
│  Parse Error Locations (file:line)                             │
│        ↓                                                        │
│  Git Blame → Find Responsible Task                             │
│        ↓                                                        │
│  ┌─────────────────────────────────────────────────┐           │
│  │  Confidence: High (1 task) / Medium / Low       │           │
│  └─────────────────────────────────────────────────┘           │
│        ↓                                                        │
│  Clear Non-Responsible Tasks (mark → done)                     │
│        ↓                                                        │
│  For attempt in 1..max_attempts:                               │
│    - Generate repair prompt                                     │
│    - Spawn repairer agent                                       │
│    - Wait for completion (30min timeout)                       │
│    - Re-run validation                                          │
│    - If pass: break                                             │
│        ↓                                                        │
│  Exhausted? Mark responsible tasks → failed                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### State Persistence Model

```
.scud/
├── tasks/
│   └── tasks.scg           # Task graph (SCG format, file-locked)
├── swarm/
│   └── {session}.json      # Swarm session state (not locked!)
├── spawn/
│   └── {session}.json      # Spawn session state (not locked!)
├── config.toml             # Configuration including backpressure
├── active-tag              # Currently active phase tag
└── current-task            # Task ID for commit prefixing

File Locking:
  tasks.scg: fs2 exclusive lock with exponential backoff
  *.json:    Plain fs::write (no locking) ← Gap
```

## Code References

### Core Files

- `scud-cli/src/commands/swarm/mod.rs:193-426` - Main swarm execution loop
- `scud-cli/src/commands/swarm/mod.rs:900-1082` - Repair loop implementation
- `scud-cli/src/backpressure.rs:225-284` - Validation command execution
- `scud-cli/src/attribution.rs:176-247` - Git blame attribution
- `scud-cli/src/storage/mod.rs:52-108` - File locking mechanism
- `scud-cli/src/storage/mod.rs:405-460` - Atomic phase updates
- `scud-cli/src/formats/scg.rs:122-368` - SCG format parser
- `scud-cli/src/commands/doctor.rs:85-264` - Stale task detection

### Descartes Context Handoff

- `descartes/descartes/src/context_handoff.rs:16-27` - ContextMonitor struct
- `descartes/descartes/src/context_handoff.rs:139-223` - Summary generation
- `descartes/descartes/src/swarm_executor.rs:583-738` - Handoff integration

### Agent Spawning

- `scud-cli/src/commands/spawn/terminal.rs:369-542` - Ralph loop bash script
- `scud-cli/src/commands/spawn/agent.rs:194-241` - Repair prompt generation

## Open Questions

1. **Should duplicate execution be prevented or accepted?**
   - Current: Possible, not prevented
   - Trade-off: Locking complexity vs resource waste

2. **Is auto-resume desirable or should humans review?**
   - Current: Manual via `scud doctor`
   - Trade-off: Automation vs oversight

3. **Would SQLite add enough value to justify complexity?**
   - For task storage: Probably not (SCG works well)
   - For transcripts/sessions: Possibly yes (queryability, atomicity)

4. **Should backpressure be per-wave or per-task?**
   - Current: Per-wave
   - Alternative: Per-task for finer attribution

5. **How to handle context handoff across multiple tasks?**
   - Current: Per-task context reset
   - Gap: No way to share learnings across tasks in same wave

## Related Research

- ghuntley.com/ralph - Original Ralph Wiggum pattern
- ghuntley.com/loop - Loop orchestration philosophy
- hatchet.run/blog/durable-execution - Durable execution patterns
