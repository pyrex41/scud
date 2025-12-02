# SCUD v2.0 "Beads-Inspired" Refactor Implementation Plan

## Overview

Refactor SCUD from a rigid 5-phase workflow with agent roles into a flexible, DAG-driven task management system inspired by Beads. The core insight: the dependency graph IS the workflow - no artificial phases or role gates needed. Agents query the graph for ready tasks and execute them; the graph evolves organically.

## Current State Analysis

**What exists today:**
- Rigid 5-phase workflow state machine (Ideation → Planning → Architecture → Implementation → Retrospective)
- 5 agent roles with phase gates that block execution
- Solid DAG-based task system with dependencies and claiming
- Wave computation (Kahn's algorithm) for visualization
- SCG format for token-efficient storage
- MCP server exposing tools/resources

**Key files to modify:**
- `scud-cli/src/models/workflow.rs` - Remove Phase enum and WorkflowState
- `scud-cli/src/storage/mod.rs` - Remove workflow state loading/saving
- `scud-cli/src/commands/*.rs` - Remove phase checks, add hooks commands
- `scud-mcp/src/tools/*.rs` - Simplify tool set
- `.claude/commands/scud/*.md` - Remove agent personas, keep task commands

### Key Discoveries:
- `scud next` already works DAG-driven (finds tasks with deps met) - `/Users/reuben/bmad-tm/scud-cli/src/commands/next.rs:22-56`
- Wave computation is read-only visualization - `/Users/reuben/bmad-tm/scud-cli/src/commands/waves.rs:201-278`
- Task claiming/locking is solid - `/Users/reuben/bmad-tm/scud-cli/src/models/task.rs:340-369`
- SCG format parser is clean - `/Users/reuben/bmad-tm/scud-cli/src/formats/scg.rs`

## Desired End State

After this refactor:

1. **No workflow phases** - Tasks have tags for grouping, but no sequential phase gates
2. **No agent roles** - Slash commands removed; any agent can work on any task
3. **DAG-driven execution** - `scud next` returns any ready task; orchestrator loops spawn agents
4. **Claude Code hooks** - Stop hooks enforce completion; SessionStart hooks handle claiming
5. **Simplified mental model** - "Query the graph, do work, update the graph"

### Verification:
- `scud init` creates `.scud/` without `workflow-state.json`
- `scud next` works without checking phase
- Claude Code Stop hook fires and calls `scud complete`
- No `/scud:pm`, `/scud:sm`, etc. commands exist
- `scud waves` still works for visualization but isn't required for execution

## What We're NOT Doing

- **NOT changing SCG format** - Keep token-efficient storage as-is
- **NOT changing task structure** - Keep dependencies, complexity, priority
- **NOT removing waves command** - Keep for visualization/planning
- **NOT adding heartbeat background processes** - Hooks handle this
- **NOT changing MCP protocol** - Just simplifying exposed tools

## Implementation Approach

Strip the rigid workflow layer while preserving the solid DAG foundation. Add Claude Code hooks for reliability. The refactor is mostly deletion with targeted additions for hooks.

---

## Phase 1: Strip Workflow State Machine

### Overview
Remove the 5-phase workflow and WorkflowState entirely. Tasks still have tags for grouping, but no sequential phase enforcement.

### Changes Required:

#### 1.1 Remove Workflow Model

**File**: `scud-cli/src/models/workflow.rs`
**Changes**: Delete the entire file (or gut it to just keep CompletedGroup for historical tracking if needed)

```rust
// DELETE: Phase enum (lines 4-12)
// DELETE: PhaseInfo struct (lines 48-54)
// DELETE: WorkflowState struct (lines 63-76)
// DELETE: WorkflowState::new() with 5 phases (lines 78-141)
// KEEP (optional): CompletedGroup for historical metrics (lines 56-61)
```

#### 1.2 Update lib.rs Exports

**File**: `scud-cli/src/lib.rs`
**Changes**: Remove workflow module export or mark as deprecated

#### 1.3 Remove Workflow State from Storage

**File**: `scud-cli/src/storage/mod.rs`
**Changes**:
- Remove `load_workflow_state()` function
- Remove `save_workflow_state()` function
- Remove `workflow-state.json` path constant
- Keep active_group as simple Option<String> in memory or config

```rust
// DELETE: WORKFLOW_STATE_PATH constant
// DELETE: load_workflow_state() (lines ~280-310)
// DELETE: save_workflow_state() (lines ~312-340)
// MODIFY: get_active_group() to read from config.toml instead
```

#### 1.4 Update Init Command

**File**: `scud-cli/src/commands/init.rs`
**Changes**: Don't create workflow-state.json on init

```rust
// DELETE: workflow state initialization
// KEEP: .scud/ directory creation
// KEEP: config.toml creation
// KEEP: tasks/ directory creation
```

#### 1.5 Remove Phase-Dependent Commands

**File**: `scud-cli/src/commands/` (multiple files)
**Changes**: Remove phase checks from commands that have them

Commands to audit:
- `next.rs` - Remove any phase validation
- `list.rs` - Remove phase filtering if present
- `stats.rs` - Remove phase-based stats

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds in scud-cli/
- [ ] `cargo test` passes in scud-cli/
- [ ] `scud init` creates `.scud/` without `workflow-state.json`
- [ ] `scud next --tag test` works without phase check
- [ ] `scud list --tag test` works without phase check

#### Manual Verification:
- [ ] Create tasks, run `scud next`, verify it returns ready tasks
- [ ] No "phase gate" errors appear anywhere

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: Remove Agent Role Commands

### Overview
Delete the 5 agent persona slash commands and their associated phase-gated behavior. Keep the simple task management commands.

### Changes Required:

#### 2.1 Delete Agent Command Files

**Directory**: `.claude/commands/scud/`
**Changes**: Delete these files entirely:

```bash
# DELETE these files:
.claude/commands/scud/pm.md
.claude/commands/scud/sm.md
.claude/commands/scud/architect.md
.claude/commands/scud/dev.md
.claude/commands/scud/retrospective.md
.claude/commands/scud/status.md  # Shows workflow phase - no longer needed
```

#### 2.2 Keep Task Management Commands

**Directory**: `.claude/commands/scud/`
**Changes**: Keep these files (they're phase-agnostic):

```bash
# KEEP these files:
.claude/commands/scud/task-list.md
.claude/commands/scud/task-next.md
.claude/commands/scud/task-show.md
.claude/commands/scud/task-status.md
.claude/commands/scud/task-claim.md
.claude/commands/scud/task-waves.md
.claude/commands/scud/task-stats.md
.claude/commands/scud/task-whois.md
.claude/commands/scud/task-tags.md
.claude/commands/scud/task-doctor.md
```

#### 2.3 Update Remaining Command Docs

**Files**: All remaining `.claude/commands/scud/*.md` files
**Changes**: Remove any references to phases or agent roles

Example updates needed:
- Remove "Phase: implementation" from headers
- Remove "Run /scud:pm first" suggestions
- Remove workflow state references

### Success Criteria:

#### Automated Verification:
- [ ] Only task-* commands exist in `.claude/commands/scud/`
- [ ] No files reference `/scud:pm`, `/scud:sm`, `/scud:architect`, `/scud:dev`, `/scud:retrospective`
- [ ] `grep -r "workflow-state" .claude/` returns no results

#### Manual Verification:
- [ ] `/scud:task-list` works in Claude Code
- [ ] `/scud:task-next` works in Claude Code
- [ ] No "agent" or "phase" language in remaining commands

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Simplify MCP Server

### Overview
Remove workflow-related MCP tools and resources. Keep task management tools.

### Changes Required:

#### 3.1 Remove Workflow Resources

**File**: `scud-mcp/src/resources/workflow.ts`
**Changes**: Delete the entire file

**File**: `scud-mcp/src/index.ts`
**Changes**: Remove workflow resource imports and handlers

```typescript
// DELETE: import { WORKFLOW_RESOURCES, handleWorkflowResource } from './resources/workflow.js';
// DELETE: ...WORKFLOW_RESOURCES from ALL_RESOURCES
// DELETE: workflow resource handler in ReadResourceRequestSchema
```

#### 3.2 Simplify Phase Tools

**File**: `scud-mcp/src/tools/phase.ts`
**Changes**: Rename to `tags.ts`, remove phase-specific logic

Keep only:
- `scud_tags` - List tags
- `scud_use_tag` - Set active tag (store in config, not workflow state)

#### 3.3 Update Index Exports

**File**: `scud-mcp/src/index.ts`
**Changes**: Update imports to reflect removed/renamed files

### Success Criteria:

#### Automated Verification:
- [ ] `npm run build` succeeds in scud-mcp/
- [ ] `npm test` passes in scud-mcp/ (if tests exist)
- [ ] No `workflow` references in scud-mcp/src/

#### Manual Verification:
- [ ] MCP server starts without errors
- [ ] `scud_tags` tool works
- [ ] `scud_list` tool works

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Add Claude Code Hooks Infrastructure

### Overview
Add CLI commands for installing/managing Claude Code hooks. This is the foundation for bulletproof task completion.

### Changes Required:

#### 4.1 Add Hooks Module

**File**: `scud-cli/src/commands/hooks.rs` (NEW)
**Changes**: Create new file with hook management commands

```rust
use std::fs;
use std::path::PathBuf;
use serde_json::{json, Value};

pub fn run(action: &str) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "install" => install_hooks()?,
        "uninstall" => uninstall_hooks()?,
        "status" => show_status()?,
        _ => println!("Usage: scud hooks [install|uninstall|status]"),
    }
    Ok(())
}

fn get_settings_path() -> PathBuf {
    PathBuf::from(".claude/settings.local.json")
}

fn install_hooks() -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = get_settings_path();

    // Ensure .claude directory exists
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Load existing settings or create new
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        json!({})
    };

    // Add hooks configuration
    let hooks = json!({
        "Stop": [{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "scud _hook-complete"
            }]
        }],
        "PreToolUse": [{
            "matcher": "Task",
            "hooks": [{
                "type": "command",
                "command": "scud _hook-session-start"
            }]
        }]
    });

    settings["hooks"] = hooks;

    // Write back
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    println!("✓ Claude Code hooks installed");
    println!("  Stop hook: Enforces task completion");
    println!("  PreToolUse hook: Handles session claiming");
    Ok(())
}

fn uninstall_hooks() -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = get_settings_path();

    if !settings_path.exists() {
        println!("No hooks installed");
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    if let Some(obj) = settings.as_object_mut() {
        obj.remove("hooks");
    }

    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    println!("✓ Claude Code hooks uninstalled");
    Ok(())
}

fn show_status() -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = get_settings_path();

    if !settings_path.exists() {
        println!("Hooks: NOT INSTALLED");
        println!("\nRun: scud hooks install");
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let settings: Value = serde_json::from_str(&content)?;

    if settings.get("hooks").is_some() {
        println!("Hooks: INSTALLED");
        println!("\nActive hooks:");
        println!("  • Stop → scud _hook-complete");
        println!("  • PreToolUse → scud _hook-session-start");
    } else {
        println!("Hooks: NOT INSTALLED");
        println!("\nRun: scud hooks install");
    }

    Ok(())
}
```

#### 4.2 Add Internal Hook Commands

**File**: `scud-cli/src/commands/hook_complete.rs` (NEW)
**Changes**: Create internal command called by Stop hook

```rust
use crate::storage::Storage;
use std::env;
use std::fs;

/// Called by Claude Code Stop hook to enforce task completion.
/// Extracts task context and ensures it's marked complete.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Try to get task ID from environment or recent context
    let task_id = get_current_task_id()?;

    if let Some(id) = task_id {
        let storage = Storage::new()?;

        // Get active tag
        let tag = storage.get_active_group()
            .ok_or("No active tag set")?;

        // Load and complete the task
        storage.update_group(&tag, |phase| {
            if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == id) {
                if task.status != crate::models::task::TaskStatus::Done {
                    task.status = crate::models::task::TaskStatus::Done;
                    task.release(); // Clear any lock
                    eprintln!("✓ Task {} marked complete by hook", id);
                }
            }
            Ok(())
        })?;
    } else {
        // No task context - this is fine, just log
        eprintln!("Hook: No active task to complete");
    }

    Ok(())
}

fn get_current_task_id() -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Check environment variable first
    if let Ok(id) = env::var("SCUD_TASK_ID") {
        return Ok(Some(id));
    }

    // Check .scud/current-task file
    let current_task_path = ".scud/current-task";
    if let Ok(id) = fs::read_to_string(current_task_path) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Ok(Some(id));
        }
    }

    Ok(None)
}
```

#### 4.3 Add Session Start Hook Command

**File**: `scud-cli/src/commands/hook_session_start.rs` (NEW)
**Changes**: Create internal command for session claiming

```rust
use crate::storage::Storage;
use std::env;
use std::fs;

/// Called by Claude Code PreToolUse hook (Task matcher).
/// Claims the task and sets up session context.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let task_id = get_task_from_context()?;

    if let Some(id) = task_id {
        let session_id = get_session_id();

        // Write current task for later hooks
        fs::write(".scud/current-task", &id)?;

        let storage = Storage::new()?;
        let tag = storage.get_active_group()
            .ok_or("No active tag set")?;

        // Claim the task
        storage.update_group(&tag, |phase| {
            if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == id) {
                match task.claim(&session_id) {
                    Ok(()) => eprintln!("✓ Task {} claimed for session {}", id, session_id),
                    Err(e) => eprintln!("! Task {} already claimed: {}", id, e),
                }
            }
            Ok(())
        })?;
    }

    Ok(())
}

fn get_task_from_context() -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Check environment
    if let Ok(id) = env::var("SCUD_TASK_ID") {
        return Ok(Some(id));
    }

    // Check stdin for hook payload (Claude Code sends JSON)
    // For now, rely on environment variable
    Ok(None)
}

fn get_session_id() -> String {
    env::var("CLAUDE_SESSION_ID")
        .unwrap_or_else(|_| format!("session-{}", std::process::id()))
}
```

#### 4.4 Update Main CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add hooks subcommand and internal hook commands

```rust
// Add to Command enum or match statement:
.subcommand(
    Command::new("hooks")
        .about("Manage Claude Code hooks")
        .arg(Arg::new("action")
            .help("install, uninstall, or status")
            .required(true))
)
.subcommand(
    Command::new("_hook-complete")
        .about("Internal: Called by Stop hook")
        .hide(true)
)
.subcommand(
    Command::new("_hook-session-start")
        .about("Internal: Called by PreToolUse hook")
        .hide(true)
)

// Add to match handlers:
Some(("hooks", sub)) => {
    let action = sub.get_one::<String>("action").unwrap();
    commands::hooks::run(action)?;
}
Some(("_hook-complete", _)) => {
    commands::hook_complete::run()?;
}
Some(("_hook-session-start", _)) => {
    commands::hook_session_start::run()?;
}
```

#### 4.5 Update lib.rs

**File**: `scud-cli/src/lib.rs`
**Changes**: Add new command modules

```rust
pub mod commands {
    // ... existing ...
    pub mod hooks;
    pub mod hook_complete;
    pub mod hook_session_start;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `scud hooks status` shows "NOT INSTALLED"
- [ ] `scud hooks install` creates `.claude/settings.local.json` with hooks
- [ ] `scud hooks status` shows "INSTALLED"
- [ ] `scud hooks uninstall` removes hooks from settings
- [ ] `scud _hook-complete` runs without error (when no task context)

#### Manual Verification:
- [ ] Create a task, set SCUD_TASK_ID env var, run `scud _hook-complete`, verify task is done
- [ ] `.claude/settings.local.json` contains valid hook configuration

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 5: Add Orchestrator Support Commands

### Overview
Add commands to support orchestrator patterns: spawning multiple agents, tracking active work, and session management.

### Changes Required:

#### 5.1 Enhanced Next Command

**File**: `scud-cli/src/commands/next.rs`
**Changes**: Add `--spawn` flag that outputs machine-readable format for orchestrators

```rust
// Add new flag:
.arg(Arg::new("spawn")
    .long("spawn")
    .help("Output machine-readable format for orchestrator")
    .action(ArgAction::SetTrue))

// Add new output mode:
if args.get_flag("spawn") {
    // Output JSON for orchestrators
    if let NextTaskResult::Available(task) = result {
        println!("{}", serde_json::json!({
            "task_id": task.id,
            "title": task.title,
            "tag": tag,
            "complexity": task.complexity
        }));
    } else {
        println!("null");
    }
}
```

#### 5.2 Add Batch Next Command

**File**: `scud-cli/src/commands/next_batch.rs` (NEW)
**Changes**: Get multiple ready tasks at once for parallel spawning

```rust
use crate::storage::Storage;
use crate::models::task::TaskStatus;

pub fn run(tag: &str, limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::new()?;
    let phase = storage.load_group(tag)?;

    let ready_tasks: Vec<_> = phase.tasks.iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .filter(|t| t.has_dependencies_met(&phase.tasks))
        .filter(|t| !t.is_locked())
        .take(limit)
        .collect();

    let output = serde_json::json!({
        "tag": tag,
        "count": ready_tasks.len(),
        "tasks": ready_tasks.iter().map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "complexity": t.complexity
            })
        }).collect::<Vec<_>>()
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
```

#### 5.3 Add Active Sessions Command

**File**: `scud-cli/src/commands/sessions.rs` (NEW)
**Changes**: Show active task sessions (claimed tasks with timing)

```rust
use crate::storage::Storage;

pub fn run(tag: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::new()?;

    let tags = if let Some(t) = tag {
        vec![t.to_string()]
    } else {
        storage.list_tags()?
    };

    println!("Active Sessions:");
    println!();

    for tag in tags {
        let phase = storage.load_group(&tag)?;

        for task in phase.tasks.iter().filter(|t| t.is_locked()) {
            let age = task.lock_age_hours().unwrap_or(0.0);
            let stale = if age > 1.0 { " (STALE)" } else { "" };

            println!("  {} | {} | locked by: {} | {:.1}h{}",
                task.id,
                task.title,
                task.locked_by.as_deref().unwrap_or("unknown"),
                age,
                stale
            );
        }
    }

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `scud next --spawn --tag test` outputs JSON or "null"
- [ ] `scud next-batch --tag test --limit 5` outputs JSON array
- [ ] `scud sessions` runs without error

#### Manual Verification:
- [ ] Claim a task, run `scud sessions`, see it listed
- [ ] `scud next-batch` returns multiple ready tasks

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 6: Update Documentation

### Overview
Update all documentation to reflect the new DAG-driven, hook-enforced model.

### Changes Required:

#### 6.1 Update README

**File**: `README.md`
**Changes**:
- Remove workflow/phase language
- Add hooks documentation
- Add orchestrator pattern examples
- Emphasize DAG-driven execution

#### 6.2 Update CLAUDE.md

**File**: `.taskmaster/CLAUDE.md` (if exists)
**Changes**: Remove references to workflow phases and agent roles

#### 6.3 Create Orchestrator Guide

**File**: `docs/orchestrator.md` (NEW)
**Changes**: Document the orchestrator pattern

```markdown
# SCUD Orchestrator Pattern

## Overview

SCUD uses DAG-driven execution: tasks become ready when their dependencies complete.
An orchestrator spawns agents for ready tasks and loops until work is done.

## Basic Loop

```bash
#!/bin/bash
# Simple orchestrator loop

scud hooks install  # Ensure hooks are active

while true; do
    TASK=$(scud next --spawn --tag myproject)

    if [ "$TASK" = "null" ]; then
        echo "No more ready tasks"
        break
    fi

    TASK_ID=$(echo $TASK | jq -r .task_id)

    # Spawn agent with task context
    SCUD_TASK_ID=$TASK_ID claude-code "Implement task $TASK_ID" &
done

wait  # Wait for all agents
```

## Parallel Spawning

```bash
# Spawn N agents for ready tasks
TASKS=$(scud next-batch --tag myproject --limit 4)

echo $TASKS | jq -c '.tasks[]' | while read task; do
    TASK_ID=$(echo $task | jq -r .id)
    SCUD_TASK_ID=$TASK_ID claude-code "Implement task $TASK_ID" &
done
```

## Hooks Ensure Completion

Claude Code Stop hooks automatically call `scud _hook-complete` when a session ends.
This marks the task as done even if the agent forgets.

## Monitoring

```bash
scud sessions           # See active work
scud whois --tag proj   # Who's working on what
scud stats --tag proj   # Completion progress
```
```

#### 6.4 Update Existing Command Docs

**Files**: `.claude/commands/scud/task-*.md`
**Changes**: Remove phase references, add hook information

### Success Criteria:

#### Automated Verification:
- [ ] No "workflow" or "phase gate" language in README
- [ ] No agent role references in documentation
- [ ] `docs/orchestrator.md` exists

#### Manual Verification:
- [ ] Documentation accurately describes new behavior
- [ ] Orchestrator guide is clear and actionable

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Testing Strategy

### Unit Tests:
- Test `scud next` returns tasks based on dependency satisfaction only
- Test hook installation/uninstallation
- Test `_hook-complete` marks task done
- Test `next-batch` returns correct subset

### Integration Tests:
- Full flow: create tasks → claim → complete via hook
- Orchestrator loop simulation
- Parallel claiming doesn't cause conflicts

### Manual Testing Steps:
1. Initialize new SCUD project: `scud init`
2. Verify no `workflow-state.json` created
3. Create tasks with dependencies
4. Run `scud next` - verify returns ready tasks
5. Install hooks: `scud hooks install`
6. Verify `.claude/settings.local.json` has hooks
7. Simulate hook call: `SCUD_TASK_ID=1 scud _hook-complete`
8. Verify task marked done

## Performance Considerations

- Hook commands must be fast (<100ms) to not slow Claude Code
- `next-batch` should efficiently query ready tasks
- File locking on hook calls prevents race conditions

## Migration Notes

### For Existing SCUD Projects:
1. Delete `.scud/workflow-state.json` (no longer needed)
2. Run `scud hooks install` to enable enforcement
3. Delete any agent-specific scripts

### Breaking Changes:
- `/scud:pm`, `/scud:sm`, etc. commands removed
- `workflow-state.json` no longer created/read
- MCP `scud://workflow/*` resources removed

## References

- PRD: `thoughts/user/refactor.md`
- Beads comparison: Lines 16-51 of refactor.md
- Hook design: Lines 545-632 of refactor.md
- Orchestrator pattern: Lines 386-456 of refactor.md
