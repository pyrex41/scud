---
date: 2026-01-10T17:50:43+00:00
researcher: Claude
git_commit: e6576dc8620495f7d61dd90c29268834881a6731
branch: claude/fix-task-zero-deps-2DkGO
repository: scud
topic: "Task Zero Dependencies and UUID Edge Propagation Issues"
tags: [research, codebase, dependencies, uuid, llm-prompts, edges]
status: complete
last_updated: 2026-01-10
last_updated_by: Claude
---

# Research: Task Zero Dependencies and UUID Edge Propagation Issues

**Date**: 2026-01-10T17:50:43+00:00
**Researcher**: Claude
**Git Commit**: e6576dc8620495f7d61dd90c29268834881a6731
**Branch**: claude/fix-task-zero-deps-2DkGO
**Repository**: scud

## Research Question

Investigate:
1. How task dependencies are managed and validated
2. Where task zero references might be incorrectly created
3. How UUID task IDs are generated and assigned
4. How edges (dependencies) are stored and whether they use numeric IDs or UUIDs
5. The reanalyze-deps command and prompting used for dependency review

## Summary

### Key Findings

1. **Task Zero Reference Issue**: The LLM prompts do not explicitly prohibit "0" as a task index, leading AI models to sometimes generate references to non-existent "task 0" when reviewing dependencies.

2. **UUID Edge Propagation Bug**: When tasks are created via `parse-prd` with UUID format, the LLM returns 1-indexed dependency references (e.g., `["1", "2"]`) that are stored directly WITHOUT being remapped to the actual UUID task IDs. This breaks dependency tracking for UUID-formatted projects.

3. **No Standalone Dependency Review Command**: Currently `reanalyze-deps` is the only dependency review command, but it's an AI-powered command that suggests changes. There's no simple "check" or "validate" command for reviewing dependencies without LLM involvement.

## Detailed Findings

### 1. Task Dependency Management

**Core Implementation**: `scud-cli/src/models/task.rs:86`
- Dependencies stored as `Vec<String>` on each Task struct
- Supports both sequential IDs (`"1"`, `"2"`) and UUID format
- Cross-tag dependencies supported via namespaced IDs (e.g., `"auth:1"`, `"api:3"`)

**Validation**: `scud-cli/src/commands/doctor.rs`
- Detects missing dependencies
- Detects cancelled/blocked dependencies
- NO validation that dependency IDs actually exist in the task graph

**Graph Operations**: `scud-cli/src/commands/waves.rs`
- Uses Kahn's algorithm for topological sort
- Builds in-degree map from task dependencies
- Computes execution waves based on dependency DAG

### 2. Task Zero Reference Problem

**Source of Issue**: `scud-cli/src/llm/prompts.rs`

The prompts use 1-indexed examples but don't explicitly state that:
- Task indices start at 1, not 0
- "0" is never a valid task reference

**parse_prd prompt** (line 51):
```
"dependencies": []  // use task indices, e.g., ["1", "2"]
```

**expand_task prompt** (lines 157, 167):
```
"dependencies": []  // Array of strings: ["1", "2", "3"] for subtask dependencies
...
- Use dependencies to enforce correct order (e.g., ["1"] means depends on first subtask)
```

**reanalyze_dependencies prompt** (line 183):
```
- Use full task IDs with phase prefix (e.g., "auth:1", "api:3")
```

**Problem**: When a smart model reviews dependencies, it may:
- Generate "0" thinking tasks are 0-indexed
- Reference "task 0" as a conceptual "foundation" task
- The prompts don't explicitly state "task IDs start at 1" or "0 is invalid"

### 3. UUID Edge Propagation Bug

**UUID Generation**: Occurs in two places:
1. `scud-cli/src/commands/ai/parse_prd.rs:143` - For new tasks from PRD
2. `scud-cli/src/commands/ai/expand.rs:302` - For subtasks during expansion

**The Bug in parse_prd.rs** (lines 140-164):
```rust
for (idx, parsed) in parsed_tasks.iter().enumerate() {
    let task_id = if use_uuid {
        // Generate UUID v4 as 32-character hex string
        Uuid::new_v4().to_string().replace("-", "")
    } else {
        (start_id + idx).to_string()
    };
    // ...
    task.dependencies = parsed.dependencies.clone();  // BUG: No remapping!
    group.add_task(task);
}
```

The LLM returns dependencies as `["1", "2"]` (1-indexed references), but these are stored directly without being remapped to the actual generated UUIDs.

**Contrast with expand.rs** (correctly handles this, lines 336-354):
```rust
new_task.dependencies = expanded
    .dependencies
    .iter()
    .filter_map(|dep| {
        if let Ok(dep_idx) = dep.parse::<usize>() {
            // Map 1-indexed reference to actual subtask ID
            if dep_idx > 0 && dep_idx <= idx + 1 {
                Some(subtask_ids[dep_idx - 1].clone())
            } else {
                None
            }
        } else {
            // Already a full ID reference
            Some(dep.clone())
        }
    })
    .collect();
```

The expand command correctly:
1. Pre-generates all UUIDs (`subtask_ids` vector)
2. Maps 1-indexed LLM references to actual UUIDs
3. Validates index bounds (dep_idx > 0)

**parse_prd.rs does NOT do this remapping**, so UUID projects have broken dependencies.

### 4. Edge Storage Format

**In-Memory**: `scud-cli/src/models/task.rs:86`
```rust
pub dependencies: Vec<String>
```

**On-Disk (SCG Format)**: `scud-cli/src/formats/scg.rs:240-244`
```
@edges
# dependent -> dependency
task1 -> task2
```

Edges are stored as task ID strings, which can be either:
- Sequential: `"1"`, `"2"`, `"1.1"`
- UUID: `"a1b2c3d4e5f6789012345678901234ab"`
- Cross-tag: `"auth:1"`, `"api:a1b2c3d4..."`

### 5. reanalyze-deps Command

**Location**: `scud-cli/src/commands/ai/reanalyze_deps.rs`

**Process**:
1. Builds task context from all phases (`build_task_context`)
2. Sends context to LLM with `Prompts::reanalyze_dependencies`
3. LLM returns `DependencySuggestion` array with add/remove changes
4. User confirms changes (or uses `--apply` flag)
5. Changes applied to task dependencies

**Prompt Issues** (from prompts.rs:183-229):
- Uses examples like `"auth:1"`, `"api:3"` - numeric IDs only
- No explicit mention that task indices start at 1
- No UUID-formatted examples provided
- No guidance on how to reference UUID tasks

## Code References

### UUID Generation
- `scud-cli/src/commands/ai/parse_prd.rs:143` - Task creation from PRD
- `scud-cli/src/commands/ai/expand.rs:302` - Subtask creation during expansion

### Dependency Handling
- `scud-cli/src/models/task.rs:86` - Dependencies field on Task struct
- `scud-cli/src/models/task.rs:287-307` - `has_dependencies_met()` and `has_dependencies_met_refs()`
- `scud-cli/src/formats/scg.rs:240-244` - Edge parsing from SCG format
- `scud-cli/src/formats/scg.rs:438-451` - Edge serialization to SCG format

### LLM Prompts
- `scud-cli/src/llm/prompts.rs:4-58` - `parse_prd` prompt
- `scud-cli/src/llm/prompts.rs:107-181` - `expand_task` prompt
- `scud-cli/src/llm/prompts.rs:183-230` - `reanalyze_dependencies` prompt

### Dependency Validation
- `scud-cli/src/commands/doctor.rs:169-173` - Dependency validation checking
- `scud-cli/src/commands/waves.rs:206-265` - Wave computation with dependency graph

## Architecture Documentation

### Task ID Formats

The system supports two ID formats tracked by `IdFormat` enum:
- `Sequential`: Numeric IDs like `"1"`, `"2"`, with subtasks as `"1.1"`, `"1.2"`
- `Uuid`: 32-character hex strings like `"a1b2c3d4e5f6789012345678901234ab"`

### Dependency Reference Types

1. **Local references**: `"1"`, `"2"` - within same phase
2. **Namespaced references**: `"auth:1"`, `"api:3"` - cross-phase
3. **Subtask references**: `"1.1"`, `"1.2"` - hierarchical
4. **UUID references**: Full 32-char hex strings

### LLM Interaction Flow

1. **parse-prd**: LLM returns dependencies as 1-indexed strings
2. **expand**: LLM returns dependencies as 1-indexed strings
3. **reanalyze-deps**: LLM returns full task IDs with phase prefix

## Open Questions

1. Should `doctor` command validate that all referenced dependencies actually exist?
2. Should there be a non-AI "check-deps" command for simple validation?
3. Should UUID tasks display a shortened ID in prompts/output?
