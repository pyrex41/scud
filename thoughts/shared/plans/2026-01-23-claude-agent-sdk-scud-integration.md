# Claude Agent SDK + SCUD Deep Integration Implementation Plan

## Overview

Integrate SCUD with Claude Code's Task system and Agent SDK to enable seamless coordination between SCUD's DAG-based task management and Claude Code's agentic execution. This moves beyond the current "separate terminal" approach where SCUD and Claude Code operate independently.

The key insight: Claude Code now has its own Tasks feature (`~/.claude/tasks/`) that Claude can read/write during execution. By syncing SCUD tasks to this location and installing hooks that propagate changes back to SCUD, we create a unified task management experience where:
- SCUD provides the DAG, dependencies, waves, and orchestration
- Claude Code agents see tasks natively via their TaskList/TaskUpdate tools
- Changes agents make to tasks automatically sync back to SCUD

## Current State Analysis

### SCUD's Current Claude Code Integration

SCUD spawns Claude Code via CLI command (`scud-cli/src/commands/spawn/terminal.rs:48-57`):
```bash
'claude' "$(cat prompt.txt)" --dangerously-skip-permissions --model <model>
```

Task completion is detected via a Stop hook in `.claude/settings.local.json` that runs:
```bash
scud set-status "$SCUD_TASK_ID" done
```

This works but has limitations:
- Claude agents don't see the full task list or dependencies
- No visibility into what other agents are working on
- Manual environment variable passing for task context
- Shell-based hooks have limited context

### Claude Code's New Tasks System

Claude Code now has built-in task management:
- Tasks stored in `~/.claude/tasks/<list-id>.json`
- Environment variable `CLAUDE_CODE_TASK_LIST_ID` sets the active list
- Agents can use `TaskCreate`, `TaskUpdate`, `TaskList`, `TaskGet` tools
- Tasks have `blockedBy`/`blocks` for dependencies
- Multiple sessions can share a task list

### Key Discoveries

1. **Claude Tasks JSON format** (`~/.claude/tasks/<id>.json`):
```json
{
  "tasks": [
    {
      "id": "1",
      "subject": "Task title",
      "description": "Details",
      "status": "pending|in_progress|completed",
      "blockedBy": ["other-task-id"],
      "blocks": ["dependent-task-id"],
      "owner": "agent-name",
      "metadata": {}
    }
  ]
}
```

2. **SCUD task status mapping**:
   - `Pending` → `pending`
   - `InProgress` → `in_progress`
   - `Done` → `completed`
   - `Failed`, `Blocked`, `Deferred`, `Cancelled` → Need custom handling

3. **Existing SCUD hook installation** (`hooks.rs:53-100`): Already modifies `.claude/settings.local.json`

4. **Existing JSON-RPC server** (`rpc/server.rs`): Can be extended for SDK communication

## Desired End State

After implementation:

1. **`scud run <task_id>`** (renamed from `spawn`):
   - Syncs SCUD tasks to `~/.claude/tasks/scud-<tag>.json`
   - Sets `CLAUDE_CODE_TASK_LIST_ID=scud-<tag>`
   - Spawns Claude Code with task context
   - Claude sees full task list via native TaskList tool
   - PostToolUse hook syncs any TaskUpdate calls back to SCUD

2. **`scud swarm --tag <tag>`**:
   - Same sync behavior for all tasks in wave
   - Each agent shares the same task list ID
   - Agents can see what others are working on
   - Completion and failures sync back automatically

3. **Hook Integration**:
   - Claude Code hooks installed when in SCUD directory
   - `PostToolUse` hook intercepts `TaskUpdate`/`TaskCreate` calls
   - Changes propagate to SCUD via `scud set-status` or direct file update

### Verification

- [ ] `scud run <task_id>` spawns agent with `CLAUDE_CODE_TASK_LIST_ID` set
- [ ] Agent can call `TaskList` and see SCUD tasks
- [ ] Agent calling `TaskUpdate` triggers sync back to SCUD
- [ ] `scud swarm` works with multiple agents sharing task list
- [ ] Task status changes in Claude reflect in `scud list`

## What We're NOT Doing

- **MCP Server**: Deferred. CLI-based integration is sufficient for now.
- **SDK TypeScript bindings from Rust**: Too complex. Use CLI + hooks instead.
- **Real-time bidirectional sync**: SCUD is source of truth. Sync is one-way with hook-based feedback.
- **Session resume**: Deferred to future phase. Focus on core sync first.
- **Replacing tmux mode**: Keeping tmux as primary execution mode. SDK direct mode deferred.

## Implementation Approach

1. Add task sync service that writes SCUD tasks to Claude Tasks format
2. Install enhanced hooks that sync TaskUpdate calls back to SCUD
3. Rename `spawn` to `run` for single-task execution
4. Update `swarm` to use shared task list
5. Test both modes work correctly with sync

---

## Phase 1: Task Sync Service

### Overview
Create a sync service that exports SCUD tasks to Claude Tasks JSON format, enabling Claude Code agents to see SCUD tasks via their native TaskList tool.

### Changes Required:

#### 1.1 Claude Tasks Data Model

**File**: `scud-cli/src/sync/mod.rs` (new)
**Changes**: Create module for Claude Tasks sync

```rust
//! Sync SCUD tasks to Claude Code's Tasks format

mod claude_tasks;

pub use claude_tasks::*;
```

**File**: `scud-cli/src/sync/claude_tasks.rs` (new)
**Changes**: Define Claude Tasks JSON structures and sync logic

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use anyhow::Result;

use scud_core::models::{Task, TaskStatus, Phase};

/// Claude Code task format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTask {
    pub id: String,
    pub subject: String,
    #[serde(default)]
    pub description: String,
    pub status: String,  // "pending", "in_progress", "completed"
    #[serde(default, rename = "blockedBy")]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Claude Code task list format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTaskList {
    pub tasks: Vec<ClaudeTask>,
}

impl ClaudeTask {
    /// Convert SCUD task to Claude task format
    pub fn from_scud_task(task: &Task, tag: &str) -> Self {
        let status = match task.status {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Done => "completed",
            // Map other statuses to pending with metadata flag
            TaskStatus::Blocked | TaskStatus::Deferred => "pending",
            TaskStatus::Failed | TaskStatus::Cancelled => "completed",
            TaskStatus::Review => "in_progress",
            TaskStatus::Expanded => "completed",
        };

        // Compute blocks (reverse dependencies)
        // Note: This requires the full task list, handled at sync level

        ClaudeTask {
            id: format!("{}:{}", tag, task.id),
            subject: task.title.clone(),
            description: task.description.clone(),
            status: status.to_string(),
            blocked_by: task.dependencies.iter()
                .map(|d| format!("{}:{}", tag, d))
                .collect(),
            blocks: vec![],  // Filled in by sync_phase
            owner: task.assigned_to.clone(),
            metadata: serde_json::json!({
                "scud_tag": tag,
                "scud_status": format!("{:?}", task.status),
                "complexity": task.complexity,
                "priority": format!("{:?}", task.priority),
            }),
        }
    }
}

/// Get Claude tasks directory
pub fn claude_tasks_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("tasks")
}

/// Generate task list ID for a SCUD tag
pub fn task_list_id(tag: &str) -> String {
    format!("scud-{}", tag)
}

/// Sync a SCUD phase to Claude Tasks format
pub fn sync_phase(phase: &Phase, tag: &str) -> Result<PathBuf> {
    let tasks_dir = claude_tasks_dir();
    std::fs::create_dir_all(&tasks_dir)?;

    let list_id = task_list_id(tag);
    let task_file = tasks_dir.join(format!("{}.json", list_id));

    // Build dependency reverse map for "blocks" field
    let mut blocks_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for task in phase.tasks.iter() {
        let task_full_id = format!("{}:{}", tag, task.id);
        for dep in &task.dependencies {
            let dep_full_id = format!("{}:{}", tag, dep);
            blocks_map.entry(dep_full_id)
                .or_default()
                .push(task_full_id.clone());
        }
    }

    // Convert tasks
    let claude_tasks: Vec<ClaudeTask> = phase.tasks.iter()
        .filter(|t| !t.is_expanded())  // Skip expanded parent tasks
        .map(|t| {
            let mut ct = ClaudeTask::from_scud_task(t, tag);
            let full_id = format!("{}:{}", tag, t.id);
            ct.blocks = blocks_map.get(&full_id).cloned().unwrap_or_default();
            ct
        })
        .collect();

    let task_list = ClaudeTaskList { tasks: claude_tasks };
    let json = serde_json::to_string_pretty(&task_list)?;
    std::fs::write(&task_file, json)?;

    Ok(task_file)
}

/// Sync multiple phases (for --all-tags mode)
pub fn sync_phases(phases: &std::collections::HashMap<String, Phase>) -> Result<Vec<PathBuf>> {
    phases.iter()
        .map(|(tag, phase)| sync_phase(phase, tag))
        .collect()
}
```

#### 1.2 Update lib.rs exports

**File**: `scud-cli/src/lib.rs`
**Changes**: Add sync module export

```rust
// Add to existing module declarations
pub mod sync;
```

#### 1.3 Integration with spawn/run command

**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Call sync before spawning agents

Add import at top:
```rust
use crate::sync::claude_tasks;
```

Add sync call before spawning loop (around line 143):
```rust
// Sync tasks to Claude format
let task_list_id = claude_tasks::task_list_id(&phase_tag);
if let Ok(sync_path) = claude_tasks::sync_phase(&phase, &phase_tag) {
    println!("Synced tasks to: {}", sync_path.display());
}
```

#### 1.4 Update terminal spawning to set task list ID

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add `CLAUDE_CODE_TASK_LIST_ID` to spawn script

In `spawn_tmux()` function (around line 306-328), update the script template:
```rust
let spawn_script = format!(
    r#"#!/usr/bin/env bash
# Source shell profile for PATH setup
source ~/.bash_profile 2>/dev/null
source ~/.zshrc 2>/dev/null
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
[ -s "$HOME/.nvm/nvm.sh" ] && source "$HOME/.nvm/nvm.sh"

export SCUD_TASK_ID='{task_id}'
export CLAUDE_CODE_TASK_LIST_ID='{task_list_id}'
{harness_cmd}
rm -f '{prompt_file}'
"#,
    task_id = task_id,
    task_list_id = task_list_id,  // New parameter
    harness_cmd = harness_cmd,
    prompt_file = prompt_file.display()
);
```

Update function signature to accept task_list_id:
```rust
fn spawn_tmux(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    binary_path: &str,
    harness: Harness,
    model: Option<&str>,
    task_list_id: &str,  // New parameter
) -> Result<String>
```

### Success Criteria:

#### Automated Verification:
- [x] Build succeeds: `cargo build -p scud-cli`
- [x] Tests pass: `cargo test -p scud-cli`
- [ ] Sync creates valid JSON: `cat ~/.claude/tasks/scud-test.json | jq .`
- [ ] Task list ID is set: Check tmux environment with `tmux show-environment`

#### Manual Verification:
- [ ] Run `scud spawn --tag test` and verify `~/.claude/tasks/scud-test.json` is created
- [ ] Verify JSON contains tasks with correct status mapping
- [ ] Inside spawned Claude session, `TaskList` shows SCUD tasks
- [ ] Dependencies appear correctly in `blockedBy` field

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that Claude Code can see SCUD tasks via TaskList before proceeding to Phase 2.

---

## Phase 2: Hook-Based Sync Back to SCUD

### Overview
Install Claude Code hooks that intercept `TaskUpdate` and `TaskCreate` tool calls, syncing changes back to SCUD. This ensures SCUD remains the source of truth while allowing Claude to make task updates.

### Changes Required:

#### 2.1 Enhanced Hook Installation

**File**: `scud-cli/src/commands/spawn/hooks.rs`
**Changes**: Add PostToolUse hook for task sync

Update `install_hooks()` function:

```rust
pub fn install_hooks(project_root: &Path) -> Result<()> {
    let claude_dir = project_root.join(".claude");
    let settings_path = claude_dir.join("settings.local.json");

    std::fs::create_dir_all(&claude_dir)?;

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Stop hook - mark task done when agent stops
    let stop_hook = json!([{
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": "bash -c 'if [ -n \"$SCUD_TASK_ID\" ]; then scud set-status \"$SCUD_TASK_ID\" done 2>/dev/null || true; fi'",
            "timeout": 10
        }]
    }]);

    // PostToolUse hook - sync TaskUpdate/TaskCreate back to SCUD
    let post_tool_hook = json!([{
        "matcher": "TaskUpdate|TaskCreate",
        "hooks": [{
            "type": "command",
            "command": "bash -c 'scud sync-from-claude 2>/dev/null || true'",
            "timeout": 10
        }]
    }]);

    if settings["hooks"].is_null() {
        settings["hooks"] = json!({});
    }
    settings["hooks"]["Stop"] = stop_hook;
    settings["hooks"]["PostToolUse"] = post_tool_hook;

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    Ok(())
}
```

#### 2.2 New sync-from-claude command

**File**: `scud-cli/src/commands/sync_from_claude.rs` (new)
**Changes**: Command to sync Claude task changes back to SCUD

```rust
use anyhow::Result;
use std::path::Path;
use std::collections::HashMap;

use scud_core::Storage;
use scud_core::models::TaskStatus;
use crate::sync::claude_tasks::{claude_tasks_dir, ClaudeTaskList};

/// Sync task status changes from Claude Tasks back to SCUD
pub fn run(project_root: &Path) -> Result<()> {
    let storage = Storage::new(project_root.to_path_buf());

    if !storage.is_initialized() {
        return Ok(());  // Silently exit if not a SCUD project
    }

    let tasks_dir = claude_tasks_dir();
    if !tasks_dir.exists() {
        return Ok(());
    }

    // Find all scud-* task files
    for entry in std::fs::read_dir(&tasks_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
            if !name.starts_with("scud-") {
                continue;
            }

            let tag = name.strip_prefix("scud-").unwrap();

            // Load Claude task list
            let content = std::fs::read_to_string(&path)?;
            let claude_list: ClaudeTaskList = serde_json::from_str(&content)?;

            // Load SCUD phase
            let mut phase = match storage.load_group(tag) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let mut changed = false;

            // Sync status changes
            for claude_task in &claude_list.tasks {
                // Parse task ID (format: "tag:id")
                let task_id = claude_task.id
                    .strip_prefix(&format!("{}:", tag))
                    .unwrap_or(&claude_task.id);

                if let Some(scud_task) = phase.get_task_mut(task_id) {
                    let new_status = match claude_task.status.as_str() {
                        "pending" => TaskStatus::Pending,
                        "in_progress" => TaskStatus::InProgress,
                        "completed" => TaskStatus::Done,
                        _ => continue,
                    };

                    if scud_task.status != new_status {
                        scud_task.set_status(new_status);
                        changed = true;
                    }
                }
            }

            if changed {
                storage.update_group(tag, &phase)?;
            }
        }
    }

    Ok(())
}
```

#### 2.3 Register command in main.rs

**File**: `scud-cli/src/main.rs`
**Changes**: Add sync-from-claude subcommand

Add to Commands enum:
```rust
/// Sync task changes from Claude Tasks back to SCUD (internal use)
#[command(hide = true)]
SyncFromClaude,
```

Add to match block:
```rust
Commands::SyncFromClaude => {
    commands::sync_from_claude::run(&project_root)?;
}
```

#### 2.4 Add module to commands/mod.rs

**File**: `scud-cli/src/commands/mod.rs`
**Changes**: Export new command

```rust
pub mod sync_from_claude;
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] Tests pass: `cargo test -p scud-cli`
- [ ] Hook file contains PostToolUse matcher: `cat .claude/settings.local.json | jq '.hooks.PostToolUse'`

#### Manual Verification:
- [ ] Spawn agent with `scud spawn --tag test`
- [ ] Inside agent, run `TaskUpdate` to change a task status
- [ ] Verify `scud list --tag test` shows updated status
- [ ] Verify hook is triggered (check logs or add debug output)

**Implementation Note**: After completing this phase, manually verify the full round-trip: SCUD → Claude Tasks → Agent modifies → Hook → SCUD updated.

---

## Phase 3: Rename spawn to run

### Overview
Rename the `spawn` command to `run` for single-task execution. This clarifies the distinction between `run` (single task) and `swarm` (parallel wave execution).

### Changes Required:

#### 3.1 Add run as alias

**File**: `scud-cli/src/main.rs`
**Changes**: Add `run` as primary command, keep `spawn` as hidden alias

In Commands enum, update:
```rust
/// Run agent(s) on ready tasks
#[command(alias = "spawn")]  // Keep spawn as alias for backwards compat
Run {
    /// Limit number of tasks to spawn
    #[arg(short, long, default_value = "5")]
    limit: usize,

    /// Phase tag (uses active tag if not specified)
    #[arg(short, long)]
    tag: Option<String>,

    // ... rest of existing spawn args
},
```

Or alternatively, add both commands pointing to same handler:
```rust
/// Run agent(s) on ready tasks (single task or batch)
Run { /* args */ },

/// [Deprecated] Use 'run' instead
#[command(hide = true)]
Spawn { /* same args */ },
```

#### 3.2 Update help text and documentation

**File**: `scud-cli/src/main.rs`
**Changes**: Update command descriptions

```rust
/// Run a single agent on the next available task
///
/// Examples:
///   scud run              # Run next available task
///   scud run --limit 3    # Run up to 3 tasks
///   scud run auth:5       # Run specific task
Run { ... }
```

#### 3.3 Update CLAUDE.md references

**File**: `CLAUDE.md`
**Changes**: Update spawn references to run

### Success Criteria:

#### Automated Verification:
- [ ] `scud run --help` shows correct documentation
- [ ] `scud spawn --help` still works (alias)
- [ ] `scud run` executes successfully

#### Manual Verification:
- [ ] Tab completion shows `run` as option
- [ ] Existing scripts using `spawn` continue to work

---

## Phase 4: Update swarm to use shared task list

### Overview
Update the swarm command to sync tasks and set `CLAUDE_CODE_TASK_LIST_ID` for all agents in a wave, enabling agents to see each other's progress.

### Changes Required:

#### 4.1 Add sync to swarm execution

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Sync before each wave and set task list ID

Add import:
```rust
use crate::sync::claude_tasks;
```

In `run()` function, before wave execution (around line 339):
```rust
// Sync tasks to Claude format before starting wave
let task_list_id = claude_tasks::task_list_id(&phase_tag);
if let Err(e) = claude_tasks::sync_phase(&phase, &phase_tag) {
    println!("Warning: Failed to sync tasks to Claude format: {}", e);
}
```

#### 4.2 Pass task_list_id to spawning functions

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Update `execute_round()` to pass task list ID

Update function signature:
```rust
fn execute_round(
    round_tasks: &[TaskInfo],
    round_idx: usize,
    session_name: &str,
    working_dir: &Path,
    harness: Harness,
    model: Option<&str>,
    task_list_id: &str,  // New parameter
) -> Result<RoundState>
```

Pass to terminal spawn:
```rust
terminal::spawn_terminal_with_harness_and_model(
    &info.task.id,
    &config.prompt,
    working_dir,
    session_name,
    config.harness,
    config.model.as_deref(),
    task_list_id,  // Pass task list ID
)?;
```

#### 4.3 Re-sync after each wave

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Re-sync after wave completion to capture any status changes

After wave completion (around line 549):
```rust
// Re-sync tasks after wave to capture any changes
if let Err(e) = claude_tasks::sync_phase(&phase, &phase_tag) {
    println!("Warning: Failed to re-sync tasks: {}", e);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] Swarm tests pass: `cargo test -p scud-cli swarm`
- [ ] Task list file updated during swarm: check timestamps

#### Manual Verification:
- [ ] Run `scud swarm --tag test --limit 2`
- [ ] Verify both agents see the same task list via `TaskList`
- [ ] Agent 1 marks task done, Agent 2 can see the change
- [ ] Post-wave validation still works correctly

**Implementation Note**: Test swarm with 2+ concurrent agents to verify shared task list behavior.

---

## Phase 5: Add task prompt enhancement

### Overview
Enhance the task prompt to inform agents about the shared task list and how to use Claude's task tools.

### Changes Required:

#### 5.1 Update prompt generation

**File**: `scud-cli/src/commands/spawn/agent.rs`
**Changes**: Add task list context to prompts

In `generate_prompt()` function (around line 164-183), add:
```rust
prompt.push_str(r#"

## Shared Task Context
You are part of a SCUD-coordinated workflow. Other agents may be working on related tasks.

- Use `TaskList` to see all tasks and their current status
- Use `TaskUpdate` to update task status (this syncs back to SCUD)
- Check task dependencies before starting work
- Mark tasks as in_progress when you start, completed when done

Discovery logging is still available:
- Run `scud log <task_id> "discovery"` to share findings with other agents
- Run `scud log-all --limit 10` to see recent discoveries
"#);
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] Prompt contains TaskList instructions

#### Manual Verification:
- [ ] Spawned agent mentions TaskList in its initial context understanding
- [ ] Agent successfully uses TaskList tool
- [ ] Agent uses TaskUpdate appropriately

---

## Testing Strategy

### Unit Tests:
- `sync::claude_tasks::ClaudeTask::from_scud_task()` conversion
- `sync::claude_tasks::sync_phase()` creates valid JSON
- Status mapping: all SCUD statuses map correctly
- Dependency mapping: `blockedBy` and `blocks` computed correctly

### Integration Tests:
- End-to-end spawn with task sync
- Hook triggers on TaskUpdate
- Round-trip sync: SCUD → Claude → Hook → SCUD

### Manual Testing Steps:
1. Initialize test project: `scud init && scud generate tests/fixtures/sample.md`
2. Run single task: `scud run --limit 1`
3. Verify task list sync: `cat ~/.claude/tasks/scud-<tag>.json`
4. Inside agent, call `TaskList` and verify tasks visible
5. Call `TaskUpdate` on a task
6. Exit agent, verify `scud list` shows updated status
7. Run swarm: `scud swarm --tag <tag> --limit 2`
8. Verify both agents see shared task list
9. Verify post-swarm status is correct

## Performance Considerations

- Task sync is file-based and fast (< 100ms for typical task lists)
- Hook commands run asynchronously, don't block agent execution
- File watching not needed - sync on demand at spawn/swarm start and via hooks

## Migration Notes

- `spawn` command becomes hidden alias for `run`
- Existing scripts using `spawn` continue to work
- Hook format is backwards compatible (adds new hooks, doesn't remove Stop hook)
- No changes to `.scud/tasks/` format

## References

- Research: `thoughts/shared/research/2026-01-23-claude-agent-sdk-scud-integration.md`
- Claude Tasks announcement: See command args for context
- Existing hooks: `scud-cli/src/commands/spawn/hooks.rs`
- Existing spawn: `scud-cli/src/commands/spawn/mod.rs`
