# Expand Improvements & Human-Readable List Implementation Plan

## Overview

Three improvements to SCUD CLI:
1. Reduce subtask generation during `scud expand` (currently creates too many for simple tasks)
2. Increase default concurrent LLM requests from 5 to 10
3. Make `scud list` default to human-readable format, with `--verbose`/`-v` for raw SCG

## Current State Analysis

### Subtask Generation
- **File**: `scud-cli/src/models/task.rs:328-337`
- Current mapping generates 2-8 subtasks based on complexity
- For complexity 3 tasks (simple), it generates 3 subtasks - often too many
- Subtasks should be broader and multi-step; agents can handle complexity

### Concurrency
- **File**: `scud-cli/src/commands/ai/expand.rs:34`
- `const CONCURRENCY: usize = 5` - hardcoded limit on parallel LLM requests

### List Output
- **File**: `scud-cli/src/commands/list.rs`
- Currently outputs raw SCG format by default (pipe-delimited, single-char codes)
- Has `--json` flag for JSON output
- No human-readable option exists

## Desired End State

1. `scud expand` generates 0-3 subtasks per task (instead of 2-8) - broader, multi-step subtasks
2. `scud expand` processes 10 tasks concurrently instead of 5
3. `scud list` shows human-friendly table by default
4. `scud list --verbose` or `scud list -v` shows raw SCG format

### Verification:
```bash
# After implementation:
scud list              # Shows human-readable table
scud list -v           # Shows raw SCG format
scud list --verbose    # Shows raw SCG format
scud list --json       # Shows JSON (unchanged)
```

## What We're NOT Doing

- Not making subtask counts configurable via config file
- Not adding color themes or customization
- Not changing the underlying SCG storage format
- Not modifying JSON output format

## Implementation Approach

All three changes are independent and can be made in any order. Total: ~60 lines changed across 3 files.

---

## Phase 1: Reduce Subtask Generation

### Overview
Dramatically reduce subtask counts to create broader, multi-step subtasks. Agents are capable of handling complex subtasks, so we don't need fine-grained decomposition.

### Changes Required:

#### 1.1 Task Model

**File**: `scud-cli/src/models/task.rs`
**Lines**: 328-337

**Current code**:
```rust
pub fn recommended_subtasks_for_complexity(complexity: u32) -> usize {
    match complexity {
        0..=2 => 2,
        3 => 3,
        5 => 4,
        8 => 5,
        13 => 6,
        _ => 8, // 21+
    }
}
```

**New code**:
```rust
pub fn recommended_subtasks_for_complexity(complexity: u32) -> usize {
    match complexity {
        0..=2 => 0,  // Trivial tasks: no expansion needed
        3 => 0,      // Simple tasks: no expansion needed
        5 => 2,      // Moderate tasks: 2 broad subtasks
        8 => 2,      // Complex tasks: 2 broad subtasks
        13 => 3,     // Very complex: 3 broad subtasks
        _ => 3,      // Extremely complex (21+): 3 broad subtasks max
    }
}
```

**Rationale**:
- Tasks with complexity < 5 don't really need expansion - agents can handle them directly
- Higher complexity tasks get 2-3 broad subtasks that can each be multi-step
- This prevents over-fragmentation and keeps subtasks meaningful

#### 1.2 Update Expansion Threshold

**File**: `scud-cli/src/models/task.rs`
**Line**: 313

The `needs_expansion()` function currently triggers at complexity >= 3. Update to >= 5:

**Current code**:
```rust
pub fn needs_expansion(&self) -> bool {
    self.complexity >= 3 && !self.is_expanded() && !self.is_subtask()
}
```

**New code**:
```rust
pub fn needs_expansion(&self) -> bool {
    self.complexity >= 5 && !self.is_expanded() && !self.is_subtask()
}
```

#### 1.3 Update Tests

**File**: `scud-cli/src/models/task.rs` (test section)

Update any tests that check specific subtask counts or expansion thresholds.

### Success Criteria:

#### Automated Verification:
- [x] Tests pass: `cargo test -p scud --lib`
- [x] Clippy passes: `cargo clippy -p scud`

#### Manual Verification:
- [ ] Run `scud expand` on a complexity-3 task - should say "doesn't need expansion"
- [ ] Run `scud expand` on a complexity-5 task - should create 2 subtasks
- [ ] Run `scud expand` on a complexity-13 task - should create 3 subtasks

---

## Phase 2: Increase Concurrency

### Overview
Double the concurrent LLM request limit from 5 to 10.

### Changes Required:

#### 2.1 Expand Command

**File**: `scud-cli/src/commands/ai/expand.rs`
**Line**: 34

**Current code**:
```rust
const CONCURRENCY: usize = 5;
```

**New code**:
```rust
const CONCURRENCY: usize = 10;
```

### Success Criteria:

#### Automated Verification:
- [x] Tests pass: `cargo test -p scud --lib`
- [x] Build succeeds: `cargo build -p scud`

#### Manual Verification:
- [ ] Run `scud expand --all` and observe output shows "10 concurrent requests"

---

## Phase 3: Human-Readable List Default

### Overview
Change `scud list` to output a human-friendly table by default, with `--verbose`/`-v` for raw SCG.

### Changes Required:

#### 3.1 CLI Arguments

**File**: `scud-cli/src/main.rs`
**Location**: `List` command definition (~lines 83-95)

Add verbose flag:
```rust
List {
    /// Filter by status
    #[arg(short, long)]
    status: Option<String>,

    /// Phase tag (uses active phase if not provided)
    #[arg(short, long)]
    tag: Option<String>,

    /// Output as JSON instead of human-readable format
    #[arg(long)]
    json: bool,

    /// Output raw SCG format (default: human-readable)
    #[arg(short = 'v', long)]
    verbose: bool,
},
```

Update command routing (~line 345-347):
```rust
Commands::List { status, tag, json, verbose } => {
    commands::list::run(cli.project, status.as_deref(), tag.as_deref(), json, verbose)
}
```

#### 3.2 List Command Implementation

**File**: `scud-cli/src/commands/list.rs`

Update function signature and add human-readable formatting:

```rust
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::formats::serialize_scg;
use crate::models::{Phase, Priority, TaskStatus};
use crate::storage::Storage;

/// Format status for human display
fn format_status(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Pending => "○ Pending".normal().to_string(),
        TaskStatus::InProgress => "◐ In Progress".yellow().to_string(),
        TaskStatus::Done => "● Done".green().to_string(),
        TaskStatus::Review => "◑ Review".cyan().to_string(),
        TaskStatus::Blocked => "✗ Blocked".red().to_string(),
        TaskStatus::Deferred => "◌ Deferred".dimmed().to_string(),
        TaskStatus::Cancelled => "⊘ Cancelled".dimmed().to_string(),
        TaskStatus::Expanded => "◈ Expanded".blue().to_string(),
    }
}

/// Format priority for human display
fn format_priority(priority: &Priority) -> String {
    match priority {
        Priority::Critical => "Crit".red().bold().to_string(),
        Priority::High => "High".yellow().to_string(),
        Priority::Medium => "Med".normal().to_string(),
        Priority::Low => "Low".dimmed().to_string(),
    }
}

/// Print human-readable task list
fn print_human_readable(phase: &Phase, phase_tag: &str) {
    println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());

    if phase.tasks.is_empty() {
        println!("{}", "(no tasks)".dimmed());
        return;
    }

    // Header
    println!(
        "{:>4}  {:<8} {:<40} {:<14} {:>4}  {}",
        "#".dimmed(),
        "ID".dimmed(),
        "Title".dimmed(),
        "Status".dimmed(),
        "Cplx".dimmed(),
        "Pri".dimmed()
    );
    println!("{}", "─".repeat(80).dimmed());

    // Sort tasks by ID for display
    let mut sorted_tasks = phase.tasks.clone();
    sorted_tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

    for (idx, task) in sorted_tasks.iter().enumerate() {
        let title = if task.title.len() > 38 {
            format!("{}...", &task.title[..35])
        } else {
            task.title.clone()
        };

        println!(
            "{:>4}  {:<8} {:<40} {:<14} {:>4}  {}",
            (idx + 1).to_string().dimmed(),
            task.id.cyan(),
            title,
            format_status(&task.status),
            task.complexity,
            format_priority(&task.priority)
        );
    }

    println!();
    println!(
        "{} {} tasks",
        "Total:".dimmed(),
        phase.tasks.len()
    );
}

/// Natural sort for task IDs
fn natural_sort_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match (ap.parse::<u32>(), bp.parse::<u32>()) {
            (Ok(an), Ok(bn)) => {
                if an != bn {
                    return an.cmp(&bn);
                }
            }
            _ => {
                if ap != bp {
                    return ap.cmp(bp);
                }
            }
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

pub fn run(
    project_root: Option<PathBuf>,
    status_filter: Option<&str>,
    tag: Option<&str>,
    json_output: bool,
    verbose: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    let phase_tag = resolve_group_tag(&storage, tag, true)?;
    let tasks = storage.load_tasks()?;
    let phase = tasks
        .get(&phase_tag)
        .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

    let filter_status = status_filter
        .map(|s| {
            TaskStatus::from_str(s).ok_or_else(|| {
                anyhow::anyhow!("Invalid status: {}. Valid: {:?}", s, TaskStatus::all())
            })
        })
        .transpose()?;

    let filtered_phase = if filter_status.is_some() {
        let filtered_tasks: Vec<_> = phase
            .tasks
            .iter()
            .filter(|t| {
                filter_status
                    .as_ref()
                    .map(|fs| t.status == *fs)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let mut filtered = Phase::new(phase.name.clone());
        filtered.tasks = filtered_tasks;
        filtered
    } else {
        phase.clone()
    };

    if filtered_phase.tasks.is_empty() {
        if json_output {
            println!("[]");
        } else if verbose {
            println!("# SCUD Graph v1");
            println!("# Phase: {}", phase_tag);
            println!();
            println!("@nodes");
            println!("# id | title | status | complexity | priority");
            println!("# (no tasks)");
        } else {
            println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());
            println!("{}", "(no tasks)".dimmed());
        }
        return Ok(());
    }

    if json_output {
        let json = serde_json::to_string_pretty(&filtered_phase.tasks)?;
        println!("{}", json);
    } else if verbose {
        // Raw SCG format
        let scg = serialize_scg(&filtered_phase);
        print!("{}", scg);
    } else {
        // Human-readable format (default)
        print_human_readable(&filtered_phase, &phase_tag);
    }

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Tests pass: `cargo test -p scud --lib`
- [x] Clippy passes: `cargo clippy -p scud`
- [x] Build succeeds: `cargo build -p scud --release`

#### Manual Verification:
- [ ] `scud list` shows human-readable table with status icons
- [ ] `scud list -v` shows raw SCG format
- [ ] `scud list --verbose` shows raw SCG format
- [ ] `scud list --json` shows JSON (unchanged behavior)
- [ ] `scud list --status pending` filters correctly in human format

---

## Testing Strategy

### Unit Tests:
- Existing tests should pass (subtask count tests may need updating)
- No new tests required for concurrency change
- List command tests may need updating for new parameter

### Manual Testing Steps:
1. `scud list` - verify human-readable output
2. `scud list -v` - verify raw SCG output
3. `scud list --status done` - verify filtering works with human format
4. `scud expand -i <complexity-3-task>` - should say "doesn't need expansion"
5. `scud expand -i <complexity-5-task>` - should create 2 broad subtasks
6. `scud expand --all` - verify 10 concurrent shown in output

## References

- Task model: `scud-cli/src/models/task.rs:328-337`
- Expand command: `scud-cli/src/commands/ai/expand.rs:34`
- List command: `scud-cli/src/commands/list.rs`
- SCG format: `scud-cli/src/formats/scg.rs`
