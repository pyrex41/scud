# Agent Type Visibility Implementation Plan

## Overview

Add visibility of agent types, models, and tmux session IDs throughout SCUD's output to make it easier to understand what's running and where to attach for debugging.

## Current State Analysis

- `agent_type` field already exists on Task (`models/task.rs:117`)
- Spawn command shows agent info (`spawn/mod.rs:202-214`) but swarm doesn't
- Tmux window index is captured (`terminal.rs:284-286`) but discarded after use
- List uses fixed-width columns (`list.rs:55-86`)
- Waves uses inline format with `[complexity]` and `<- deps` (`waves.rs:142-174`)

## Desired End State

1. `scud list` shows agent_type column for each task
2. `scud waves` shows agent_type after complexity for each task
3. `scud swarm` output shows agent/model and tmux target (e.g., `scud:3`) while running

## What We're NOT Doing

- Adding agent_type to JSON output (already included via serde)
- Changing the agent definition loading logic
- Modifying how agent_type is assigned to tasks

## Phase 1: Update `scud list` to show agent_type column

### Overview
Add an "Agent" column to the list command's table output.

### Changes Required:

#### 1.1 Add agent_type formatting function

**File**: `scud-cli/src/commands/list.rs`

Add after `format_priority` function (around line 33):

```rust
fn format_agent_type(agent_type: &Option<String>) -> String {
    match agent_type {
        Some(at) => at.clone(),
        None => "-".to_string(),
    }
}
```

#### 1.2 Update header row

**File**: `scud-cli/src/commands/list.rs`

Update the header println! (around line 55-63) to add Agent column:

```rust
println!(
    "{:>4}  {:<11} {:<32} {:<14} {:>4}  {:<5} {}",
    "#".dimmed(),
    "ID".dimmed(),
    "Title".dimmed(),
    "Status".dimmed(),
    "Cplx".dimmed(),
    "Pri".dimmed(),
    "Agent".dimmed()
);
```

#### 1.3 Update separator line width

**File**: `scud-cli/src/commands/list.rs`

Update separator line (around line 64) to match new width:

```rust
println!("{}", "─".repeat(90).dimmed());
```

#### 1.4 Update data rows

**File**: `scud-cli/src/commands/list.rs`

Update title truncation (reduce from 36 to 30 chars to make room):

```rust
let title = if task.title.len() > 30 {
    format!("{}...", &task.title[..27])
} else {
    task.title.clone()
};
```

Update the data row println! to include agent_type:

```rust
println!(
    "{:>4}  {:<11} {:<32} {:<14} {:>4}  {:<5} {}",
    (idx + 1).to_string().dimmed(),
    format_task_id(&task.id).cyan(),
    title,
    format_status(&task.status),
    task.complexity,
    format_priority(&task.priority),
    format_agent_type(&task.agent_type).dimmed()
);
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `scud list` shows Agent column with agent types or "-" for unassigned

---

## Phase 2: Update `scud waves` to show agent_type

### Overview
Add agent_type display after complexity in the waves output.

### Changes Required:

#### 2.1 Add agent_type formatting

**File**: `scud-cli/src/commands/waves.rs`

After the complexity formatting block (around line 164), add:

```rust
let agent = if let Some(ref agent_type) = task.agent_type {
    format!(" @{}", agent_type).dimmed().to_string()
} else {
    String::new()
};
```

#### 2.2 Update println to include agent

**File**: `scud-cli/src/commands/waves.rs`

Update the task println! (around line 166-173) to include agent:

```rust
println!(
    "    {} {} {}{}{}{}",
    status_indicator,
    task_id.cyan(),
    task.title,
    complexity,
    agent,
    deps
);
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `scud waves` shows `@agent_type` after complexity for tasks with agent assigned

---

## Phase 3: Update swarm spawn output with agent/model and tmux target

### Overview
Modify spawn functions to return tmux window index, and update swarm output to show agent info and tmux target.

### Changes Required:

#### 3.1 Update spawn_tmux to return window index

**File**: `scud-cli/src/commands/spawn/terminal.rs`

Change function signature (around line 231) from:
```rust
pub fn spawn_tmux(...) -> Result<()>
```
to:
```rust
pub fn spawn_tmux(...) -> Result<String>
```

Update the return statement at the end to return window_index:
```rust
Ok(window_index)
```

#### 3.2 Update spawn_terminal_with_harness_and_model

**File**: `scud-cli/src/commands/spawn/terminal.rs`

Change function signature (around line 205) from:
```rust
pub fn spawn_terminal_with_harness_and_model(...) -> Result<()>
```
to:
```rust
pub fn spawn_terminal_with_harness_and_model(...) -> Result<String>
```

The return value from `spawn_tmux` will now propagate.

#### 3.3 Update spawn_terminal_with_harness

**File**: `scud-cli/src/commands/spawn/terminal.rs`

Change function signature (around line 193) from:
```rust
pub fn spawn_terminal_with_harness(...) -> Result<()>
```
to:
```rust
pub fn spawn_terminal_with_harness(...) -> Result<String>
```

#### 3.4 Update swarm execute_round to capture and display tmux info

**File**: `scud-cli/src/commands/swarm/mod.rs`

In `execute_round` function, update the spawn call and output (around line 600-612):

```rust
match terminal::spawn_terminal_with_harness(
    session_name,
    &info.task.id,
    &prompt,
    &info.working_dir,
    harness,
) {
    Ok(window_index) => {
        // Determine agent info for display
        let agent_info = if let Some(ref agent_type) = info.task.agent_type {
            format!("@{}", agent_type)
        } else {
            format!("{}", harness.name())
        };
        println!(
            "    {} Spawned: {} | {} [{}] {}:{}",
            "✓".green(),
            info.task.id.cyan(),
            info.task.title.dimmed(),
            agent_info.dimmed(),
            session_name.dimmed(),
            window_index.dimmed()
        );
        round_state.task_ids.push(info.task.id.clone());
        round_state.tags.push(info.tag.clone());
    }
```

#### 3.5 Update spawn command to handle new return type

**File**: `scud-cli/src/commands/spawn/mod.rs`

The spawn command calls `spawn_terminal_with_harness_and_model` (around line 194-201). Update to handle the returned window index (can ignore it since spawn already shows agent info):

```rust
match terminal::spawn_terminal_with_harness_and_model(
    &session_name,
    &info.task.id,
    &prompt,
    &info.working_dir,
    effective_harness,
    effective_model.as_deref(),
) {
    Ok(_window_index) => {
        // ... existing success handling
    }
```

### Success Criteria:

#### Automated Verification:
- [ ] Code compiles: `cargo build`
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `scud swarm` output shows `[agent_type] session:window` for each spawned task

---

## Testing Strategy

### Unit Tests:
- Existing tests should continue to pass (no behavioral changes)

### Manual Testing Steps:
1. Run `scud list` on a project with agent_type assigned to some tasks
2. Run `scud waves` on a project with agent_type assigned
3. Run `scud swarm --limit 1` and verify output shows agent info and tmux target

## References

- Task model with agent_type: `scud-cli/src/models/task.rs:117`
- Spawn output formatting: `scud-cli/src/commands/spawn/mod.rs:202-214`
- Swarm spawn output: `scud-cli/src/commands/swarm/mod.rs:607-613`
