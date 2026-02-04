# CLI Agent Configuration & Generate Pipeline Fixes

## Overview

This plan addressed two related issues:
1. **Generate Pipeline Fixes**: Deterministic agent_type assignment based on complexity, and warning for invalid dependencies
2. **CLI Interactive Configuration**: Add `scud config spawn-agents configure` command to modify harness/model settings

## Implementation Summary

### 1. Deterministic Agent Type Assignment (parse_prd.rs)

**File**: `scud-cli/src/commands/ai/parse_prd.rs`

Changed agent_type assignment from passthrough (`task.agent_type = parsed.agent_type.clone()`) to deterministic based on complexity:

```rust
// Deterministically assign agent_type based on complexity
// Keep special types (reviewer, planner, tester) if LLM identified them
task.agent_type = Some(match parsed.agent_type.as_deref() {
    Some("reviewer") => "reviewer".to_string(),
    Some("planner") => "planner".to_string(),
    Some("tester") => "tester".to_string(),
    _ => {
        // For implementation tasks, use complexity to determine agent
        if parsed.complexity <= 2 {
            "fast-builder".to_string()
        } else {
            "builder".to_string()
        }
    }
});
```

**Rules**:
- `reviewer`, `planner`, `tester` → preserved from LLM (task-nature-based)
- complexity 0-2 → `fast-builder`
- complexity 3+ → `builder`

### 2. Deterministic Agent Type for Subtasks (expand.rs)

**File**: `scud-cli/src/commands/ai/expand.rs`

Added agent_type assignment when creating subtasks (subtasks have complexity=0 by definition):

```rust
new_task.agent_type = Some("fast-builder".to_string());
```

### 3. Invalid Dependency Warnings

**Files**: `parse_prd.rs` and `expand.rs`

Added warning messages when filtering out invalid dependencies (task "0" or out-of-range indices):

```rust
eprintln!(
    "  {} Task {}: skipping invalid dependency '{}' (indices are 1-{})",
    "⚠".yellow(),
    task_id,
    dep,
    task_ids.len()
);
```

### 4. Interactive Agent Configuration Command

**Files**: `main.rs`, `config.rs`

Added `scud config agents configure [name]` command:

- Lists installed agents and prompts for selection (or uses provided name)
- Shows current harness and model configuration
- Interactive prompts to select new harness (claude/opencode)
- Interactive prompts to select model (with provider-specific suggestions)
- Option to enter custom model name
- Saves changes to `.scud/agents/<name>.toml`

## Usage

### Generate Pipeline
```bash
# Tasks now get deterministic agent_type based on complexity
scud generate prd.md --tag my-feature

# Subtasks from expand also get agent_type
scud expand --tag my-feature
```

### Agent Configuration
```bash
# List installed agents
scud config spawn-agents list

# Configure a specific agent
scud config agents configure builder

# Configure interactively (prompts for selection)
scud config agents configure
```

## Testing

- Build: `cargo build -p scud-cli` ✓
- Tests: `cargo test -p scud-cli` ✓ (181 passed)

## Files Changed

1. `scud-cli/src/commands/ai/parse_prd.rs` - Deterministic agent_type + dependency warnings
2. `scud-cli/src/commands/ai/expand.rs` - Subtask agent_type + dependency warnings
3. `scud-cli/src/main.rs` - Added Configure subcommand
4. `scud-cli/src/commands/config.rs` - Implemented spawn_agents_configure()
