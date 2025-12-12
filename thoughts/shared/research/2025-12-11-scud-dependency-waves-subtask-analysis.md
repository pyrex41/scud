---
date: 2025-12-11T20:50:00-06:00
researcher: pyrex41
git_commit: 890db51856f65f1f71c9ee5b9b05593b7eb976d6
branch: master
repository: scud
topic: "SCUD Dependency Creation, Waves Multi-Tag, and Subtask Configuration"
tags: [research, codebase, dependencies, waves, expand, configuration]
status: complete
last_updated: 2025-12-11
last_updated_by: pyrex41
---

# Research: SCUD Dependency Creation, Waves Multi-Tag, and Subtask Configuration

**Date**: 2025-12-11T20:50:00-06:00
**Researcher**: pyrex41
**Git Commit**: 890db51856f65f1f71c9ee5b9b05593b7eb976d6
**Branch**: master
**Repository**: scud

## Research Questions

1. Why do dependencies sometimes reference "task 0" when there is no task 0?
2. Does `scud waves` support viewing all tags at once?
3. How can the subtask granularity be controlled?
4. How can the maximum number of parallel subtasks be changed?

## Summary

### 1. Dependency "Task 0" Issue

**Root Cause**: The LLM may return 0-based indices despite the prompt requesting 1-based indices.

**How SCUD handles task IDs**:
- Task IDs **start at 1** throughout the codebase
- PRD parsing: `start_id = 1` at `parse_prd.rs:75`
- Subtask creation: `idx + 1` at `expand.rs:272`

**The problem**: When parsing PRDs, dependencies from the LLM are **directly copied without validation**:
```rust
// parse_prd.rs:111
task.dependencies = parsed.dependencies.clone();
```

If the LLM returns `["0", "1"]` instead of `["1", "2"]`, these invalid references are stored as-is, creating dangling dependencies to non-existent "task 0".

**Task expansion is safer**: The expand command validates dependencies:
```rust
// expand.rs:299
if dep_idx > 0 && dep_idx <= idx + 1 {
    Some(format!("{}.{}", parent_id, dep_idx))
} else {
    None  // Filters out 0-based or invalid indices
}
```

### 2. Waves Multi-Tag Support

**Yes, `--all-tags` flag exists!**

```bash
scud waves --all-tags           # View waves across ALL tags
scud waves --tag my-feature     # View waves for specific tag
scud waves                      # View waves for active tag (default)
```

**Implementation** (`waves.rs:29-44`):
- `--all-tags` flag defined at `main.rs:160-161`
- When set, collects all phase tags: `all_tasks.keys().cloned().collect()`
- Task IDs are namespaced (e.g., `auth:1`, `api:3`) to avoid collisions

**Additional flag**:
- `--max-parallel` or `-n`: Controls tasks per round (default: 5)

### 3. Subtask Granularity Configuration

**No configuration file setting exists** - values are hardcoded in Rust.

**Current mapping** (`task.rs:325-333`):
```rust
match complexity {
    0..=3 => 0,  // No expansion
    5 => 2,      // 2 subtasks
    8 => 2,      // 2 subtasks
    13 => 3,     // 3 subtasks
    _ => 3,      // 3 subtasks max (for 21+)
}
```

**Expansion threshold**: Tasks with complexity >= 5 are eligible for expansion (`task.rs:312`)

**To change granularity**: Must modify source code at:
- `/Users/reuben/gauntlet/scud/scud-cli/src/models/task.rs:325-333`

### 4. Max Parallel Subtasks Configuration

**Two different "parallel" concepts exist**:

#### A. LLM Concurrency (expand command)
How many tasks are expanded simultaneously via LLM API:
- **Hardcoded**: `CONCURRENCY = 10` at `expand.rs:34`
- Used with `buffer_unordered(CONCURRENCY)` at `expand.rs:239`

#### B. Wave Parallelism (waves command)
How many tasks per wave round:
- **Configurable via CLI**: `scud waves --max-parallel 8`
- Default: 5 (`main.rs:156`)
- Minimum: 1

**To change LLM concurrency**: Modify `expand.rs:34`
**To change wave parallelism**: Use `--max-parallel` flag or change default at `main.rs:156`

---

## Detailed Findings

### Dependency Creation Flow

#### PRD Parsing (`parse_prd.rs`)

1. User runs `scud parse feature.md --tag auth`
2. PRD content sent to LLM with prompt requesting 1-based indices
3. LLM returns JSON with dependencies like `["1", "2"]`
4. **Dependencies copied directly without validation** (line 111)
5. Tasks saved to `.scud/tasks/tasks.scg`

**Prompt instruction** (`prompts.rs:34`):
```
"Identify dependencies where tasks must be done in specific order (use task indices, e.g., [\"1\", \"2\"])"
```

#### Task Expansion (`expand.rs`)

1. User runs `scud expand --all`
2. Each task with complexity >= 5 is expanded
3. LLM returns subtask dependencies like `["1", "2"]`
4. **Dependencies validated and filtered** (lines 294-308):
   - Must be > 0
   - Must reference previously created subtasks
   - Invalid indices silently dropped
5. Mapped to full IDs: `"1"` → `"parent.1"`

### Waves Command Implementation

**File**: `/Users/reuben/gauntlet/scud/scud-cli/src/commands/waves.rs`

**Algorithm**: Kahn's topological sort with level assignment

**Phase selection logic** (lines 29-44):
```rust
let phase_tags: Vec<String> = if all_tags {
    all_tasks.keys().cloned().collect()  // ALL tags
} else if let Some(tag) = tag {
    // Validate specific tag exists
    vec![tag.to_string()]
} else {
    // Fall back to active phase
    storage.get_active_group()?.map_or_else(
        || bail!("No active task group"),
        |t| vec![t]
    )
}
```

**Task filtering** (lines 51-67):
- Excludes: Done, Expanded, Cancelled tasks
- Includes subtasks only if parent is Expanded

### Subtask Generation Settings

**File**: `/Users/reuben/gauntlet/scud/scud-cli/src/models/task.rs`

**Expansion eligibility** (line 312):
```rust
pub fn needs_expansion(&self) -> bool {
    self.complexity >= 5 && !self.is_expanded() && !self.is_subtask()
}
```

**Recommended subtasks** (lines 325-333):
```rust
pub fn recommended_subtasks_for_complexity(complexity: u32) -> usize {
    match complexity {
        0..=3 => 0,
        5 => 2,
        8 => 2,
        13 => 3,
        _ => 3,
    }
}
```

**LLM prompt flexibility** (`prompts.rs:134`):
```
"Aim for {N} subtasks total (can vary by 1-2 if needed for logical breakdown)"
```

### Configuration Files

**config.toml** (`/Users/reuben/gauntlet/scud/scud-cli/src/config.rs`):
```toml
[llm]
provider = "xai"
model = "grok-code-fast-1"
max_tokens = 4096
```

**No settings for**:
- Subtask count limits
- Expansion threshold
- LLM concurrency
- Default wave parallelism

---

## Code References

### Dependency Creation
- `scud-cli/src/commands/ai/parse_prd.rs:111` - Direct dependency assignment (no validation)
- `scud-cli/src/commands/ai/expand.rs:294-308` - Dependency validation for subtasks
- `scud-cli/src/llm/prompts.rs:34` - LLM prompt for 1-based indices

### Waves Command
- `scud-cli/src/main.rs:149-162` - CLI argument definitions
- `scud-cli/src/commands/waves.rs:15-199` - Main implementation
- `scud-cli/src/commands/waves.rs:30-31` - `--all-tags` handling
- `scud-cli/src/commands/waves.rs:201-278` - Wave computation algorithm

### Subtask Configuration
- `scud-cli/src/models/task.rs:309-333` - Expansion logic and subtask counts
- `scud-cli/src/commands/ai/expand.rs:34` - `CONCURRENCY = 10`
- `scud-cli/src/llm/prompts.rs:90-145` - Expansion prompt

---

## Potential Improvements (for consideration)

### 1. Fix Task 0 Dependency Issue
Add validation in `parse_prd.rs` after line 111:
```rust
task.dependencies = parsed.dependencies
    .iter()
    .filter(|d| d.parse::<usize>().map(|n| n > 0).unwrap_or(true))
    .cloned()
    .collect();
```

### 2. Make Subtask Settings Configurable
Add to `config.toml`:
```toml
[expand]
threshold = 5
max_subtasks = 3
llm_concurrency = 10
```

### 3. Improve LLM Prompt
Strengthen the dependency instruction in `prompts.rs`:
```
"Dependencies MUST use 1-based indices (1, 2, 3...). Never use 0."
```

---

## Quick Reference

| Setting | Location | Default | CLI Flag |
|---------|----------|---------|----------|
| Wave parallelism | `main.rs:156` | 5 | `--max-parallel` |
| All tags | `waves.rs:30` | false | `--all-tags` |
| LLM concurrency | `expand.rs:34` | 10 | None |
| Expansion threshold | `task.rs:312` | 5 | None |
| Max subtasks | `task.rs:325-333` | 2-3 | None |

---

## Related Research

None yet.

## Open Questions

1. Should dependency validation be added to PRD parsing?
2. Should subtask configuration be exposed in config.toml?
3. Should LLM prompts more explicitly require 1-based indexing?
