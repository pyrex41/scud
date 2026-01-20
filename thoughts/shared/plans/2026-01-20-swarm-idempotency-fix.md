# Swarm Idempotency Fix Implementation Plan

## Overview

Fix the swarm command to handle orphan in-progress tasks properly. Currently, if swarm crashes or is stopped while tasks are running, those tasks remain `InProgress` forever and block dependent tasks. The fix will detect orphan tasks and re-spawn them.

## Current State Analysis

### The Problem
1. `is_task_actionable()` at `swarm/mod.rs:546` only considers `Pending` tasks
2. Tasks marked `InProgress` when spawned (line 628) are excluded from wave computation
3. When swarm restarts, in-progress tasks are skipped - swarm waits for them but they never complete
4. Dependent tasks treat `InProgress` dependencies as "satisfied" (not in actionable set)

### Key Discoveries:
- `swarm/mod.rs:547-549`: `is_task_actionable()` returns false for non-Pending status
- `swarm/mod.rs:628`: Tasks marked `InProgress` immediately when spawned
- `swarm/mod.rs:215-222`: Swarm waits if in-progress tasks exist, but doesn't re-spawn them
- `swarm/mod.rs:499-504`: Dependencies only checked within actionable (Pending) set
- `spawn/terminal.rs`: Has `tmux_window_exists()` helper we can use

## Desired End State

1. On startup, swarm detects orphan in-progress tasks (no tmux window running)
2. Orphan tasks are either reset to `Pending` or killed and restarted
3. Dependencies on `InProgress` tasks properly block dependent tasks
4. User is prompted if detection is ambiguous

### Verification:
- Start swarm, kill it mid-execution, restart - orphan tasks are detected and handled
- Tasks depending on in-progress tasks wait properly
- `cargo test` passes

## What We're NOT Doing

- Not changing how spawn handles in-progress tasks (spawn is one-shot, not continuous)
- Not adding persistent agent tracking to swarm (that's a separate monitor integration)
- Not adding automatic timeout-based orphan detection (just tmux check)

## Implementation Approach

1. Add orphan detection function that checks tmux windows
2. Modify swarm startup to detect and handle orphans
3. Fix dependency checking to treat `InProgress` as blocking
4. Add user prompt for ambiguous cases

## Phase 1: Add Orphan Detection

### Overview
Create helper function to detect in-progress tasks that have no running tmux window.

### Changes Required:

#### 1.1 Add tmux window check helper

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add function to detect orphan tasks

```rust
/// Check if a tmux window exists for a task
fn tmux_window_exists_for_task(session_name: &str, task_id: &str) -> bool {
    let window_name = format!("task-{}", task_id);
    terminal::tmux_window_exists(session_name, &window_name)
}

/// Find in-progress tasks that have no running tmux window (orphans)
fn find_orphan_tasks(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
    session_name: &str,
) -> Vec<(String, String)> {
    // (task_id, tag) pairs
    let tags: Vec<&str> = if all_tags {
        all_phases.keys().map(|s| s.as_str()).collect()
    } else {
        vec![phase_tag]
    };

    let mut orphans = Vec::new();

    for tag in tags {
        if let Some(phase) = all_phases.get(tag) {
            for task in &phase.tasks {
                if task.status == TaskStatus::InProgress {
                    if !tmux_window_exists_for_task(session_name, &task.id) {
                        orphans.push((task.id.clone(), tag.to_string()));
                    }
                }
            }
        }
    }

    orphans
}
```

#### 1.2 Add tmux window existence check to terminal module

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add helper function (may already exist, verify first)

```rust
/// Check if a specific window exists in a tmux session
pub fn tmux_window_exists(session_name: &str, window_name: &str) -> bool {
    let output = Command::new("tmux")
        .args([
            "list-windows",
            "-t", session_name,
            "-F", "#{window_name}",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let windows = String::from_utf8_lossy(&out.stdout);
            windows.lines().any(|w| w == window_name || w.starts_with(&format!("{}-", window_name)))
        }
        _ => false,
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] New functions compile without warnings

---

## Phase 2: Handle Orphans on Startup

### Overview
Modify swarm startup to detect orphans and prompt user for action.

### Changes Required:

#### 2.1 Add orphan handling to swarm run()

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add orphan detection after session lock, before main loop

Insert after line ~90 (after `acquire_session_lock`), before entering main loop:

```rust
// Detect orphan in-progress tasks
let all_phases = storage.load_tasks()?;
let orphans = find_orphan_tasks(&all_phases, &phase_tag, all_tags, &session_name);

if !orphans.is_empty() {
    println!();
    println!("{}", "Detected orphan in-progress tasks (no tmux window):".yellow());
    for (task_id, tag) in &orphans {
        println!("  {} {} (tag: {})", "•".yellow(), task_id.cyan(), tag.dimmed());
    }
    println!();

    // Prompt user for action
    let choices = vec![
        "Reset to pending and re-run",
        "Kill existing windows (if any) and restart",
        "Skip and continue (leave as in-progress)",
        "Abort",
    ];

    let selection = dialoguer::Select::new()
        .with_prompt("How should orphan tasks be handled?")
        .items(&choices)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Reset to pending
            for (task_id, tag) in &orphans {
                if let Ok(mut phase) = storage.load_group(tag) {
                    if let Some(task) = phase.get_task_mut(task_id) {
                        task.set_status(TaskStatus::Pending);
                        storage.update_group(tag, &phase)?;
                        println!("  {} {} → pending", "✓".green(), task_id);
                    }
                }
            }
        }
        1 => {
            // Kill and restart - first try to kill any matching windows
            for (task_id, _) in &orphans {
                let window_name = format!("task-{}", task_id);
                let _ = terminal::kill_tmux_window(&session_name, &window_name);
            }
            // Reset to pending so they'll be picked up
            for (task_id, tag) in &orphans {
                if let Ok(mut phase) = storage.load_group(tag) {
                    if let Some(task) = phase.get_task_mut(task_id) {
                        task.set_status(TaskStatus::Pending);
                        storage.update_group(tag, &phase)?;
                        println!("  {} {} → pending (will re-spawn)", "✓".green(), task_id);
                    }
                }
            }
        }
        2 => {
            // Skip - do nothing, leave as in-progress
            println!("{}", "Leaving orphan tasks as in-progress.".dimmed());
        }
        3 => {
            // Abort
            anyhow::bail!("Aborted by user");
        }
        _ => {}
    }
    println!();
}
```

#### 2.2 Add kill_tmux_window helper

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add helper to kill a specific window

```rust
/// Kill a specific tmux window
pub fn kill_tmux_window(session_name: &str, window_name: &str) -> Result<()> {
    let target = format!("{}:{}", session_name, window_name);
    Command::new("tmux")
        .args(["kill-window", "-t", &target])
        .output()?;
    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes

#### Manual Verification:
- [ ] Start swarm, spawn tasks, kill swarm process
- [ ] Restart swarm - should detect orphan tasks and prompt
- [ ] Selecting "Reset to pending" should work
- [ ] Tasks should be re-spawned in next wave

---

## Phase 3: Fix Dependency Blocking for InProgress Tasks

### Overview
Modify wave computation to treat `InProgress` dependencies as blocking (not satisfied).

### Changes Required:

#### 3.1 Update dependency graph building

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: In `compute_waves_from_tasks()`, check for in-progress dependencies

Current code at lines 498-506 only counts dependencies within actionable set. Modify to also check for in-progress dependencies:

```rust
// Build dependency graph
let task_ids: HashSet<String> = actionable.iter().map(|t| t.id.clone()).collect();
let mut in_degree: HashMap<String, usize> = HashMap::new();
let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

// Collect in-progress task IDs for blocking check
let in_progress_ids: HashSet<String> = {
    let tags: Vec<&str> = if all_tags {
        all_phases.keys().map(|s| s.as_str()).collect()
    } else {
        vec![phase_tag]
    };

    tags.iter()
        .filter_map(|tag| all_phases.get(*tag))
        .flat_map(|phase| &phase.tasks)
        .filter(|t| t.status == TaskStatus::InProgress)
        .map(|t| t.id.clone())
        .collect()
};

for task in &actionable {
    in_degree.entry(task.id.clone()).or_insert(0);

    for dep in &task.dependencies {
        if task_ids.contains(dep) {
            // Dependency is pending - will be in a wave
            *in_degree.entry(task.id.clone()).or_insert(0) += 1;
            dependents.entry(dep.clone()).or_default().push(task.id.clone());
        } else if in_progress_ids.contains(dep) {
            // Dependency is in-progress - block this task
            // Set very high in-degree so it never becomes ready
            *in_degree.entry(task.id.clone()).or_insert(0) += 1000;
        }
        // If dep is Done/Failed/etc, it's satisfied - do nothing
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes

#### Manual Verification:
- [ ] Create task B that depends on task A
- [ ] Mark A as in-progress manually: `scud set-status A in-progress`
- [ ] Run swarm - B should NOT be included in waves
- [ ] Mark A as done: `scud set-status A done`
- [ ] Run swarm - B should now be included

---

## Testing Strategy

### Unit Tests:
- Test `find_orphan_tasks()` with mock data
- Test dependency blocking logic

### Integration Tests:
- Test orphan detection with real tmux sessions

### Manual Testing Steps:
1. Create a tag with multiple tasks with dependencies
2. Run `scud swarm --tag test`
3. While tasks are running, kill the swarm process (Ctrl+C or kill)
4. Run `scud swarm --tag test` again
5. Verify orphan detection prompt appears
6. Select "Reset to pending" and verify tasks are re-spawned

## References

- Swarm implementation: `scud-cli/src/commands/swarm/mod.rs`
- Terminal helpers: `scud-cli/src/commands/spawn/terminal.rs`
- Task model: `scud-cli/src/models/task.rs`
