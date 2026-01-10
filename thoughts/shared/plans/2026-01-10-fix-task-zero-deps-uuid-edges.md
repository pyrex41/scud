# Fix Task Zero Dependencies and UUID Edge Propagation Implementation Plan

## Overview

Fix three related issues in the SCUD task management system:
1. LLM prompts don't explicitly prohibit "0" as a task index, leading AI models to generate invalid "task 0" references
2. `parse_prd.rs` doesn't remap 1-indexed LLM dependency references to actual UUIDs (unlike `expand.rs` which does this correctly)
3. No standalone command exists to validate dependencies without using AI

## Current State Analysis

### Key Discoveries:
- LLM prompts use 1-indexed examples but never explicitly state that 0 is invalid (`scud-cli/src/llm/prompts.rs`)
- `parse_prd.rs:161` directly clones LLM dependencies without remapping to actual task IDs
- `expand.rs:298-354` correctly pre-generates UUIDs and remaps 1-indexed references
- `doctor.rs` provides a good validation pattern to model after

## Desired End State

After implementation:
1. All LLM prompts explicitly state that task indices start at 1 (never 0)
2. `parse_prd.rs` correctly remaps 1-indexed LLM dependency references to actual UUIDs
3. A new `check-deps` command validates dependency integrity without requiring AI
4. All changes are covered by unit tests

### Verification:
- Run `cargo test` - all tests pass
- Create a UUID-formatted project with `scud parse-prd --id-format uuid` - dependencies are valid UUIDs
- Run `scud check-deps` - validates dependencies and reports issues

## What We're NOT Doing

- Not changing the SCG file format
- Not modifying how edges are stored on disk
- Not adding AI-powered auto-fix capabilities to check-deps
- Not changing the existing doctor command

---

## Phase 1: Improve LLM Prompts to Prevent Task Zero References

### Overview
Update all three LLM prompts to explicitly prohibit "0" as a task index and clarify that task indices are 1-based.

### Changes Required:

#### 1.1 Update parse_prd Prompt

**File**: `scud-cli/src/llm/prompts.rs`
**Changes**: Add explicit 1-based indexing guidance to the parse_prd prompt

Replace the dependencies line in the JSON template and add a guideline:

```rust
// In the format! macro around line 36-37, update to:
    "dependencies": []  // Use 1-indexed task references, e.g., ["1", "2"]. NEVER use "0" - task indices start at 1.

// Add to Guidelines section around line 51:
- Dependencies use 1-indexed task references (first task is "1", NOT "0")
- NEVER reference task "0" - it does not exist
```

#### 1.2 Update expand_task Prompt

**File**: `scud-cli/src/llm/prompts.rs`
**Changes**: Clarify 1-based indexing in expand_task prompt

Update around lines 157 and 167:

```rust
// Line 157 area:
    "dependencies": []  // 1-indexed subtask refs: ["1", "2"]. NEVER use "0".

// Line 167 area:
- Use 1-indexed dependencies (e.g., ["1"] = first subtask). "0" is INVALID.
```

#### 1.3 Update reanalyze_dependencies Prompt

**File**: `scud-cli/src/llm/prompts.rs`
**Changes**: Add explicit guidance about valid task IDs

Add to the Rules section around line 202:

```rust
- Task IDs are 1-indexed. NEVER suggest dependencies on task "0" or any ID ending in ":0"
- Valid examples: "auth:1", "api:3", "main:10" - Invalid: "auth:0", "0"
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

#### Manual Verification:
- [ ] Run `scud parse-prd` with a test PRD - verify no "0" dependencies in output
- [ ] Run `scud expand --task 1` - verify no "0" dependencies in subtasks

---

## Phase 2: Fix UUID Edge Propagation in parse_prd.rs

### Overview
Implement the same pre-generation and remapping pattern from `expand.rs` in `parse_prd.rs` to correctly handle UUID dependencies.

### Changes Required:

#### 2.1 Pre-generate Task IDs

**File**: `scud-cli/src/commands/ai/parse_prd.rs`
**Changes**: Pre-generate all task IDs before creating tasks

Insert after line 138 (after determining `use_uuid`):

```rust
    // Pre-generate all task IDs so we can remap dependencies
    let task_ids: Vec<String> = parsed_tasks
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            if use_uuid {
                Uuid::new_v4().to_string().replace("-", "")
            } else {
                (start_id + idx).to_string()
            }
        })
        .collect();
```

#### 2.2 Update Task Creation Loop

**File**: `scud-cli/src/commands/ai/parse_prd.rs`
**Changes**: Use pre-generated IDs and remap dependencies

Replace lines 140-164 with:

```rust
    for (idx, parsed) in parsed_tasks.iter().enumerate() {
        let task_id = task_ids[idx].clone();

        let priority = match parsed.priority.to_lowercase().as_str() {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Medium,
        };

        let mut task = Task::new(
            task_id.clone(),
            parsed.title.clone(),
            parsed.description.clone(),
        );
        task.complexity = parsed.complexity;
        task.priority = priority;

        // Map 1-indexed LLM dependency references to actual task IDs
        task.dependencies = parsed
            .dependencies
            .iter()
            .filter_map(|dep| {
                if let Ok(dep_idx) = dep.parse::<usize>() {
                    // Map 1-indexed reference to actual task ID
                    if dep_idx > 0 && dep_idx <= task_ids.len() {
                        Some(task_ids[dep_idx - 1].clone())
                    } else {
                        // Invalid index (0 or out of range) - skip
                        None
                    }
                } else {
                    // Already a full ID reference (cross-phase)
                    Some(dep.clone())
                }
            })
            .collect();

        group.add_task(task);
    }
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] New unit test for dependency remapping passes

#### Manual Verification:
- [ ] Create UUID project: `scud parse-prd test.md --tag test --id-format uuid`
- [ ] Verify dependencies in `.scud/test.scg` are UUIDs, not "1", "2"
- [ ] Run `scud waves` - verify dependency graph is correct

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that UUID dependencies are working correctly.

---

## Phase 3: Create check-deps Command

### Overview
Add a new `check-deps` command that validates dependency integrity without requiring AI. This command will:
- Check that all referenced dependencies exist
- Detect invalid "0" references
- Report circular dependencies
- Suggest fixes for common issues

### Changes Required:

#### 3.1 Create check_deps Module

**File**: `scud-cli/src/commands/check_deps.rs` (new file)

```rust
use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::models::{Phase, TaskStatus};
use crate::storage::Storage;

/// Results from dependency validation
#[derive(Debug, Default)]
pub struct DepCheckResults {
    pub missing_deps: Vec<(String, String, String)>,      // (tag, task_id, missing_dep)
    pub invalid_zero_deps: Vec<(String, String)>,          // (tag, task_id)
    pub self_refs: Vec<(String, String)>,                  // (tag, task_id)
    pub cancelled_deps: Vec<(String, String, String)>,     // (tag, task_id, cancelled_dep)
}

impl DepCheckResults {
    pub fn has_issues(&self) -> bool {
        !self.missing_deps.is_empty()
            || !self.invalid_zero_deps.is_empty()
            || !self.self_refs.is_empty()
            || !self.cancelled_deps.is_empty()
    }

    pub fn issue_count(&self) -> usize {
        self.missing_deps.len()
            + self.invalid_zero_deps.len()
            + self.self_refs.len()
            + self.cancelled_deps.len()
    }
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let all_phases = storage.load_tasks()?;

    if all_phases.is_empty() {
        println!("{}", "No tasks found.".yellow());
        return Ok(());
    }

    // Determine which phases to check
    let phases_to_check: Vec<String> = match tag {
        Some(t) if !all_tags => {
            if !all_phases.contains_key(t) {
                anyhow::bail!("Tag '{}' not found", t);
            }
            vec![t.to_string()]
        }
        _ => all_phases.keys().cloned().collect(),
    };

    println!(
        "{} Checking dependencies across {} phase(s)...\n",
        "Validating".blue(),
        phases_to_check.len()
    );

    let mut results = DepCheckResults::default();

    // Build global task ID set for cross-phase validation
    let all_task_ids: HashSet<String> = all_phases
        .iter()
        .flat_map(|(tag, phase)| {
            phase.tasks.iter().flat_map(move |t| {
                let mut ids = vec![t.id.clone(), format!("{}:{}", tag, t.id)];
                // Also add subtask IDs if present
                for subtask_id in &t.subtasks {
                    ids.push(subtask_id.clone());
                    ids.push(format!("{}:{}", tag, subtask_id));
                }
                ids
            })
        })
        .collect();

    // Validate each phase
    for tag in &phases_to_check {
        if let Some(phase) = all_phases.get(tag) {
            validate_phase(tag, phase, &all_task_ids, &mut results);
        }
    }

    // Print results
    print_results(&results);

    if results.has_issues() {
        std::process::exit(1);
    }

    Ok(())
}

fn validate_phase(
    tag: &str,
    phase: &Phase,
    all_task_ids: &HashSet<String>,
    results: &mut DepCheckResults,
) {
    let local_ids: HashSet<_> = phase.tasks.iter().map(|t| t.id.clone()).collect();

    for task in &phase.tasks {
        // Skip completed/cancelled tasks
        if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
            continue;
        }

        for dep in &task.dependencies {
            // Check for invalid "0" reference
            if dep == "0" || dep.ends_with(":0") {
                results.invalid_zero_deps.push((tag.to_string(), task.id.clone()));
                continue;
            }

            // Check for self-reference
            if dep == &task.id || dep == &format!("{}:{}", tag, task.id) {
                results.self_refs.push((tag.to_string(), task.id.clone()));
                continue;
            }

            // Check if dependency exists
            let exists = local_ids.contains(dep)
                || all_task_ids.contains(dep)
                || all_task_ids.contains(&format!("{}:{}", tag, dep));

            if !exists {
                results.missing_deps.push((
                    tag.to_string(),
                    task.id.clone(),
                    dep.clone(),
                ));
                continue;
            }

            // Check if dependency is cancelled
            if let Some(dep_task) = phase.get_task(dep) {
                if dep_task.status == TaskStatus::Cancelled {
                    results.cancelled_deps.push((
                        tag.to_string(),
                        task.id.clone(),
                        dep.clone(),
                    ));
                }
            }
        }
    }
}

fn print_results(results: &DepCheckResults) {
    if !results.has_issues() {
        println!("{}", "✓ No dependency issues found!".green().bold());
        return;
    }

    // Invalid zero references
    if !results.invalid_zero_deps.is_empty() {
        println!("{}", "Invalid Task Zero References".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id) in &results.invalid_zero_deps {
            println!(
                "  {} Task {} references invalid task \"0\"",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan()
            );
            println!(
                "    {}",
                "→ Task indices start at 1. Remove or update this dependency.".dimmed()
            );
        }
        println!();
    }

    // Missing dependencies
    if !results.missing_deps.is_empty() {
        println!("{}", "Missing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id, dep) in &results.missing_deps {
            println!(
                "  {} Task {} depends on non-existent task {}",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan(),
                dep.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency or create task {}", dep).dimmed()
            );
        }
        println!();
    }

    // Self-references
    if !results.self_refs.is_empty() {
        println!("{}", "Self-Referencing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id) in &results.self_refs {
            println!(
                "  {} Task {} depends on itself",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan()
            );
            println!("    {}", "→ Remove self-referencing dependency.".dimmed());
        }
        println!();
    }

    // Cancelled dependencies
    if !results.cancelled_deps.is_empty() {
        println!("{}", "Dependencies on Cancelled Tasks".yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        for (tag, task_id, dep) in &results.cancelled_deps {
            println!(
                "  {} Task {} depends on cancelled task {}",
                "⚠".yellow(),
                format!("{}:{}", tag, task_id).cyan(),
                dep.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency or un-cancel {}", dep).dimmed()
            );
        }
        println!();
    }

    // Summary
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());
    println!(
        "  Total issues: {}",
        results.issue_count().to_string().red()
    );
    println!();
    println!("{}", "To fix issues:".blue());
    println!("  - Edit .scud/<tag>.scg directly");
    println!("  - Or run: scud reanalyze-deps --apply");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn test_results_has_issues() {
        let mut results = DepCheckResults::default();
        assert!(!results.has_issues());

        results.missing_deps.push(("test".to_string(), "1".to_string(), "99".to_string()));
        assert!(results.has_issues());
    }

    #[test]
    fn test_detect_invalid_zero() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Test".to_string(), "".to_string());
        task.dependencies = vec!["0".to_string()];
        phase.add_task(task);

        let all_ids: HashSet<String> = ["1".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert_eq!(results.invalid_zero_deps.len(), 1);
        assert_eq!(results.invalid_zero_deps[0], ("test".to_string(), "1".to_string()));
    }

    #[test]
    fn test_detect_missing_dep() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Test".to_string(), "".to_string());
        task.dependencies = vec!["99".to_string()];
        phase.add_task(task);

        let all_ids: HashSet<String> = ["1".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert_eq!(results.missing_deps.len(), 1);
    }

    #[test]
    fn test_valid_deps_no_issues() {
        let mut phase = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "First".to_string(), "".to_string());
        let mut task2 = Task::new("2".to_string(), "Second".to_string(), "".to_string());
        task2.dependencies = vec!["1".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);

        let all_ids: HashSet<String> = ["1".to_string(), "2".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert!(!results.has_issues());
    }
}
```

#### 3.2 Register Module in commands/mod.rs

**File**: `scud-cli/src/commands/mod.rs`
**Changes**: Add the new module export

```rust
pub mod check_deps;
```

#### 3.3 Add Command to CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add command enum variant and routing

In the `Commands` enum (around line 150):

```rust
    /// Check dependency validity without AI
    CheckDeps {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Check across all phases
        #[arg(long)]
        all_tags: bool,
    },
```

In the match expression (around line 400):

```rust
        Commands::CheckDeps { tag, all_tags } => {
            commands::check_deps::run(cli.project, tag.as_deref(), all_tags)
        }
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes (including new check_deps tests)
- [ ] `cargo clippy` has no warnings

#### Manual Verification:
- [ ] Run `scud check-deps` on clean project - shows "No issues found"
- [ ] Manually add invalid dependency "0" to a task, run `scud check-deps` - detects issue
- [ ] Run `scud check-deps --all-tags` - validates across all phases

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that the check-deps command works correctly.

---

## Phase 4: Add Unit Tests for parse_prd Dependency Remapping

### Overview
Add comprehensive tests for the new dependency remapping logic in parse_prd.rs.

### Changes Required:

#### 4.1 Add Tests to parse_prd.rs

**File**: `scud-cli/src/commands/ai/parse_prd.rs`
**Changes**: Add test module at the end of the file

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to simulate the dependency remapping logic
    fn remap_dependencies(deps: &[String], task_ids: &[String]) -> Vec<String> {
        deps.iter()
            .filter_map(|dep| {
                if let Ok(dep_idx) = dep.parse::<usize>() {
                    if dep_idx > 0 && dep_idx <= task_ids.len() {
                        Some(task_ids[dep_idx - 1].clone())
                    } else {
                        None
                    }
                } else {
                    Some(dep.clone())
                }
            })
            .collect()
    }

    #[test]
    fn test_remap_sequential_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let deps = vec!["1".to_string(), "2".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        assert_eq!(result, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn test_remap_uuid_deps() {
        let task_ids = vec![
            "abc123def456789012345678901234ab".to_string(),
            "def456abc789012345678901234abcde".to_string(),
            "789012345678901234abcdef12345678".to_string(),
        ];
        let deps = vec!["1".to_string(), "2".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        assert_eq!(result, vec![
            "abc123def456789012345678901234ab".to_string(),
            "def456abc789012345678901234abcde".to_string(),
        ]);
    }

    #[test]
    fn test_filter_zero_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["0".to_string(), "1".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "0" should be filtered out
        assert_eq!(result, vec!["1".to_string()]);
    }

    #[test]
    fn test_filter_out_of_range_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["1".to_string(), "99".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "99" should be filtered out
        assert_eq!(result, vec!["1".to_string()]);
    }

    #[test]
    fn test_preserve_cross_phase_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["1".to_string(), "auth:3".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "auth:3" should be preserved as-is
        assert_eq!(result, vec!["1".to_string(), "auth:3".to_string()]);
    }

    #[test]
    fn test_empty_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps: Vec<String> = vec![];

        let result = remap_dependencies(&deps, &task_ids);

        assert!(result.is_empty());
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo test` passes all new tests
- [ ] `cargo test --lib` specifically tests the library code

---

## Testing Strategy

### Unit Tests:
- Dependency remapping with sequential IDs
- Dependency remapping with UUIDs
- Filtering of invalid "0" references
- Filtering of out-of-range references
- Preservation of cross-phase references
- check_deps validation logic

### Integration Tests:
- End-to-end `parse-prd` with UUID format
- End-to-end `check-deps` with various issue types

### Manual Testing Steps:
1. Create a new project with `scud init`
2. Parse a PRD with `scud parse-prd test.md --tag test --id-format uuid`
3. Verify `.scud/test.scg` has UUID dependencies
4. Run `scud check-deps` - should show no issues
5. Manually corrupt a dependency to "0"
6. Run `scud check-deps` - should detect and report the issue

## Performance Considerations

- The check-deps command uses HashSet for O(1) lookups
- Pre-generating UUIDs in parse_prd is O(n) and adds minimal overhead
- Dependency remapping is O(n*m) where n=tasks and m=avg dependencies per task

## Migration Notes

- No migration needed - changes are backwards compatible
- Existing projects with broken UUID dependencies can be fixed by re-running `scud reanalyze-deps --apply`

## References

- Original research: `thoughts/shared/research/2026-01-10-task-zero-deps-uuid-edges.md`
- Working UUID remapping pattern: `scud-cli/src/commands/ai/expand.rs:298-354`
- Validation pattern: `scud-cli/src/commands/doctor.rs`
