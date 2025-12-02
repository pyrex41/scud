# Cross-Tag Dependency Resolution Implementation Plan

**Date**: 2025-12-02
**Status**: COMPLETE (Part 1 & Part 2)
**Goal**: Enable tasks to depend on tasks in other tags/phases, making phases useful for task organization but not execution boundaries.

## Problem Statement

Currently, `has_dependencies_met()` only searches within a single phase's task slice. A task in the `api` phase cannot depend on a task in the `auth` phase because the dependency won't be found when checking.

Example that currently fails:
```
# auth phase
task auth:1 - "Create user model" [done]

# api phase
task api:3 - "Create user endpoints" [pending, depends on auth:1]
```

When checking if `api:3` is ready, we only search `api` phase tasks, so `auth:1` is not found and the dependency appears unmet.

## Design Decision

**Approach**: Flatten all phases into a single task slice at call sites.

This approach:
- Keeps `has_dependencies_met()` signature unchanged
- Minimizes code changes
- Is backward-compatible (local IDs still work within single phase)
- Leverages existing `load_tasks()` which returns all phases

## Implementation Steps

### Step 1: Add Helper Function for Flattening Tasks - [x] DONE

**File**: `scud-cli/src/commands/helpers.rs` (new or existing)

```rust
/// Flatten all tasks from all phases into a single Vec for dependency checking
pub fn flatten_all_tasks(all_phases: &HashMap<String, Phase>) -> Vec<&Task> {
    all_phases
        .values()
        .flat_map(|phase| phase.tasks.iter())
        .collect()
}
```

### Step 2: Update `next.rs` (3 call sites) - [x] DONE

**File**: `scud-cli/src/commands/next.rs`

**Current** (line ~39 in `find_next_available`):
```rust
.filter(|t| t.has_dependencies_met(&phase.tasks))
```

**Change**: This function signature needs updating to accept all tasks:

```rust
pub fn find_next_available<'a>(
    phase: &'a Phase,
    all_tasks: &[&Task],  // NEW: all tasks from all phases
    exclude_locked: bool,
) -> NextTaskResult<'a> {
    // ...
    let deps_met: Vec<_> = pending_tasks
        .iter()
        .filter(|t| t.has_dependencies_met_refs(all_tasks))  // Use refs version
        .collect();
```

**Call sites to update**:
- Line 90: `find_next_available(phase, &all_tasks_flat, true)` (spawn mode)
- Line 107: `find_next_available(phase, &all_tasks_flat, false)` (standard mode)
- Line 170: Inside `handle_claim()` - same pattern
- Line 177: Inside `handle_claim()` - same pattern

### Step 3: Add `has_dependencies_met_refs` Method - [x] DONE

**File**: `scud-cli/src/models/task.rs`

Add a new method that works with references (for the flattened case):

```rust
/// Check if all dependencies are met, searching across provided task references
/// Supports cross-tag dependencies when passed tasks from all phases
pub fn has_dependencies_met_refs(&self, all_tasks: &[&Task]) -> bool {
    self.dependencies.iter().all(|dep_id| {
        all_tasks
            .iter()
            .find(|t| &t.id == dep_id)
            .map(|t| t.status == TaskStatus::Done)
            .unwrap_or(false)
    })
}
```

### Step 4: Update `next_batch.rs` - [x] DONE

**File**: `scud-cli/src/commands/next_batch.rs`

**Line ~18**: Update to pass all tasks for dependency checking.

```rust
// Load all phases, not just the active one
let all_phases = storage.load_tasks()?;
let all_tasks_flat: Vec<&Task> = flatten_all_tasks(&all_phases);

// Get active phase for filtering
let phase = all_phases.get(&phase_tag)
    .ok_or_else(|| anyhow::anyhow!("Phase not found"))?;

// Use flattened tasks for dependency checks
let available: Vec<_> = phase.tasks.iter()
    .filter(|t| t.status == TaskStatus::Pending)
    .filter(|t| t.has_dependencies_met_refs(&all_tasks_flat))
    .collect();
```

### Step 5: Update `warmup.rs` - [x] DONE

**File**: `scud-cli/src/commands/warmup.rs`

**Line ~118**: Similar pattern - load all phases and flatten for dependency checking.

### Step 6: Update `waves.rs` (Already Partially Supports) - [x] VERIFIED

**File**: `scud-cli/src/commands/waves.rs`

The `compute_waves()` function already handles multiple phases via `--all-tags`. Verify it correctly resolves cross-tag dependencies in the `task_ids` HashSet (line 206).

**Current behavior** (good): It builds a `task_ids` HashSet from all actionable tasks and only counts dependencies within that set. This should work for cross-tag deps already.

**Verification needed**: Ensure dependency IDs in the edges match the expected format (namespaced or local).

### Step 7: Update `phase.rs` (Optional) - [x] DONE

**File**: `scud-cli/src/models/phase.rs`

**Line ~107**: `find_next_task()` method - if this is still used, update it to accept all tasks.

```rust
pub fn find_next_task<'a>(&'a self, all_tasks: &[&Task]) -> Option<&'a Task> {
    self.tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending && !t.is_locked())
        .find(|t| t.has_dependencies_met_refs(all_tasks))
}
```

## Testing Plan

### Unit Tests

1. **Cross-tag dependency resolution**:
   ```rust
   #[test]
   fn test_cross_tag_dependency_met() {
       let task_a = Task::new("auth:1", "Auth task", "");
       task_a.set_status(TaskStatus::Done);

       let mut task_b = Task::new("api:1", "API task", "");
       task_b.dependencies = vec!["auth:1".to_string()];

       let all_tasks = vec![&task_a, &task_b];
       assert!(task_b.has_dependencies_met_refs(&all_tasks));
   }

   #[test]
   fn test_cross_tag_dependency_not_met() {
       let task_a = Task::new("auth:1", "Auth task", "");
       // task_a still pending

       let mut task_b = Task::new("api:1", "API task", "");
       task_b.dependencies = vec!["auth:1".to_string()];

       let all_tasks = vec![&task_a, &task_b];
       assert!(!task_b.has_dependencies_met_refs(&all_tasks));
   }
   ```

2. **Backward compatibility** (local deps still work):
   ```rust
   #[test]
   fn test_local_dependency_still_works() {
       let mut task_a = Task::new("1", "First", "");
       task_a.set_status(TaskStatus::Done);

       let mut task_b = Task::new("2", "Second", "");
       task_b.dependencies = vec!["1".to_string()];

       let all_tasks = vec![&task_a, &task_b];
       assert!(task_b.has_dependencies_met_refs(&all_tasks));
   }
   ```

### Integration Tests

1. **CLI test**: Create tasks across two tags with cross-tag dependency, verify `scud next` finds correct task
2. **Waves test**: Verify `scud waves --all-tags` correctly orders cross-tag dependencies

## Migration Notes

- **Backward compatible**: Existing task files with local-only dependencies continue to work
- **No file format changes**: SCG format unchanged, just interpretation of dependency IDs
- **No data migration**: Existing `.scud/tasks/` files work as-is

## Files Modified Summary

| File | Changes |
|------|---------|
| `src/models/task.rs` | Add `has_dependencies_met_refs()` method |
| `src/commands/helpers.rs` | Add `flatten_all_tasks()` helper |
| `src/commands/next.rs` | Update `find_next_available()` signature and 4 call sites |
| `src/commands/next_batch.rs` | Load all phases, flatten for dep checking |
| `src/commands/warmup.rs` | Load all phases, flatten for dep checking |
| `src/models/phase.rs` | Update `find_next_task()` if still used |

## Complexity Estimate

- **Effort**: ~3 hours (mostly mechanical changes)
- **Risk**: Low (additive changes, backward compatible)
- **Testing**: +1 hour for unit and integration tests

## Part 2: Dependency Re-Analysis Command

### Problem Statement

When adding a new phase mid-project (e.g., creating an `api` phase after `auth` is 50% complete), the dependency graph may need reconfiguration:
- New tasks might depend on existing completed tasks
- Existing tasks might need to depend on new tasks
- The AI that generated tasks didn't know about the other phase

### New Command: `scud reanalyze-deps`

**Purpose**: Use AI to re-analyze and suggest cross-tag dependencies, respecting current completion state.

```
scud reanalyze-deps [--tag <tag>] [--all-tags] [--apply] [--dry-run]
```

**Options**:
- `--tag <tag>`: Only reanalyze dependencies for tasks in this tag
- `--all-tags`: Reanalyze across all tags (default if no tag specified)
- `--apply`: Automatically apply suggested changes (default: interactive)
- `--dry-run`: Show suggestions without prompting to apply

### Implementation

#### Step 8: Add `reanalyze_deps.rs` Command

**File**: `scud-cli/src/commands/ai/reanalyze_deps.rs`

```rust
use crate::llm::{LLMClient, Prompts};
use crate::models::{Phase, Task, TaskStatus};
use crate::storage::Storage;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct DependencySuggestion {
    task_id: String,
    add_dependencies: Vec<String>,
    remove_dependencies: Vec<String>,
    reasoning: String,
}

pub async fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
    apply: bool,
    dry_run: bool,
) -> Result<()> {
    let storage = Storage::new(project_root.clone())?;
    let all_phases = storage.load_tasks()?;

    // Determine which phases to analyze
    let phases_to_analyze: Vec<&str> = if all_tags || tag.is_none() {
        all_phases.keys().map(|s| s.as_str()).collect()
    } else {
        vec![tag.unwrap()]
    };

    // Build context for AI: all tasks with their current state
    let task_context = build_task_context(&all_phases);

    // Generate analysis prompt
    let client = LLMClient::new_with_project_root(project_root)?;
    let prompt = Prompts::reanalyze_dependencies(&task_context, &phases_to_analyze);

    println!("Analyzing dependencies across {} phase(s)...", phases_to_analyze.len());

    let suggestions: Vec<DependencySuggestion> = client.complete_json(&prompt).await?;

    // Filter out suggestions that don't change anything
    let meaningful_suggestions: Vec<_> = suggestions
        .into_iter()
        .filter(|s| !s.add_dependencies.is_empty() || !s.remove_dependencies.is_empty())
        .collect();

    if meaningful_suggestions.is_empty() {
        println!("No dependency changes suggested.");
        return Ok(());
    }

    // Display suggestions
    println!("\nSuggested dependency changes:\n");
    for suggestion in &meaningful_suggestions {
        println!("Task: {}", suggestion.task_id);
        if !suggestion.add_dependencies.is_empty() {
            println!("  + Add: {:?}", suggestion.add_dependencies);
        }
        if !suggestion.remove_dependencies.is_empty() {
            println!("  - Remove: {:?}", suggestion.remove_dependencies);
        }
        println!("  Reason: {}", suggestion.reasoning);
        println!();
    }

    if dry_run {
        return Ok(());
    }

    // Apply changes
    if apply || confirm_apply()? {
        apply_suggestions(&storage, &mut all_phases.clone(), &meaningful_suggestions)?;
        println!("Dependencies updated successfully.");
    }

    Ok(())
}

fn build_task_context(all_phases: &HashMap<String, Phase>) -> String {
    let mut context = String::new();

    for (tag, phase) in all_phases {
        context.push_str(&format!("\n## Phase: {}\n", tag));
        for task in &phase.tasks {
            let status_marker = match task.status {
                TaskStatus::Done => "[DONE]",
                TaskStatus::InProgress => "[IN PROGRESS]",
                TaskStatus::Pending => "[PENDING]",
                TaskStatus::Blocked => "[BLOCKED]",
                TaskStatus::Expanded => "[EXPANDED]",
            };
            context.push_str(&format!(
                "- {} {} - {}\n  Current deps: {:?}\n",
                task.id, status_marker, task.title, task.dependencies
            ));
        }
    }

    context
}
```

#### Step 9: Add Prompt for Dependency Re-Analysis

**File**: `scud-cli/src/llm/prompts.rs`

Add new prompt function:

```rust
pub fn reanalyze_dependencies(task_context: &str, phases: &[&str]) -> String {
    format!(r#"You are analyzing a software project's task dependencies across multiple phases.

## Current Task State

{task_context}

## Your Task

Review the tasks above and suggest dependency changes that would improve execution order. Consider:

1. **Logical ordering**: Tasks that produce artifacts another task needs
2. **Current completion state**: Don't add deps on PENDING tasks for DONE tasks
3. **Cross-phase dependencies**: Tasks in one phase that should wait for tasks in another
4. **Remove redundant deps**: If A depends on B, and B depends on C, A doesn't also need C

## Rules

- Use full task IDs with phase prefix (e.g., "auth:1", "api:3")
- Only suggest changes for tasks that are PENDING or IN PROGRESS
- Don't modify DONE or EXPANDED tasks
- Consider that some tasks may intentionally have no dependencies

## Response Format

Return a JSON array of suggestions:
```json
[
  {{
    "task_id": "api:3",
    "add_dependencies": ["auth:1", "core:2"],
    "remove_dependencies": [],
    "reasoning": "API endpoints need authentication service and core models"
  }}
]
```

Return empty array [] if no changes are needed.

Phases to analyze: {:?}
"#, task_context, phases)
}
```

#### Step 10: Update Task Generation Commands

**File**: `scud-cli/src/commands/ai/parse_prd.rs`

After parsing PRD into tasks, if other phases exist, prompt for cross-phase dependencies:

```rust
// After line 98 (saving tasks)
// Check if other phases exist and suggest cross-tag deps
let existing_phases: Vec<_> = all_phases.keys()
    .filter(|k| *k != tag)
    .cloned()
    .collect();

if !existing_phases.is_empty() {
    println!("\nOther phases detected: {:?}", existing_phases);
    println!("Consider running 'scud reanalyze-deps' to identify cross-phase dependencies.");
}
```

**File**: `scud-cli/src/commands/ai/expand.rs`

After expanding tasks, remind about dependency analysis:

```rust
// After line 284 (saving tasks)
if successful > 0 {
    println!("\nTip: Run 'scud reanalyze-deps --tag {}' to check for new dependencies.", tag);
}
```

### Step 11: Register Command in Main

**File**: `scud-cli/src/main.rs`

Add new subcommand:

```rust
// In Commands enum
ReanalyzeDeps {
    /// Tag to analyze (default: all tags)
    #[arg(long)]
    tag: Option<String>,

    /// Analyze all tags
    #[arg(long)]
    all_tags: bool,

    /// Automatically apply suggestions
    #[arg(long)]
    apply: bool,

    /// Show suggestions without applying
    #[arg(long)]
    dry_run: bool,
},

// In match statement
Commands::ReanalyzeDeps { tag, all_tags, apply, dry_run } => {
    ai::reanalyze_deps::run(
        project_root,
        tag.as_deref(),
        all_tags,
        apply,
        dry_run,
    ).await?;
}
```

### Step 12: Add MCP Tool for Dependency Re-Analysis

**File**: `scud-mcp/src/tools/ai.ts`

```typescript
{
  name: 'scud_reanalyze_deps',
  description: 'Re-analyze and suggest cross-phase dependencies using AI. Use after adding new phases mid-project.',
  inputSchema: {
    type: 'object',
    properties: {
      tag: {
        type: 'string',
        description: 'Specific tag to analyze (optional, default: all)',
      },
      all_tags: {
        type: 'boolean',
        description: 'Analyze all tags (default if no tag specified)',
      },
      apply: {
        type: 'boolean',
        description: 'Automatically apply suggestions (default: false)',
      },
    },
  },
}
```

## Updated Files Summary

| File | Changes |
|------|---------|
| `src/models/task.rs` | Add `has_dependencies_met_refs()` method |
| `src/commands/helpers.rs` | Add `flatten_all_tasks()` helper |
| `src/commands/next.rs` | Update `find_next_available()` signature and 4 call sites |
| `src/commands/next_batch.rs` | Load all phases, flatten for dep checking |
| `src/commands/warmup.rs` | Load all phases, flatten for dep checking |
| `src/models/phase.rs` | Update `find_next_task()` if still used |
| `src/commands/ai/reanalyze_deps.rs` | **NEW** - AI-powered dependency re-analysis |
| `src/commands/ai/parse_prd.rs` | Add reminder about cross-phase deps |
| `src/commands/ai/expand.rs` | Add reminder about dependency analysis |
| `src/llm/prompts.rs` | Add `reanalyze_dependencies()` prompt |
| `src/main.rs` | Register `reanalyze-deps` command |
| `scud-mcp/src/tools/ai.ts` | Add `scud_reanalyze_deps` MCP tool |

## Updated Complexity Estimate

- **Part 1 (Cross-tag resolution)**: ~3 hours
- **Part 2 (Reanalyze command)**: ~4 hours
- **Testing**: +2 hours
- **Total**: ~9 hours

## Workflow Example

```bash
# Start project with auth phase
scud parse-prd auth-spec.md --tag auth --num-tasks 8
# Work on auth tasks...
scud status auth:1 done
scud status auth:2 done

# Add API phase mid-project
scud parse-prd api-spec.md --tag api --num-tasks 10
# Output: "Other phases detected: [auth]. Consider running 'scud reanalyze-deps'..."

# Re-analyze dependencies across both phases
scud reanalyze-deps --all-tags
# Shows suggestions like:
# Task: api:3
#   + Add: ["auth:1", "auth:2"]
#   Reason: "API user endpoints need completed auth models"

# Apply suggestions
scud reanalyze-deps --all-tags --apply

# Now scud next will respect cross-phase deps
scud next  # Won't show api:3 until auth:1 and auth:2 are done
```

## Open Questions

1. **ID format normalization**: Should we normalize `auth:1` and `1` (when in auth phase) to the same lookup? Currently they'd be different strings.
   - **Recommendation**: Keep simple for now. Users should use consistent ID format.

2. **Circular cross-tag dependencies**: Should we add detection?
   - **Recommendation**: `waves` command already detects cycles. Low priority for now.

3. **Performance**: Flattening all tasks each time could be slow for large projects.
   - **Recommendation**: Monitor. Can add caching later if needed.

4. **Interactive mode for reanalyze-deps**: Should we allow per-suggestion accept/reject?
   - **Recommendation**: Start with all-or-nothing. Add interactive mode if requested.
