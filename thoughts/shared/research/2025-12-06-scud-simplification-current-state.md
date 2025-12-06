---
date: 2025-12-06T17:54:59Z
researcher: Claude
git_commit: 21b72a750f994b63615b09369947c2f15c4acaea
branch: master
repository: pyrex41/scud
topic: "SCUD Simplification - Current State Documentation"
tags: [research, codebase, task-management, claiming, locking, workflow, scg-format]
status: complete
last_updated: 2025-12-06
last_updated_by: Claude
---

# Research: SCUD Simplification - Current State Documentation

**Date**: 2025-12-06T17:54:59Z
**Researcher**: Claude
**Git Commit**: 21b72a750f994b63615b09369947c2f15c4acaea
**Branch**: master
**Repository**: pyrex41/scud

## Research Question

Document the current state of SCUD components targeted for simplification:
- Task claiming/locking system
- 5-phase workflow and agent personas
- Phase gate enforcement
- workflow-state.json
- SCG format lock fields
- set-status granular tracking

## Summary

SCUD has evolved from a complex 5-phase workflow system to a dependency-based task management tool. Key findings:

| Component | Status | Location |
|-----------|--------|----------|
| Task claiming/locking | **Active** | Fully implemented in Rust CLI |
| 5-phase workflow | **Documentation only** | Not enforced in code |
| Agent personas (PM, SM, etc.) | **Not created** | Defined in config, files don't exist |
| workflow-state.json | **Legacy** | Not read by Rust CLI |
| Phase gate enforcement | **None** | Convention-based only |
| SCG lock fields | **Active** | `assigned_to`, `locked_by`, `locked_at` |

## Detailed Findings

### 1. Task Claiming/Locking System

**Status**: Fully implemented and active

#### Core Data Model (`scud-cli/src/models/task.rs:108-115`)

```rust
pub assigned_to: Option<String>,   // Soft assignment (persists after release)
pub locked_by: Option<String>,     // Hard lock (exclusive access)
pub locked_at: Option<String>,     // ISO 8601 timestamp
```

#### Commands

| Command | File | Purpose |
|---------|------|---------|
| `scud claim` | `commands/claim.rs` | Acquire exclusive lock on task |
| `scud release` | `commands/release.rs` | Release lock (with `--force` option) |
| `scud assign` | `commands/assign.rs` | Soft assignment without locking |
| `scud next --claim` | `commands/next.rs:136-296` | Find and auto-claim next task |
| `scud sessions` | `commands/sessions.rs` | List active locks |
| `scud whois` | `commands/whois.rs` | Show who is working on what |
| `scud doctor` | `commands/doctor.rs:156-290` | Detect stale locks, auto-fix |

#### Lock Methods (`task.rs:347-407`)

- `claim(&mut self, assignee: &str)` - Acquire lock, fails if locked by another
- `release(&mut self)` - Clear lock (preserves `assigned_to`)
- `is_locked()` / `is_locked_by(assignee)` - Check lock status
- `lock_age_hours()` - Calculate lock duration
- `is_stale_lock(threshold)` - Check if lock exceeds threshold

#### Atomic Operations (`storage/mod.rs:31-53, 319-376`)

File-level locking with retry:
- Exclusive lock via `fs2::FileExt`
- Exponential backoff (10ms → 1s)
- Maximum 10 retries
- Lock held across read-modify-write cycle

---

### 2. Five-Phase Workflow

**Status**: Documentation-only, not enforced in code

#### The 5 Phases (from documentation)

1. **Ideation** - Define what to build (PRD)
2. **Planning** - Break into tasks
3. **Architecture** - Design technical solution
4. **Implementation** - Execute tasks
5. **Retrospective** - Learn and improve

#### Key Finding

From `thoughts/shared/research/2025-12-02-scud-vs-beads-comparison.md:359`:
> "Important Finding: The workflow state file is NOT read by the CLI. Phase enforcement is convention-based through agent documentation."

#### What Actually Exists

The `Phase` struct in `models/phase.rs:5-8` is a **task container**, not a workflow phase:

```rust
pub struct Phase {
    pub name: String,      // Tag name (e.g., "epic-1-auth")
    pub tasks: Vec<Task>,  // Tasks in this group
}
```

Phase transitions are **not implemented** - only task dependencies are enforced.

---

### 3. Agent Personas

**Status**: Defined in configuration but files do not exist

#### Agent Definitions (`commands/config.rs:12-39`)

| Agent | File | Description |
|-------|------|-------------|
| PM | `pm.md` | Product Manager - PRD creation |
| SM | `sm.md` | Scrum Master - Task breakdown |
| Architect | `architect.md` | Technical design |
| Dev | `dev.md` | Task implementation |
| Retrospective | `retrospective.md` | Post-phase analysis |
| Status | `status.md` | Workflow status reporting |

#### Expected Location
`.claude/commands/scud/{agent}.md`

#### Current State
- Infrastructure exists: `scud config agents add/remove/list`
- Agent source files: **Never created**
- Referenced by: `.opencode/skills/scud-sm.md` (line 16)

---

### 4. workflow-state.json

**Status**: Legacy file, not used by Rust CLI

#### Historical Location
`.taskmaster/workflow-state.json`

#### Historical Schema
```json
{
  "version": "1.0.0",
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": { "status": "active", "agent": "tm-pm", ... },
    "planning": { "status": "pending", "agent": "tm-pm", ... },
    ...
  },
  "history": [],
  "completed_epics": []
}
```

#### Modern Replacement

| Old | New |
|-----|-----|
| `workflow-state.json` | `.scud/active-tag` (plain text) |
| `tasks/tasks.json` | `.scud/tasks/tasks.scg` |
| Phase tracking | **Removed** |

#### Active Tag Implementation (`storage/mod.rs`)

- `get_active_group()` - Lines 242-270
- `set_active_group()` - Lines 272-287
- Simple text file containing tag name

---

### 5. SCG Format Lock Fields

**Status**: Active, lock fields ARE stored in SCG format

#### @assignments Section (`formats/scg.rs:248-270, 455-474`)

Format: `id | assigned_to | locked_by | locked_at`

```
@assignments
# id | assigned_to | locked_by | locked_at
auth:1 | alice | alice | 2025-01-15T10:30:00Z
auth:2 | bob |  |
```

#### Parsing (lines 248-269)
- All three fields parsed as optional strings
- Empty string treated as None

#### Serialization (lines 455-474)
- Section only written if any task has assignment or lock
- Empty values serialized as empty strings

#### Round-Trip Test (lines 679-694)
Confirms lock fields survive serialization cycle.

---

### 6. set-status Command

**Status**: Active with auto-release logic

#### Implementation (`commands/set_status.rs:9-63`)

#### Supported Statuses (`models/task.rs:5-15`)

| Status | Code | Description |
|--------|------|-------------|
| pending | P | Default, waiting |
| in-progress | I | Being worked on |
| done | D | Completed |
| review | R | Awaiting review |
| blocked | B | External blocker |
| deferred | F | Postponed |
| cancelled | C | Cancelled |
| expanded | X | Broken into subtasks |

#### Auto-Release on Done (lines 32-38)

```rust
if is_done && was_locked {
    task.release();           // Clear locked_by, locked_at
    task.assigned_to = None;  // Clear assignment
}
```

---

## Components Summary for Simplification

### KEEP (as documented)

| Component | Location | Reason |
|-----------|----------|--------|
| SCG format | `formats/scg.rs` | Efficient, git-friendly |
| Task dependencies | `models/task.rs` | DAG structure |
| Wave computation | `commands/waves.rs` | Parallelism planning |
| Complexity estimation | `models/task.rs:320-340` | Fibonacci scoring |
| `scud parse-prd` | `commands/ai/parse_prd.rs` | AI task generation |
| `scud waves` | `commands/waves.rs` | Execution planning |
| `scud stats` | `commands/stats.rs` | Progress tracking |

### REMOVE/SIMPLIFY (as documented)

| Component | Location | Status |
|-----------|----------|--------|
| Task claiming | `commands/claim.rs` | Active, can remove |
| Task release | `commands/release.rs` | Active, can remove |
| Lock fields | `models/task.rs:109-115` | Active, can remove |
| @assignments section | `formats/scg.rs:248-270` | Active, can remove |
| `scud sessions` | `commands/sessions.rs` | Active, can remove |
| Agent personas | `commands/config.rs:12-39` | Defined, files missing |
| 5-phase workflow | Documentation only | Already not enforced |
| workflow-state.json | `.taskmaster/` | Already legacy |
| Phase gates | None exist | Already not implemented |

### ALREADY SIMPLIFIED

| Component | Status |
|-----------|--------|
| 5-phase workflow | Convention-only, no code enforcement |
| Phase gates | Not implemented |
| Handoff ceremonies | Not implemented |
| workflow-state.json | Not read by CLI |

## Code References

### Task Model
- `scud-cli/src/models/task.rs:108-115` - Lock fields
- `scud-cli/src/models/task.rs:347-407` - Lock methods
- `scud-cli/src/models/task.rs:286-289` - set_status method

### Commands
- `scud-cli/src/commands/claim.rs` - Claim command
- `scud-cli/src/commands/release.rs` - Release command
- `scud-cli/src/commands/set_status.rs:32-38` - Auto-release
- `scud-cli/src/commands/next.rs:136-346` - Claim/release modes
- `scud-cli/src/commands/sessions.rs` - Active sessions
- `scud-cli/src/commands/whois.rs` - Assignment tracking
- `scud-cli/src/commands/doctor.rs:156-290` - Stale lock detection

### Storage
- `scud-cli/src/storage/mod.rs:31-53` - File locking
- `scud-cli/src/storage/mod.rs:242-287` - Active group
- `scud-cli/src/storage/mod.rs:319-376` - Atomic updates

### Format
- `scud-cli/src/formats/scg.rs:248-270` - Assignment parsing
- `scud-cli/src/formats/scg.rs:455-474` - Assignment serialization

### Configuration
- `scud-cli/src/commands/config.rs:12-39` - Agent definitions

### Documentation
- `docs/guides/COMPLETE_GUIDE.md` - 5-phase workflow docs
- `docs/reference/QUICK_REFERENCE.md` - Phase mapping
- `thoughts/shared/plans/2025-12-01-scud-v2-beads-inspired-refactor.md` - Refactor plan

## Architecture Documentation

### Current Task Flow

```
scud parse-prd → Tasks created
                     ↓
              scud analyze-complexity
                     ↓
              scud expand (complex tasks)
                     ↓
              scud waves → Execution plan
                     ↓
              scud next → Find available task
                     ↓
              [Optional: scud claim → Lock task]
                     ↓
              Work on task
                     ↓
              scud set-status done → [Auto-release lock]
```

### Dependency Enforcement (What Actually Works)

Task order is enforced by:
1. Dependency graph in SCG format (`@edges` section)
2. `has_dependencies_met()` check before task selection
3. Cross-tag dependency support

Phase order is **NOT** enforced by any code.

## Open Questions

1. Should `assigned_to` field be kept even if locking is removed? (For informational purposes)
2. Should `scud doctor` be kept for other diagnostic purposes beyond stale locks?
3. Should the agent persona infrastructure be removed entirely or repurposed?
