# SCUD CLI UX Improvements Implementation Plan

## Overview

This plan addresses critical data model issues and UX anti-patterns in the scud-cli Rust tool:
1. **Fix ID ambiguity**: Namespace task IDs by epic tag (e.g., `phase1:5.1` not just `5.1`)
2. **Fix complexity model**: Remove complexity from subtasks, track only at parent level
3. **Add waves command**: Plan parallel execution based on dependencies
4. **Remove hidden state**: Add `--tag` flags with interactive fallback, remove `use-tag`
5. **Consolidate commands**: Merge tags/use-tag, remove groups (redundant with tags)
6. **Parallelize AI**: Process tasks concurrently with configurable limits

## Current State Analysis

### Data Model Issues Discovered

**1. Task ID Collisions** (CRITICAL)
```json
// In tasks.json, multiple epics can have task "1.1":
{
  "phase1": { "tasks": [{"id": "1.1", ...}] },
  "phase2": { "tasks": [{"id": "1.1", ...}] }
}
// Dependency "5.1" is ambiguous - which epic?
```

**2. Complexity Double-Counting**
- Parent tasks retain complexity after expansion (e.g., task 10 has complexity 13)
- Subtasks have complexity 0 (can't plan workload)
- Total complexity counts parent + subtasks = incorrect

**3. Groups vs Tags Confusion**
- **Tags/Epics**: Primary organizational unit (`HashMap<tag, Epic>`)
- **Groups**: Optional bundle of epics in separate file (`epic-groups.json`)
- Groups add complexity without clear value - epics already serve as groupings
- Recommendation: **Remove groups entirely**, use epic tags for organization

### Key Files
| File | Purpose |
|------|---------|
| `models/task.rs` | Task struct with ID, complexity, dependencies |
| `models/epic.rs` | Epic struct containing Vec<Task> |
| `models/group.rs` | EpicGroup (candidate for removal) |
| `storage/mod.rs` | JSON I/O, active_epic state management |
| `commands/ai/expand.rs` | Creates subtasks with bare IDs |

## Desired End State

### Data Model
```json
{
  "phase1": {
    "name": "phase1",
    "tasks": [
      {
        "id": "phase1:10",           // Namespaced ID
        "title": "Implement auth",
        "complexity": 13,            // Parent keeps complexity
        "status": "expanded",        // New status for parents
        "subtasks": ["phase1:10.1", "phase1:10.2", "phase1:10.3"],
        "dependencies": ["phase1:9"] // Fully qualified deps
      },
      {
        "id": "phase1:10.1",         // Subtask ID includes parent
        "title": "Create auth models",
        "parent_id": "phase1:10",    // Reference to parent
        "complexity": 0,             // Subtasks have no complexity (inherited)
        "dependencies": []
      }
    ]
  }
}
```

### Command Changes
```bash
# Tags (merged with use-tag)
scud tags                    # List tags, prompt to select
scud tags phase1             # Set active tag

# All commands support --tag
scud list --tag phase1       # Works without active tag
scud expand --all --tag phase1

# New waves command
scud waves                   # Show execution waves for active epic
scud waves --tag phase1 -n 5 # 5 parallel tasks max
scud waves --all-tags        # Cross-epic wave planning

# Removed commands
# - use-tag (merged into tags)
# - analyze-complexity (merged into expand)
# - group create/list/status/add (removed entirely)
```

### Verification
- All task IDs globally unique (can grep for collisions)
- Dependencies always resolve to exactly one task
- Total complexity = sum of non-expanded parent tasks only
- Waves command produces valid execution plan

## What We're NOT Doing

- Changing storage format (still JSON files)
- Adding database backend
- Web UI
- Multi-user collaboration
- External integrations (Jira, GitHub)

## Implementation Approach

**Phase 1**: Fix data model (IDs, complexity, parent/subtask relationship)
**Phase 2**: Add waves command for parallel execution planning
**Phase 3**: Remove groups, simplify to tags only
**Phase 4**: Add `--tag` flags, remove `use-tag` hidden state
**Phase 5**: Parallelize AI commands
**Phase 6**: Cleanup and testing

---

## Phase 1: Data Model Fixes

### Overview
Fix the fundamental data model issues: ID namespacing, complexity tracking, and parent/subtask relationships.

### Changes Required:

#### 1.1 Add Namespaced ID Format

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add ID validation and helper methods

```rust
impl Task {
    // Add new constant
    const ID_SEPARATOR: char = ':';

    /// Parse a task ID into (epic_tag, local_id) parts
    /// e.g., "phase1:10.1" -> Some(("phase1", "10.1"))
    /// e.g., "10.1" -> None (legacy format)
    pub fn parse_id(id: &str) -> Option<(&str, &str)> {
        id.split_once(Self::ID_SEPARATOR)
    }

    /// Create a namespaced task ID
    pub fn make_id(epic_tag: &str, local_id: &str) -> String {
        format!("{}{}{}", epic_tag, Self::ID_SEPARATOR, local_id)
    }

    /// Get the local ID part (without epic prefix)
    pub fn local_id(&self) -> &str {
        Self::parse_id(&self.id)
            .map(|(_, local)| local)
            .unwrap_or(&self.id)
    }

    /// Get the epic tag from a namespaced ID
    pub fn epic_tag(&self) -> Option<&str> {
        Self::parse_id(&self.id).map(|(tag, _)| tag)
    }

    /// Check if this is a subtask (has parent)
    pub fn is_subtask(&self) -> bool {
        self.parent_id.is_some()
    }

    /// Check if this task has been expanded into subtasks
    pub fn is_expanded(&self) -> bool {
        !self.subtasks.is_empty()
    }
}
```

#### 1.2 Add Parent/Subtask Fields to Task

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add new fields to Task struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,

    #[serde(default)]
    pub status: TaskStatus,

    #[serde(default)]
    pub complexity: u32,

    #[serde(default)]
    pub priority: Priority,

    #[serde(default)]
    pub dependencies: Vec<String>,

    // NEW: Parent-child relationship
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<String>,

    // ... existing optional fields ...
}
```

#### 1.3 Add "Expanded" Status

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add Expanded status variant

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Done,
    Review,
    Blocked,
    Deferred,
    Cancelled,
    Expanded,  // NEW: Task has been broken into subtasks
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            // ... existing ...
            TaskStatus::Expanded => "expanded",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            // ... existing ...
            "expanded" => Some(TaskStatus::Expanded),
            _ => None,
        }
    }

    pub fn all() -> Vec<&'static str> {
        vec![
            "pending", "in-progress", "done", "review",
            "blocked", "deferred", "cancelled", "expanded",
        ]
    }
}
```

#### 1.4 Update Epic Stats to Exclude Expanded Parents

**File**: `scud-cli/src/models/epic.rs`
**Changes**: Fix complexity calculation

```rust
impl Epic {
    pub fn get_stats(&self) -> EpicStats {
        let mut total = 0;
        let mut pending = 0;
        let mut in_progress = 0;
        let mut done = 0;
        let mut blocked = 0;
        let mut expanded = 0;
        let mut total_complexity = 0;

        for task in &self.tasks {
            // Don't count subtasks in total (they're part of parent)
            if task.is_subtask() {
                continue;
            }

            total += 1;

            // Only count complexity for non-expanded tasks
            // (expanded tasks have their work represented by subtasks)
            if !task.is_expanded() {
                total_complexity += task.complexity;
            }

            match task.status {
                TaskStatus::Pending => pending += 1,
                TaskStatus::InProgress => in_progress += 1,
                TaskStatus::Done => done += 1,
                TaskStatus::Blocked => blocked += 1,
                TaskStatus::Expanded => expanded += 1,
                _ => {}
            }
        }

        EpicStats {
            total,
            pending,
            in_progress,
            done,
            blocked,
            expanded,  // NEW field
            total_complexity,
        }
    }

    /// Get actionable tasks (not expanded, not subtasks of incomplete parents)
    pub fn get_actionable_tasks(&self) -> Vec<&Task> {
        self.tasks.iter()
            .filter(|t| {
                // Exclude expanded parents (work on subtasks instead)
                if t.is_expanded() {
                    return false;
                }
                // Include subtasks only if they're actionable
                if let Some(ref parent_id) = t.parent_id {
                    // Parent must be expanded
                    self.get_task(parent_id)
                        .map(|p| p.is_expanded())
                        .unwrap_or(false)
                } else {
                    // Top-level task
                    true
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub done: usize,
    pub blocked: usize,
    pub expanded: usize,  // NEW
    pub total_complexity: u32,
}
```

#### 1.5 Update Expand Command to Use Namespaced IDs

**File**: `scud-cli/src/commands/ai/expand.rs`
**Changes**: Generate namespaced subtask IDs, set parent status

```rust
pub async fn run(
    project_root: Option<PathBuf>,
    task_id: Option<&str>,
    expand_all: bool,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let epic_tag = resolve_epic_tag(&storage, tag, true)?;

    // ... existing code to get tasks ...

    for id in task_ids {
        let task = epic.get_task(&id)?;
        // ... LLM expansion ...

        // Create subtasks with namespaced IDs
        let parent_local_id = Task::parse_id(&id)
            .map(|(_, local)| local)
            .unwrap_or(&id);

        let mut subtask_ids = Vec::new();
        for (idx, expanded) in expanded_tasks.iter().enumerate() {
            // Namespaced subtask ID: epic:parent.index
            let subtask_local_id = format!("{}.{}", parent_local_id, idx + 1);
            let subtask_id = Task::make_id(&epic_tag, &subtask_local_id);

            let mut new_task = Task::new(
                subtask_id.clone(),
                expanded.title.clone(),
                expanded.description.clone(),
            );
            new_task.parent_id = Some(id.clone());  // Link to parent
            new_task.complexity = 0;  // Subtasks don't have independent complexity

            // Map dependencies to namespaced IDs
            new_task.dependencies = expanded.dependencies.iter()
                .filter_map(|dep| {
                    if let Ok(dep_idx) = dep.parse::<usize>() {
                        // Relative subtask reference -> absolute ID
                        if dep_idx > 0 && dep_idx <= idx {
                            let dep_local = format!("{}.{}", parent_local_id, dep_idx);
                            Some(Task::make_id(&epic_tag, &dep_local))
                        } else {
                            None
                        }
                    } else if !dep.contains(':') {
                        // Bare ID -> namespace it
                        Some(Task::make_id(&epic_tag, dep))
                    } else {
                        // Already namespaced
                        Some(dep.clone())
                    }
                })
                .collect();

            subtask_ids.push(subtask_id.clone());
            epic.add_task(new_task);
        }

        // Update parent task
        let parent = epic.get_task_mut(&id).unwrap();
        parent.status = TaskStatus::Expanded;  // Mark as expanded
        parent.subtasks = subtask_ids;         // Track subtask IDs
        // DON'T modify title with [PARENT] prefix anymore
        // DON'T zero out complexity (it represents the total work)
    }

    // ... save and output ...
}
```

#### 1.6 Add Migration Command for Existing Data

**File**: `scud-cli/src/commands/migrate.rs` (NEW)

```rust
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::models::task::Task;
use crate::storage::Storage;

/// Migrate task IDs to namespaced format
pub fn run(project_root: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let storage = Storage::new(project_root);
    let mut all_tasks = storage.load_tasks()?;
    let mut changes = Vec::new();

    for (epic_tag, epic) in all_tasks.iter_mut() {
        let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Phase 1: Collect ID mappings
        for task in &epic.tasks {
            if !task.id.contains(':') {
                let new_id = Task::make_id(epic_tag, &task.id);
                id_map.insert(task.id.clone(), new_id.clone());
                changes.push(format!("{}: {} -> {}", epic_tag, task.id, new_id));
            }
        }

        // Phase 2: Update IDs and references
        for task in &mut epic.tasks {
            // Update task ID
            if let Some(new_id) = id_map.get(&task.id) {
                task.id = new_id.clone();
            }

            // Update dependencies
            task.dependencies = task.dependencies.iter()
                .map(|dep| {
                    id_map.get(dep).cloned().unwrap_or_else(|| {
                        if dep.contains(':') {
                            dep.clone()
                        } else {
                            Task::make_id(epic_tag, dep)
                        }
                    })
                })
                .collect();

            // Update parent_id if present
            if let Some(ref parent) = task.parent_id {
                task.parent_id = Some(
                    id_map.get(parent).cloned()
                        .unwrap_or_else(|| Task::make_id(epic_tag, parent))
                );
            }

            // Update subtask references
            task.subtasks = task.subtasks.iter()
                .map(|sub| {
                    id_map.get(sub).cloned()
                        .unwrap_or_else(|| Task::make_id(epic_tag, sub))
                })
                .collect();

            // Fix [PARENT] prefix -> Expanded status
            if task.title.starts_with("[PARENT]") {
                task.title = task.title.trim_start_matches("[PARENT]").trim().to_string();
                task.status = crate::models::task::TaskStatus::Expanded;
            }
        }
    }

    if dry_run {
        println!("{}", "Dry run - no changes made".yellow());
        println!("\nProposed changes:");
        for change in &changes {
            println!("  {}", change);
        }
        println!("\nTotal: {} ID changes", changes.len());
    } else {
        storage.save_tasks(&all_tasks)?;
        println!("{}", "Migration complete!".green());
        println!("Updated {} task IDs", changes.len());
    }

    Ok(())
}
```

#### 1.7 Add Migrate Command to CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add migrate command

```rust
/// Migrate task data to new format
Migrate {
    /// Show what would change without making changes
    #[arg(long)]
    dry_run: bool,
},

// In match:
Commands::Migrate { dry_run } => commands::migrate::run(cli.project, dry_run),
```

### Success Criteria:

#### Automated Verification:
- [ ] Compiles: `cargo build`
- [ ] Tests pass: `cargo test`
- [ ] No duplicate IDs: `grep -o '"id": "[^"]*"' .taskmaster/tasks/tasks.json | sort | uniq -d` returns empty

#### Manual Verification:
- [ ] `scud migrate --dry-run` shows expected ID changes
- [ ] `scud migrate` updates IDs correctly
- [ ] `scud list` shows tasks with namespaced IDs
- [ ] `scud stats` shows correct complexity (no double-counting)
- [ ] Dependencies resolve correctly after migration

---

## Phase 2: Waves Command for Parallel Execution Planning

### Overview
Add a command to analyze dependencies and create execution "waves" - groups of tasks that can be run in parallel.

### Changes Required:

#### 2.1 Create Waves Command

**File**: `scud-cli/src/commands/waves.rs` (NEW)

```rust
use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::commands::helpers::resolve_epic_tag;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

#[derive(Debug)]
pub struct Wave {
    pub number: usize,
    pub tasks: Vec<String>,
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    max_parallel: usize,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let all_tasks = storage.load_tasks()?;

    // Collect tasks from specified epic(s)
    let tasks_to_plan: Vec<(&str, &Task)> = if all_tags {
        all_tasks.iter()
            .flat_map(|(tag, epic)| {
                epic.tasks.iter().map(move |t| (tag.as_str(), t))
            })
            .collect()
    } else {
        let epic_tag = resolve_epic_tag(&storage, tag, true)?;
        all_tasks.get(&epic_tag)
            .map(|epic| epic.tasks.iter().map(|t| (epic_tag.as_str(), t)).collect())
            .unwrap_or_default()
    };

    // Filter to actionable tasks (not done, not expanded parents)
    let actionable: Vec<&Task> = tasks_to_plan.iter()
        .filter(|(_, t)| {
            t.status != TaskStatus::Done &&
            t.status != TaskStatus::Expanded &&
            t.status != TaskStatus::Cancelled
        })
        .map(|(_, t)| *t)
        .collect();

    if actionable.is_empty() {
        println!("{}", "No actionable tasks found.".yellow());
        return Ok(());
    }

    // Build dependency graph
    let waves = compute_waves(&actionable, max_parallel);

    // Display waves
    println!("{}", "Execution Waves".blue().bold());
    println!("Max parallel tasks: {}", max_parallel);
    println!();

    let mut total_tasks = 0;
    for wave in &waves {
        total_tasks += wave.tasks.len();
        println!(
            "{} {} ({} task{})",
            format!("Wave {}:", wave.number).yellow().bold(),
            if wave.tasks.len() <= max_parallel { "".to_string() } else {
                format!(" (batched into {} rounds)", (wave.tasks.len() + max_parallel - 1) / max_parallel)
            },
            wave.tasks.len(),
            if wave.tasks.len() == 1 { "" } else { "s" }
        );

        for (i, chunk) in wave.tasks.chunks(max_parallel).enumerate() {
            if wave.tasks.len() > max_parallel {
                println!("  Round {}:", i + 1);
            }
            for task_id in chunk {
                // Find task details
                let task = actionable.iter().find(|t| &t.id == task_id);
                if let Some(t) = task {
                    let deps = if t.dependencies.is_empty() {
                        "".to_string()
                    } else {
                        format!(" (after: {})", t.dependencies.join(", ").dimmed())
                    };
                    println!("    {} {}{}", task_id.cyan(), t.title, deps);
                }
            }
        }
        println!();
    }

    // Summary
    println!("{}", "Summary".blue().bold());
    println!("Total waves: {}", waves.len());
    println!("Total tasks: {}", total_tasks);

    let sequential_time = total_tasks;
    let parallel_time: usize = waves.iter()
        .map(|w| (w.tasks.len() + max_parallel - 1) / max_parallel)
        .sum();

    println!(
        "Estimated speedup: {}x (from {} sequential to {} parallel rounds)",
        format!("{:.1}", sequential_time as f64 / parallel_time as f64).green(),
        sequential_time,
        parallel_time
    );

    Ok(())
}

/// Compute execution waves using topological sort with level assignment
fn compute_waves(tasks: &[&Task], max_parallel: usize) -> Vec<Wave> {
    let task_ids: HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();

    // Build reverse dependency map (who depends on me?)
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    for task in tasks {
        in_degree.entry(task.id.as_str()).or_insert(0);

        for dep in &task.dependencies {
            // Only count dependencies within our task set
            if task_ids.contains(dep.as_str()) {
                *in_degree.entry(task.id.as_str()).or_insert(0) += 1;
                dependents.entry(dep.as_str()).or_default().push(task.id.as_str());
            }
        }
    }

    // Kahn's algorithm with level tracking
    let mut waves: Vec<Wave> = Vec::new();
    let mut remaining = in_degree.clone();

    let mut wave_number = 1;
    while !remaining.is_empty() {
        // Find all tasks with no remaining dependencies
        let ready: Vec<&str> = remaining.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        if ready.is_empty() {
            // Circular dependency detected
            println!("{}", "Warning: Circular dependency detected!".red());
            println!("Remaining tasks with unmet dependencies:");
            for (id, _) in &remaining {
                println!("  {}", id);
            }
            break;
        }

        // Remove ready tasks and update dependents
        for &task_id in &ready {
            remaining.remove(task_id);

            if let Some(deps) = dependents.get(task_id) {
                for &dep_id in deps {
                    if let Some(deg) = remaining.get_mut(dep_id) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        waves.push(Wave {
            number: wave_number,
            tasks: ready.iter().map(|&s| s.to_string()).collect(),
        });
        wave_number += 1;
    }

    waves
}
```

#### 2.2 Add Waves Command to CLI

**File**: `scud-cli/src/main.rs`

```rust
/// Plan parallel execution waves based on dependencies
Waves {
    /// Epic tag (uses active epic if not provided)
    #[arg(short, long)]
    tag: Option<String>,

    /// Maximum parallel tasks per round (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    max_parallel: usize,

    /// Plan across all epics
    #[arg(long)]
    all_tags: bool,
},

// In match:
Commands::Waves { tag, max_parallel, all_tags } => {
    commands::waves::run(cli.project, tag.as_deref(), max_parallel, all_tags)
}
```

#### 2.3 Add Module Export

**File**: `scud-cli/src/commands/mod.rs`

```rust
pub mod waves;  // NEW
```

### Success Criteria:

#### Automated Verification:
- [ ] Compiles: `cargo build`
- [ ] Tests pass: `cargo test`

#### Manual Verification:
- [ ] `scud waves` shows execution plan for active epic
- [ ] `scud waves -n 5` limits to 5 parallel tasks per round
- [ ] `scud waves --all-tags` plans across all epics
- [ ] Circular dependencies are detected and reported
- [ ] Tasks with dependencies appear in later waves than their dependencies

---

## Phase 3: Remove Groups, Simplify to Tags Only

### Overview
Remove the `EpicGroup` concept entirely. Epic tags already provide sufficient organizational structure, and groups add confusion without clear value.

### Changes Required:

#### 3.1 Remove Group Model

**File**: `scud-cli/src/models/group.rs`
**Action**: DELETE this file entirely

#### 3.2 Remove Group from Models Module

**File**: `scud-cli/src/models/mod.rs`
**Changes**: Remove group export

```rust
mod epic;
// REMOVE: mod group;
mod task;
mod workflow;

pub use epic::{Epic, EpicStats};
// REMOVE: pub use group::{EpicGroup, EpicGroups, GroupStatus};
pub use task::{Priority, Task, TaskStatus};
pub use workflow::WorkflowState;
```

#### 3.3 Remove Group Storage Methods

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Remove group-related methods

```rust
// REMOVE these methods:
// pub fn groups_file(&self) -> PathBuf
// pub fn load_groups(&self) -> Result<EpicGroups>
// pub fn save_groups(&self, groups: &EpicGroups) -> Result<()>
```

#### 3.4 Remove Group Commands

**Files to DELETE**:
- `scud-cli/src/commands/create_group.rs`
- `scud-cli/src/commands/list_groups.rs`
- `scud-cli/src/commands/group_status.rs`
- `scud-cli/src/commands/add_to_group.rs`

#### 3.5 Update Commands Module

**File**: `scud-cli/src/commands/mod.rs`

```rust
// REMOVE:
// pub mod add_to_group;
// pub mod create_group;
// pub mod group_status;
// pub mod list_groups;
```

#### 3.6 Remove Group Commands from CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Remove all group-related command variants and match arms

```rust
// REMOVE these command variants:
// CreateGroup { ... }
// ListGroups
// GroupStatus { ... }
// AddToGroup { ... }

// REMOVE these match arms:
// Commands::CreateGroup { ... } => ...
// Commands::ListGroups => ...
// Commands::GroupStatus { ... } => ...
// Commands::AddToGroup { ... } => ...
```

### Success Criteria:

#### Automated Verification:
- [ ] Compiles: `cargo build`
- [ ] Tests pass: `cargo test`
- [ ] `scud --help` shows no group commands

#### Manual Verification:
- [ ] `scud group` returns unknown command error
- [ ] `scud create-group` returns unknown command error
- [ ] Existing `.taskmaster/epic-groups.json` files are ignored (no error)

---

## Phase 4: Remove Hidden State, Add --tag Flags

### Overview
Add `--tag` flag to all epic-dependent commands and interactive fallback. Remove `use-tag` command, merge functionality into `tags`.

### Changes Required:

#### 4.1 Add Dependencies

**File**: `scud-cli/Cargo.toml`

```toml
[dependencies]
atty = "0.2"  # TTY detection
```

#### 4.2 Create Tag Resolution Helper

**File**: `scud-cli/src/commands/helpers.rs` (NEW)

```rust
use anyhow::Result;
use dialoguer::Select;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn resolve_epic_tag(
    storage: &Storage,
    explicit_tag: Option<&str>,
    allow_interactive: bool,
) -> Result<String> {
    // Priority 1: Explicit --tag argument
    if let Some(tag) = explicit_tag {
        let tasks = storage.load_tasks()?;
        if !tasks.contains_key(tag) {
            anyhow::bail!("Epic '{}' not found. Run: scud tags", tag);
        }
        return Ok(tag.to_string());
    }

    // Priority 2: Active epic
    if let Some(active) = storage.get_active_epic()? {
        return Ok(active);
    }

    // Priority 3: Interactive selection
    if allow_interactive && is_interactive() {
        let tasks = storage.load_tasks()?;
        if tasks.is_empty() {
            anyhow::bail!("No epics found. Create one with: scud parse-prd <file> --tag <tag>");
        }

        let tags: Vec<&String> = tasks.keys().collect();
        let selection = Select::new()
            .with_prompt("Select epic")
            .items(&tags)
            .default(0)
            .interact()?;

        let selected = tags[selection].clone();
        storage.set_active_epic(&selected)?;
        return Ok(selected);
    }

    anyhow::bail!("No active epic. Use --tag <epic-tag> or run: scud tags <tag>")
}

pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}
```

#### 4.3 Merge tags + use-tag

**File**: `scud-cli/src/commands/tags.rs`
**Changes**: Accept optional tag argument, add interactive selection

```rust
use anyhow::Result;
use colored::Colorize;
use dialoguer::Select;
use std::path::PathBuf;

use crate::commands::helpers::is_interactive;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, set_tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let tasks = storage.load_tasks()?;

    if tasks.is_empty() {
        println!("{}", "No epics found.".yellow());
        println!("Create one with: scud parse-prd <file> --tag <tag>");
        return Ok(());
    }

    // If tag provided, set it as active
    if let Some(tag) = set_tag {
        if !tasks.contains_key(tag) {
            anyhow::bail!("Epic '{}' not found", tag);
        }
        storage.set_active_epic(tag)?;
        println!("{} {}", "Active epic:".green(), tag.green().bold());

        if let Some(epic) = tasks.get(tag) {
            let stats = epic.get_stats();
            println!("  {} tasks ({} pending, {} done)", stats.total, stats.pending, stats.done);
        }
        return Ok(());
    }

    // Display all tags
    let active_epic = storage.get_active_epic()?;
    println!("{}", "Epic Tags:".blue().bold());
    println!();

    let mut tag_list: Vec<&String> = tasks.keys().collect();
    tag_list.sort();

    for (idx, tag) in tag_list.iter().enumerate() {
        let epic = tasks.get(*tag).unwrap();
        let stats = epic.get_stats();
        let is_active = active_epic.as_ref() == Some(*tag);

        let indicator = if is_active { "●".green() } else { "○".white() };
        println!(
            "  {} [{}] {} ({} tasks, {} pending)",
            indicator,
            idx + 1,
            if is_active { tag.green().bold() } else { tag.normal() },
            stats.total,
            stats.pending
        );
    }

    println!();

    // Interactive selection
    if is_interactive() {
        let selection = Select::new()
            .with_prompt("Select epic to activate (Ctrl+C to cancel)")
            .items(&tag_list)
            .default(active_epic.as_ref().and_then(|a| tag_list.iter().position(|t| *t == a)).unwrap_or(0))
            .interact_opt()?;

        if let Some(idx) = selection {
            let selected = tag_list[idx];
            storage.set_active_epic(selected)?;
            println!("\n{} {}", "Active epic:".green(), selected.green().bold());
        }
    } else if active_epic.is_none() {
        println!("{}", "Set active epic: scud tags <tag>".yellow());
    }

    Ok(())
}
```

#### 4.4 Update main.rs

**File**: `scud-cli/src/main.rs`

```rust
/// List epic tags or set active tag
Tags {
    /// Tag to set as active (interactive selection if not provided)
    tag: Option<String>,
},

// REMOVE use-tag command entirely

// Update all epic-dependent commands to add --tag flag
// Example for List:
List {
    #[arg(short, long)]
    status: Option<String>,

    #[arg(short, long)]
    tag: Option<String>,  // NEW
},
```

#### 4.5 Delete use_tag.rs

**File**: `scud-cli/src/commands/use_tag.rs`
**Action**: DELETE

#### 4.6 Update All Commands

Apply `--tag` flag and use `resolve_epic_tag()` helper to:
- `list.rs`
- `show.rs`
- `stats.rs`
- `next.rs`
- `set_status.rs`
- `assign.rs`
- `claim.rs`
- `release.rs`
- `ai/expand.rs`

### Success Criteria:

#### Automated Verification:
- [ ] Compiles: `cargo build`
- [ ] Tests pass: `cargo test`
- [ ] `scud use-tag` returns unknown command

#### Manual Verification:
- [ ] `scud tags` lists tags and prompts for selection
- [ ] `scud tags phase1` sets active tag
- [ ] `scud list --tag phase1` works without active tag
- [ ] `scud list` (no active tag) prompts interactively

---

## Phase 5: Parallelize AI Commands

### Overview
Process multiple tasks concurrently with configurable limits and consistent retry logic.

### Changes Required:

#### 5.1 Add Dependencies

**File**: `scud-cli/Cargo.toml`

```toml
[dependencies]
futures = "0.3"
```

#### 5.2 Add Concurrency Config

**File**: `scud-cli/src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub research_model: Option<String>,

    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
}

fn default_max_concurrent() -> usize { 5 }
fn default_max_retries() -> u32 { 3 }
fn default_retry_delay_ms() -> u64 { 1000 }
```

#### 5.3 Create LLM Executor

**File**: `scud-cli/src/llm/executor.rs` (NEW)

```rust
use anyhow::Result;
use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::config::LLMConfig;
use crate::llm::LLMClient;

pub struct LLMExecutor {
    client: Arc<LLMClient>,
    semaphore: Arc<Semaphore>,
    max_retries: u32,
    retry_delay: Duration,
}

impl LLMExecutor {
    pub fn new(client: LLMClient, config: &LLMConfig) -> Self {
        LLMExecutor {
            client: Arc::new(client),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            max_retries: config.max_retries,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
        }
    }

    pub async fn execute_parallel<T, F, Fut>(&self, tasks: Vec<F>) -> Vec<Result<T>>
    where
        F: Fn(Arc<LLMClient>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send + 'static,
    {
        let futures = tasks.into_iter().map(|task| {
            let semaphore = Arc::clone(&self.semaphore);
            let client = Arc::clone(&self.client);
            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;

            async move {
                let _permit = semaphore.acquire().await.unwrap();

                let mut last_error = None;
                for attempt in 1..=max_retries {
                    match task(Arc::clone(&client)).await {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            last_error = Some(e);
                            if attempt < max_retries {
                                tokio::time::sleep(retry_delay).await;
                            }
                        }
                    }
                }
                Err(last_error.unwrap())
            }
        });

        join_all(futures).await
    }
}
```

#### 5.4 Update Expand to Use Parallel Executor

**File**: `scud-cli/src/commands/ai/expand.rs`
**Changes**: Use `LLMExecutor::execute_parallel()` for LLM calls

(See Phase 4 of previous plan for detailed implementation)

#### 5.5 Remove analyze-complexity Command

Merge into expand - expand auto-analyzes tasks with complexity=0.

**File**: `scud-cli/src/commands/ai/analyze_complexity.rs`
**Action**: DELETE (functionality merged into expand)

### Success Criteria:

#### Automated Verification:
- [ ] Compiles: `cargo build`
- [ ] Tests pass: `cargo test`
- [ ] `scud analyze-complexity` returns unknown command

#### Manual Verification:
- [ ] `scud expand --all` processes tasks in parallel (multiple spinners)
- [ ] Tasks with complexity=0 are auto-analyzed before expansion
- [ ] Retry logic works (test with temporary bad API key)
- [ ] Concurrency limit respected (no more than 5 simultaneous)

---

## Phase 6: Cleanup and Testing

### Overview
Final cleanup, documentation updates, and comprehensive testing.

### Changes Required:

#### 6.1 Add Integration Tests

**File**: `scud-cli/tests/integration_tests.rs` (NEW)

```rust
// Test ID namespacing
// Test wave computation
// Test migration command
// Test tag selection
```

#### 6.2 Update Documentation

**Files**:
- `scud-cli/README.md`
- `log_docs/RUST_CLI_IMPLEMENTATION.md`

### Success Criteria:

#### Automated Verification:
- [ ] All tests pass: `cargo test`
- [ ] Integration tests pass
- [ ] No clippy warnings: `cargo clippy`

#### Manual Verification:
- [ ] Full workflow test: init → parse-prd → expand → waves → execute
- [ ] Documentation accurately reflects new commands

---

## Summary of Breaking Changes

| Old | New | Migration |
|-----|-----|-----------|
| Task ID `5.1` | `phase1:5.1` | Run `scud migrate` |
| `scud use-tag <tag>` | `scud tags <tag>` | Update scripts |
| `scud analyze-complexity` | `scud expand` (auto-analyzes) | Update scripts |
| `scud create-group` | (removed) | Use epic tags instead |
| `scud list-groups` | (removed) | Use `scud tags` |
| `scud group-status` | (removed) | Use `scud stats --tag` |
| `scud add-to-group` | (removed) | Use epic tags |
| `[PARENT]` title prefix | `status: expanded` | Run `scud migrate` |
| Subtask complexity values | Always 0 (inherited) | Run `scud migrate` |

## References

- Research: `thoughts/shared/research/2025-11-25-scud-cli-ux-improvement-analysis.md`
- Task model: `scud-cli/src/models/task.rs`
- Epic model: `scud-cli/src/models/epic.rs`
- Storage: `scud-cli/src/storage/mod.rs`
