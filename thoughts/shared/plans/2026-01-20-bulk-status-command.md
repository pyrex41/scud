# Bulk Status Change Command Implementation Plan

## Overview

Extend `scud set-status` to support bulk status changes. Two modes:
1. **Status transition mode**: `scud set-status --from in-progress --to pending` - changes all tasks matching `from` status
2. **Explicit task IDs mode**: `scud set-status done task1 task2 task3` - changes specific tasks

Both modes support tag scoping: active tag (default), `--tag <tag>`, or `--all-tags`.

## Current State Analysis

### Existing Implementation
- `src/main.rs:178-187`: SetStatus command takes single task_id, status, optional tag
- `src/commands/set_status.rs`: Simple implementation that updates one task

### Needed Changes
- Extend CLI arguments to support both modes
- Add bulk update logic
- Add `--from`/`--to` flags for transition mode
- Add `--all-tags` support

## Desired End State

1. `scud set-status done task1 task2 task3` - set multiple specific tasks to "done"
2. `scud set-status --from in-progress --to pending` - bulk transition
3. `scud set-status --from in-progress --to pending --all-tags` - across all tags
4. Backward compatible: `scud set-status task1 done` still works (single task)

### Verification:
- `cargo test` passes
- All command variants work as expected

## What We're NOT Doing

- Not adding dry-run mode (can be added later if needed)
- Not adding confirmation prompts (user knows what they're doing)
- Not adding complex filtering (just status-based)

## Implementation Approach

Extend the existing SetStatus command with optional arguments for bulk operations.

## Phase 1: Extend CLI Arguments

### Overview
Modify the SetStatus command to accept multiple task IDs and add bulk transition flags.

### Changes Required:

#### 1.1 Update SetStatus command definition

**File**: `scud-cli/src/main.rs`
**Changes**: Modify SetStatus struct to support both modes

Find the SetStatus command (around line 178) and replace with:

```rust
    /// Update task status
    ///
    /// Single task: scud set-status <task_id> <status>
    /// Multiple tasks: scud set-status <status> <task_id> [task_id...]
    /// Bulk transition: scud set-status --from <status> --to <status>
    SetStatus {
        /// Status (for multi-task mode) or task ID (for single task mode)
        first_arg: Option<String>,

        /// Task IDs (for multi-task mode) or status (for single task mode)
        rest: Vec<String>,

        /// Source status for bulk transition
        #[arg(long)]
        from: Option<String>,

        /// Target status for bulk transition
        #[arg(long)]
        to: Option<String>,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Apply to all tags
        #[arg(long)]
        all_tags: bool,
    },
```

#### 1.2 Update command dispatch

**File**: `scud-cli/src/main.rs`
**Changes**: Update the match arm for SetStatus (around line 722)

```rust
        Commands::SetStatus {
            first_arg,
            rest,
            from,
            to,
            tag,
            all_tags,
        } => commands::set_status::run(
            cli.project,
            first_arg.as_deref(),
            &rest,
            from.as_deref(),
            to.as_deref(),
            tag.as_deref(),
            all_tags,
        ),
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] CLI help shows new arguments

---

## Phase 2: Implement Bulk Status Logic

### Overview
Rewrite set_status.rs to handle all modes.

### Changes Required:

#### 2.1 Rewrite set_status.rs

**File**: `scud-cli/src/commands/set_status.rs`
**Changes**: Complete rewrite to support all modes

```rust
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::models::TaskStatus;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    first_arg: Option<&str>,
    rest: &[String],
    from: Option<&str>,
    to: Option<&str>,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Determine mode based on arguments
    match (from, to, first_arg, rest.is_empty()) {
        // Mode 1: Bulk transition (--from X --to Y)
        (Some(from_str), Some(to_str), _, _) => {
            run_bulk_transition(&storage, from_str, to_str, tag, all_tags)
        }
        // Mode 2: Multiple task IDs (status task1 task2 ...)
        (None, None, Some(first), false) => {
            // first is status, rest are task IDs
            run_multi_task(&storage, first, rest, tag, all_tags)
        }
        // Mode 3: Single task (task_id status) - backward compatible
        (None, None, Some(first), true) => {
            anyhow::bail!(
                "Missing status. Usage:\n  \
                 scud set-status <task_id> <status>\n  \
                 scud set-status <status> <task_id> [task_id...]\n  \
                 scud set-status --from <status> --to <status>"
            )
        }
        // Mode 4: Check if old syntax (task_id status) - need to detect
        // Actually, old syntax was: set-status <task_id> <status>
        // New multi-task syntax is: set-status <status> <task_id> [task_id...]
        // We need heuristics or explicit separation

        // Invalid combinations
        (Some(_), None, _, _) | (None, Some(_), _, _) => {
            anyhow::bail!("Both --from and --to must be specified together")
        }
        (None, None, None, _) => {
            anyhow::bail!(
                "Usage:\n  \
                 scud set-status <task_id> <status>\n  \
                 scud set-status <status> <task_id> [task_id...]\n  \
                 scud set-status --from <status> --to <status>"
            )
        }
    }
}

/// Bulk transition: change all tasks from one status to another
fn run_bulk_transition(
    storage: &Storage,
    from_str: &str,
    to_str: &str,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let from_status = TaskStatus::from_str(from_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid --from status: {}. Valid: {:?}",
            from_str,
            TaskStatus::all()
        )
    })?;

    let to_status = TaskStatus::from_str(to_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid --to status: {}. Valid: {:?}",
            to_str,
            TaskStatus::all()
        )
    })?;

    let tags = get_target_tags(storage, tag, all_tags)?;
    let mut total_changed = 0;

    for epic_tag in &tags {
        let mut epic = storage.load_group(epic_tag)?;
        let mut changed_in_tag = 0;

        for task in &mut epic.tasks {
            if task.status == from_status {
                task.set_status(to_status);
                println!(
                    "  {} {} → {}",
                    "✓".green(),
                    task.id.cyan(),
                    to_str.green()
                );
                changed_in_tag += 1;
            }
        }

        if changed_in_tag > 0 {
            storage.update_group(epic_tag, &epic)?;
            total_changed += changed_in_tag;
        }
    }

    if total_changed == 0 {
        println!(
            "{} No tasks found with status '{}'",
            "!".yellow(),
            from_str
        );
    } else {
        println!(
            "\n{} Changed {} task(s) from {} to {}",
            "✓".green(),
            total_changed,
            from_str.yellow(),
            to_str.green()
        );
    }

    Ok(())
}

/// Multi-task mode: set specific tasks to a status
fn run_multi_task(
    storage: &Storage,
    status_str: &str,
    task_ids: &[String],
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    // Check if this might be old-style single-task syntax
    // Old: set-status <task_id> <status> where task_ids would have 1 element (the status)
    // New: set-status <status> <task_id> [task_id...] where status_str is status

    // Heuristic: if status_str looks like a task ID (contains ':') and
    // task_ids[0] looks like a status, swap them
    if task_ids.len() == 1 && TaskStatus::from_str(&task_ids[0]).is_some()
        && TaskStatus::from_str(status_str).is_none()
    {
        // Old syntax: task_id status
        return run_single_task(storage, status_str, &task_ids[0], tag);
    }

    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let tags = get_target_tags(storage, tag, all_tags)?;
    let mut changed = 0;
    let mut not_found: Vec<String> = task_ids.to_vec();

    for epic_tag in &tags {
        let mut epic = storage.load_group(epic_tag)?;
        let mut modified = false;

        for task in &mut epic.tasks {
            if not_found.contains(&task.id) {
                task.set_status(new_status);
                println!(
                    "  {} {} → {}",
                    "✓".green(),
                    task.id.cyan(),
                    status_str.green()
                );
                not_found.retain(|id| id != &task.id);
                modified = true;
                changed += 1;
            }
        }

        if modified {
            storage.update_group(epic_tag, &epic)?;
        }
    }

    if !not_found.is_empty() {
        println!(
            "\n{} Task(s) not found: {}",
            "!".yellow(),
            not_found.join(", ")
        );
    }

    if changed > 0 {
        println!(
            "\n{} Changed {} task(s) to {}",
            "✓".green(),
            changed,
            status_str.green()
        );
    }

    Ok(())
}

/// Single task mode (backward compatible)
fn run_single_task(
    storage: &Storage,
    task_id: &str,
    status_str: &str,
    tag: Option<&str>,
) -> Result<()> {
    let new_status = TaskStatus::from_str(status_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status: {}. Valid: {:?}",
            status_str,
            TaskStatus::all()
        )
    })?;

    let epic_tag = resolve_group_tag(storage, tag, true)?;
    let mut epic = storage.load_group(&epic_tag)?;

    let task = epic
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in epic '{}'", task_id, epic_tag))?;

    task.set_status(new_status);

    storage.update_group(&epic_tag, &epic)?;

    println!(
        "{} Task {} → {}",
        "✓".green(),
        task_id.cyan(),
        status_str.green()
    );

    Ok(())
}

/// Get list of tags to operate on
fn get_target_tags(
    storage: &Storage,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<Vec<String>> {
    if all_tags {
        storage.list_groups()
    } else {
        let epic_tag = resolve_group_tag(storage, tag, true)?;
        Ok(vec![epic_tag])
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes

#### Manual Verification:
- [ ] `scud set-status task1 done` works (old syntax)
- [ ] `scud set-status done task1 task2` works (new multi-task)
- [ ] `scud set-status --from in-progress --to pending` works
- [ ] `scud set-status --from in-progress --to pending --all-tags` works

---

## Testing Strategy

### Manual Testing Steps:
1. Create test tasks with various statuses
2. Test single task: `scud set-status task:1 done`
3. Test multi-task: `scud set-status pending task:1 task:2`
4. Test bulk transition: `scud set-status --from pending --to in-progress`
5. Test with --all-tags flag
6. Test error cases (invalid status, task not found)

## References

- Current implementation: `scud-cli/src/commands/set_status.rs`
- CLI definition: `scud-cli/src/main.rs:178-187`
- TaskStatus model: `scud-cli/src/models/task.rs`
