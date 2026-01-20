---
date: 2026-01-20T18:45:00-08:00
researcher: Claude
git_commit: 66770928ce940c3b2f449e711e294be8d5691d16
branch: master
repository: scud
topic: "Status Update Implementation and Transcript/Summary Capabilities"
tags: [research, codebase, status, transcripts, swarm, spawn, sessions]
status: complete
last_updated: 2026-01-20
last_updated_by: Claude
---

# Research: Status Update Implementation and Transcript/Summary Capabilities

**Date**: 2026-01-20T18:45:00-08:00
**Researcher**: Claude
**Git Commit**: 66770928ce940c3b2f449e711e294be8d5691d16
**Branch**: master
**Repository**: scud

## Research Question

How can we better implement status updates and get summaries of what tasks did (either from transcripts or last output)?

## Summary

The codebase has a mature status update system supporting single, multi-task, and bulk transitions. For transcript/summary capabilities, there are three distinct systems:

1. **Discovery Logs** (`scud log`) - Simple timestamped append-only logs per task that agents can write/read
2. **Spawn Sessions** - Track agent status in `.scud/spawn/` with basic counts by status
3. **Swarm Sessions** - Rich wave-based tracking with git commit ranges, file changes, validation results, and repair attempts

**Key finding**: While wave summaries track *what files changed* and *which tasks completed*, there is no current mechanism to capture *what the agent actually did* - no transcript capture, no output summarization, and no structured summary of agent actions beyond file diffs.

## Detailed Findings

### 1. Status Update System

The status update system is fully implemented at `scud-cli/src/commands/set_status.rs` with three modes:

**Single Task Mode** (backward compatible):
```bash
scud set-status <task_id> <status>
```

**Multi-Task Mode**:
```bash
scud set-status <status> <task_id> [task_id...]
```

**Bulk Transition Mode**:
```bash
scud set-status --from <status> --to <status> [--all-tags]
```

#### Status Values
Defined at `scud-cli/src/models/task.rs:5-16`:
- `pending` (default)
- `in-progress`
- `done`
- `review`
- `blocked`
- `deferred`
- `cancelled`
- `expanded`
- `failed`

#### How Status Updates Work

1. **Validation**: `TaskStatus::from_str()` validates status string
2. **Model Update**: `task.set_status(new_status)` at `task.rs:287-290`:
   - Sets `task.status` field
   - Updates `task.updated_at` to current RFC3339 timestamp
3. **Persistence**: `storage.update_group(tag, &phase)` atomically updates the `.scud/tasks/tasks.scg` file

#### Automatic Status Updates (Hooks)

The spawn command installs a Claude Code stop hook at `spawn/hooks.rs:68-85`:
```json
{
  "type": "command",
  "command": "bash -c 'if [ -n \"$SCUD_TASK_ID\" ]; then scud set-status \"$SCUD_TASK_ID\" done 2>/dev/null || true; fi'",
  "timeout": 10
}
```
This automatically marks tasks as `done` when an agent session ends.

### 2. Discovery Log System

Location: `scud-cli/src/commands/log.rs`

**Purpose**: Simple file-based logging for agents to share discoveries

**Storage**: `.scud/logs/<task-id>.log` - one file per task

**Entry Format**:
```
--- 2026-01-20 14:30:45 ---
Found authentication helpers in lib/auth.rs, not in utils/

--- 2026-01-20 15:15:22 ---
Token refresh logic has edge case handling
```

**Commands**:
- `scud log <task-id> "message"` - Append timestamped entry
- `scud log-show <task-id>` - View logs for specific task
- `scud log-all --limit N [--tag <tag>]` - View recent entries across tasks

**Integration with Agents** (`agent.rs:62-82`):
- Agents are instructed to check `scud log-all --limit 10` for discoveries
- Agents are told to log findings via `scud log {id} "message"`

**Limitations**:
- No automatic capture of agent output
- Relies on agent cooperation to log discoveries
- No summarization capability
- No structured format

### 3. Spawn Session State

Location: `scud-cli/src/commands/spawn/monitor.rs`

**Storage**: `.scud/spawn/<session-name>.json`

**Data Structures**:

```rust
// Per-agent state
pub struct AgentState {
    task_id: String,
    task_title: String,
    window_name: String,      // tmux window name
    status: AgentStatus,      // Starting|Running|Completed|Failed
    started_at: String,       // RFC3339
    tag: String,
}

// Session state
pub struct SpawnSession {
    session_name: String,
    tag: String,
    terminal: String,         // "tmux"
    created_at: String,
    working_dir: String,
    agents: Vec<AgentState>,
}
```

**Summary Capability**:
- `SpawnStats` derives counts: `count_by_status()` returns map of status to count
- No content summarization
- No output capture

### 4. Swarm Session State

Location: `scud-cli/src/commands/swarm/session.rs`

**Storage**: `.scud/swarm/<session-name>.json`

**Data Structures**:

```rust
// Wave summary (what exists today)
pub struct WaveSummary {
    wave_number: usize,
    tasks_completed: Vec<String>,    // Task IDs
    files_changed: Vec<String>,      // File paths from git diff
}

// Per-round state
pub struct RoundState {
    round_number: usize,
    task_ids: Vec<String>,
    tags: Vec<String>,
    failures: Vec<String>,
    started_at: String,
    completed_at: Option<String>,
}

// Review results
pub struct ReviewState {
    reviewed_tasks: Vec<String>,
    all_passed: bool,
    tasks_needing_improvement: Vec<String>,
    completed_at: String,
}

// Repair tracking
pub struct RepairAttempt {
    attempt_number: usize,
    attributed_tasks: Vec<String>,
    cleared_tasks: Vec<String>,
    attribution_confidence: String,  // high/medium/low
    validation_passed: bool,
    completed_at: String,
}

// Wave-level state
pub struct WaveState {
    wave_number: usize,
    rounds: Vec<RoundState>,
    validation: Option<ValidationResult>,
    summary: Option<WaveSummary>,
    start_commit: Option<String>,     // Git SHA at wave start
    review: Option<ReviewState>,
    repairs: Vec<RepairAttempt>,
    started_at: String,
    completed_at: Option<String>,
}
```

**Wave Summary Generation** (`swarm/mod.rs:424-430`):
```rust
WaveSummary {
    wave_number,
    tasks_completed: wave_state.all_task_ids(),
    files_changed: collect_changed_files(start_commit),  // git diff
}
```

**Text Summary Output** (`session.rs:45-73`):
```rust
pub fn to_text(&self) -> String {
    // "Wave N completed N tasks"
    // "Tasks: id1, id2, id3"
    // "Files changed: file1.rs, file2.rs, ..."
}
```

### 5. What's Missing for Task Summaries

**No Transcript Capture**:
- Agents run in tmux windows via `spawn_tmux()` at `terminal.rs:235-324`
- Commands sent via `tmux send-keys`, no stdout/stderr capture
- TUI can poll tmux panes (`tmux capture-pane -p -S -1000`) but only for live display
- No persistent storage of agent output

**No Output Summarization**:
- Wave summaries list files changed (from git diff) and task IDs
- No mechanism to summarize *what* an agent did beyond file changes
- No LLM-powered summarization of agent activities

**Current "Last Output" Equivalent**:
- `WaveSummary.to_text()` provides: wave number, task list, files changed
- `SwarmSession.get_previous_summary()` retrieves last wave's text summary
- Comment explicitly states: "This is just 'what was done', not accumulated context"

### 6. TUI Live Output Capture

Location: `scud-cli/src/commands/spawn/tui/app.rs:200-266`

The TUI does capture tmux pane output for display:
```rust
// Captures up to 1000 lines of scrollback every 500ms
Command::new("tmux")
    .args(["capture-pane", "-p", "-S", "-1000", "-t", &window_name])
```

This pattern could be leveraged for persistent capture, but currently:
- Only used for live display
- Not persisted to disk
- No summarization

## Code References

### Status Update System
- `scud-cli/src/commands/set_status.rs` - Command implementation
- `scud-cli/src/models/task.rs:287-290` - `Task.set_status()` method
- `scud-cli/src/models/task.rs:5-16` - `TaskStatus` enum
- `scud-cli/src/main.rs:182-204` - CLI argument definition
- `scud-cli/src/commands/spawn/hooks.rs:68-85` - Auto-done hook

### Discovery Logs
- `scud-cli/src/commands/log.rs:12-58` - `run()` write entry
- `scud-cli/src/commands/log.rs:61-75` - `show()` single task
- `scud-cli/src/commands/log.rs:78-174` - `show_all()` aggregated view
- `scud-cli/src/commands/spawn/agent.rs:62-82` - Agent prompt instructions

### Spawn Sessions
- `scud-cli/src/commands/spawn/monitor.rs:22-30` - `AgentState` struct
- `scud-cli/src/commands/spawn/monitor.rs:33-41` - `SpawnSession` struct
- `scud-cli/src/commands/spawn/monitor.rs:124-133` - `save_session()`

### Swarm Sessions
- `scud-cli/src/commands/swarm/session.rs:35-42` - `WaveSummary` struct
- `scud-cli/src/commands/swarm/session.rs:45-73` - `WaveSummary.to_text()`
- `scud-cli/src/commands/swarm/session.rs:141-164` - `WaveState` struct
- `scud-cli/src/commands/swarm/session.rs:208-226` - `SwarmSession` struct
- `scud-cli/src/commands/swarm/session.rs:270-276` - `get_previous_summary()`
- `scud-cli/src/commands/swarm/mod.rs:424-430` - Wave summary creation
- `scud-cli/src/commands/swarm/mod.rs:820-843` - `collect_changed_files()`

### TUI Output Capture
- `scud-cli/src/commands/spawn/tui/app.rs:200-266` - `refresh_live_output()`

## Architecture Documentation

### Current Data Flow for Status Updates

```
User/Agent → scud set-status → TaskStatus validation → Task.set_status()
           → storage.update_group() → SCG file (atomic write with lock)
```

### Current Data Flow for Task Activity Tracking

```
Agent starts → AgentState added to SpawnSession → JSON saved
Agent works  → (no capture)
Agent stops  → Hook marks task done → SpawnSession updated
Wave ends    → git diff → WaveSummary created (files + task IDs)
```

### Session File Locations
```
.scud/
├── tasks/
│   └── tasks.scg          # Task definitions with status
├── logs/
│   └── <task-id>.log      # Discovery logs (manual entries)
├── spawn/
│   └── <session>.json     # Spawn session state
└── swarm/
    ├── <session>.json     # Swarm session state with wave summaries
    └── <tag>.lock         # Concurrency lock
```

## Potential Extension Points

Based on the existing architecture, here are natural extension points for transcript/summary capabilities:

1. **Extend `WaveSummary`** with an `agent_summary: Option<String>` field
2. **Capture tmux output** on task completion using the TUI's `capture-pane` pattern
3. **Add summarization step** after wave completion that processes captured output
4. **Extend discovery logs** with structured format for machine parsing

The infrastructure for capturing tmux output exists in the TUI code and could be adapted for persistent capture.

## Related Research

- `thoughts/shared/plans/2026-01-20-bulk-status-command.md` - Bulk status implementation plan (already implemented)
- `thoughts/shared/plans/2026-01-20-monitor-swarm-integration.md` - Monitor/swarm integration plan
- `thoughts/shared/research/2026-01-18-ralph-loop-backpressure-idempotency-analysis.md` - Related analysis

## Comparison: Existing Capabilities vs. Proposed Summary Plan

Reference: `thoughts/shared/plans/2026-01-20-opt-in-task-summaries.md`

### What Exists Today

| Capability | Current State | Location |
|------------|---------------|----------|
| Task status field | Yes - `TaskStatus` enum | `models/task.rs:5-16` |
| Task summary field | **No** | - |
| Discovery logs | Yes - append-only `.scud/logs/<id>.log` | `commands/log.rs` |
| Log parsing | Yes - timestamp-based entry splitting | `log.rs:120-147` |
| LLM client | Yes - generic client in `llm/` module | `scud-cli/src/llm/` |
| Config system | Yes - `.scud/config.toml` | `scud-cli/src/config.rs` |
| Wave summaries | Yes - files changed + task IDs only | `swarm/session.rs:35-73` |

### What the Plan Proposes

1. **New `summary: Option<String>` field on Task** - Stores human/LLM-generated summary
2. **New `scud summary` command** - Generates summaries from logs via LLM or extracts last entry
3. **Integration points**: `show`, `next`, `stats`, `set-status --summary`

### Gap Analysis

| Plan Requirement | Existing Infrastructure | Gap |
|-----------------|------------------------|-----|
| `task.summary` field | Task model has fields, SCG format has `@details` section | Need to add field + SCG serialization |
| LLM summarization | LLM client exists in `src/llm/` | Need new command + prompt |
| Log parsing | `log.rs:120-147` parses entries | Can reuse directly |
| Config for model | Config system exists | Need `[llm]` section |
| Display in `show`/`next` | These commands read Task fields | Minor addition |

### Alignment with Existing Patterns

**Positive alignments:**
- Uses existing log storage (`.scud/logs/<id>.log`) - no new storage location
- Follows existing command pattern (separate `commands/summary.rs` file)
- Opt-in approach matches codebase philosophy (no automatic behavior)
- Reuses LLM client infrastructure

**Considerations:**
- SCG format would need `@summaries` section (similar to `@details`, `@agents`)
- The plan's `--last` flag aligns well with existing log entry format
- Bulk operations (`--all-done`) follow pattern from `set-status --from/--to`

### Recommended Implementation Order

Based on codebase analysis, natural order would be:

1. **Add field to Task model** (`models/task.rs`)
   - Add `summary: Option<String>`
   - Update `Task::new()` default
   - No validation needed (optional field)

2. **Add SCG serialization** (`formats/scg.rs`)
   - Add `@summaries` section (pattern: `id | summary text`)
   - Follow existing `@details` multi-line pattern for long summaries

3. **Create `summary.rs` command**
   - Reuse log parser from `log.rs:120-147`
   - Use existing LLM client
   - Follow existing command structure

4. **Add config**
   - Add `[llm]` section with `model` key
   - Default to "fast" or first configured model

5. **Update display commands**
   - `show.rs`: Add summary to output
   - `next.rs`: Show summary if available

## Open Questions

1. **Where should captured output be stored?** Options: per-task files, in swarm session JSON, separate transcript directory
2. **When should summarization happen?** Options: on task completion, on wave completion, on-demand
3. **What format for summaries?** Options: free-form text, structured JSON, markdown
4. **Should summaries be LLM-generated or template-based?** The codebase has LLM integration (`scud-cli/src/llm/`) that could be leveraged

### Additional Questions from Plan Review

5. **Prompt template**: What specific prompt works best for task summarization?
6. **Max summary length**: 200 words seems reasonable; how to handle truncation?
7. **Bulk operations**: Should `--all-in-progress` be supported initially or added later?
8. **Agent integration**: Should swarm/spawn prompts include `scud summary {id} --auto --set`?
