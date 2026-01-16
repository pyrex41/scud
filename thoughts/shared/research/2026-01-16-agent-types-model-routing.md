---
date: 2026-01-16T23:55:11+00:00
researcher: Claude
git_commit: ac33f54d50efa6cdae570c0b1c34ed00fca48f37
branch: claude/durable-execution-agents-QMxda
repository: scud
topic: "Adding Agent Types with Model Routing to SCUD"
tags: [research, codebase, agents, model-routing, spawn, scg-format]
status: complete
last_updated: 2026-01-16
last_updated_by: Claude
---

# Research: Adding Agent Types with Model Routing to SCUD

**Date**: 2026-01-16T23:55:11+00:00
**Researcher**: Claude
**Git Commit**: ac33f54d50efa6cdae570c0b1c34ed00fca48f37
**Branch**: claude/durable-execution-agents-QMxda
**Repository**: scud

## Research Question

How can we add agent/sub-agent types with model routing to SCUD so that:
1. Tasks can specify which agent type should run them
2. Each agent type has a prompt template and default model
3. Agent assignments are stored in the task graph (.scg files)
4. Spawn command routes tasks to the right model/harness (claude for Anthropic, opencode for xAI)

## Summary

SCUD currently has the infrastructure to support agent-based model routing with minimal changes. The key pieces are:

1. **Task struct** needs a new optional `agent_type: Option<String>` field
2. **SCG format** needs a new `@agents` section or extension to `@nodes`
3. **Agent definitions** can be stored in `.scud/agents/<name>.toml` with prompt templates and model defaults
4. **Spawn command** already supports multiple harnesses (Claude, OpenCode) - needs to route based on task's agent type
5. **Config** already has smart/fast model tiers that could be leveraged

The estimated changes are ~400-600 lines across 5-6 files.

## Detailed Findings

### 1. Current Task Structure

**Location**: `/home/user/scud/scud-cli/src/models/task.rs:75-114`

The `Task` struct currently has these fields:
```rust
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub complexity: u32,
    pub priority: Priority,
    pub dependencies: Vec<String>,
    pub parent_id: Option<String>,
    pub subtasks: Vec<String>,
    pub details: Option<String>,
    pub test_strategy: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub assigned_to: Option<String>,  // Informational only
}
```

**Gap**: No field for agent type or model preference.

**Change needed**: Add `pub agent_type: Option<String>` field.

### 2. Current SCG Format

**Location**: `/home/user/scud/scud-cli/src/formats/scg.rs`

Current sections in `.scg` files:
- `@meta` - Phase metadata (name, id_format, updated)
- `@nodes` - Task definitions (id | title | status | complexity | priority)
- `@edges` - Dependencies (dependent -> dependency)
- `@parents` - Parent-child relationships
- `@assignments` - Who tasks are assigned to
- `@details` - Multiline fields (description, details, test_strategy)

**Gap**: No section for agent types.

**Option A**: Add `@agents` section:
```
@agents
# id | agent_type
1 | builder
2 | reviewer
3 | planner
```

**Option B**: Extend `@nodes` format:
```
@nodes
# id | title | status | complexity | priority | agent_type
1 | Setup project | P | 3 | H | builder
```

Option A is cleaner for backwards compatibility (empty section = use default).

### 3. Current Spawn Command

**Location**: `/home/user/scud/scud-cli/src/commands/spawn/`

Key files:
- `mod.rs:37-278` - Main spawn logic
- `terminal.rs:271-301` - Terminal spawning dispatch
- `agent.rs:8-76` - Prompt generation

**Current behavior**:
- Hardcodes `Harness::Claude` (line 279 in terminal.rs)
- No model selection per task
- Uses same prompt template for all tasks

**Swarm command** (`/home/user/scud/scud-cli/src/commands/swarm/mod.rs:88-90`):
- Has `-H/--harness` flag for global harness selection
- Already calls `spawn_terminal_with_harness()` with selected harness

**Change needed**:
1. Read agent_type from task
2. Load agent definition (prompt, model, harness)
3. Pass harness and model to spawn

### 4. Current Harness Abstraction

**Location**: `/home/user/scud/scud-cli/src/commands/spawn/terminal.rs:12-65`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Harness {
    #[default]
    Claude,    // --dangerously-skip-permissions
    OpenCode,  // run subcommand
}
```

**Binary resolution** (lines 94-176):
- Claude: `/opt/homebrew/bin/claude`, `~/.local/bin/claude`, etc.
- OpenCode: `/opt/homebrew/bin/opencode`, `~/.bun/bin/opencode`, etc.

**Command generation** (lines 51-64):
- Claude: `'<binary>' "$(cat '<prompt>')" --dangerously-skip-permissions`
- OpenCode: `'<binary>' run "$(cat '<prompt>')"`

**Gap**: No model flag passed to CLI. Claude CLI supports `--model <model>`.

**Change needed**: Update `Harness::command()` to accept optional model parameter:
```rust
fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
    match self {
        Harness::Claude => {
            let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
            format!(
                r#"'{}' "$(cat '{}')" --dangerously-skip-permissions{}"#,
                binary_path, prompt_file.display(), model_flag
            )
        }
        Harness::OpenCode => {
            let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
            format!(
                r#"'{}' run{} "$(cat '{}')""#,
                binary_path, model_flag, prompt_file.display()
            )
        }
    }
}
```

### 5. Current Prompt Generation

**Location**: `/home/user/scud/scud-cli/src/commands/spawn/agent.rs:8-76`

```rust
pub fn generate_prompt(task: &Task, tag: &str) -> String {
    // Fixed template with task fields interpolated
}
```

**Gap**: No agent-specific prompt customization.

**Change needed**: Load prompt template from agent definition or use default.

### 6. Current Config Structure

**Location**: `/home/user/scud/scud-cli/src/config.rs`

```rust
pub struct Config {
    pub llm: LLMConfig,  // For SCUD's own AI ops, not spawned agents
}

pub struct LLMConfig {
    pub provider: String,       // xai, anthropic, claude-cli, etc.
    pub model: String,
    pub smart_provider: String, // For validation tasks
    pub smart_model: String,    // opus by default
    pub fast_provider: String,  // For generation tasks
    pub fast_model: String,     // grok-code-fast-1 by default
    pub max_tokens: u32,
}
```

**Note**: This config is for SCUD's internal LLM usage (parse-prd, expand, etc.), NOT for spawned agents.

**Gap**: No agent definitions config.

## Code References

### Files to Modify

| File | Change | Est. Lines |
|------|--------|------------|
| `scud-cli/src/models/task.rs` | Add `agent_type` field | +10 |
| `scud-cli/src/formats/scg.rs` | Add `@agents` section parsing/serialization | +80 |
| `scud-cli/src/commands/spawn/terminal.rs` | Add model parameter to command | +20 |
| `scud-cli/src/commands/spawn/mod.rs` | Load agent def, route to model/harness | +100 |
| `scud-cli/src/commands/spawn/agent.rs` | Load prompt from agent definition | +60 |
| `scud-cli/src/agents/mod.rs` (NEW) | Agent definition loading | +150 |

**Total**: ~420 lines

### Agent Definition Format (Proposed)

**Location**: `.scud/agents/<name>.toml`

```toml
# .scud/agents/builder.toml
[agent]
name = "builder"
description = "Fast code implementation agent"

[model]
harness = "claude"      # or "opencode"
model = "sonnet"        # CLI model name
# Alternatively for xAI:
# harness = "opencode"
# model = "grok-4"

[prompt]
# Either inline or file reference
template = """
You are a code implementation agent. Work quickly and efficiently.

Task: {task.title}
Description: {task.description}

Focus on clean, working code. Run tests when done.
"""
# Or: template_file = "builder-prompt.md"
```

**Default agents** (could ship with SCUD):
- `builder` - Fast implementation (sonnet/grok)
- `reviewer` - Code review (opus)
- `planner` - Architecture planning (opus)
- `analyzer` - Complexity analysis (opus)

### Flow After Changes

1. **Task creation**: User sets `agent_type` via CLI or PRD parsing
2. **Spawn command**:
   ```rust
   // Load agent definition
   let agent = if let Some(agent_type) = &task.agent_type {
       AgentDef::load(agent_type, &project_root)?
   } else {
       AgentDef::default()  // Use default builder
   };

   // Generate prompt (may use agent's template)
   let prompt = agent.generate_prompt(task, tag);

   // Get harness and model
   let harness = agent.harness();  // Claude or OpenCode
   let model = agent.model();      // sonnet, opus, grok-4, etc.

   // Spawn with model
   terminal::spawn_terminal_with_harness_and_model(
       &terminal, task_id, &prompt, &working_dir,
       &session_name, harness, Some(model)
   )?;
   ```

3. **Terminal spawning**: Passes `--model <model>` flag to CLI

### SCG Format Extension

```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
  id_format sequential
  updated 2026-01-16T10:30:00Z
}

@nodes
# id | title | status | complexity | priority
1 | Design auth flow | P | 5 | H
2 | Implement JWT | P | 8 | H
3 | Review implementation | P | 3 | M

@edges
2 -> 1
3 -> 2

@agents
# id | agent_type
1 | planner
2 | builder
3 | reviewer

@details
1 | description |
  Design the authentication flow
```

## Architecture Documentation

### Current Architecture
```
User runs: scud spawn --tag auth
    ↓
spawn/mod.rs: get_ready_tasks()
    ↓
spawn/agent.rs: generate_prompt(task, tag)
    ↓
spawn/terminal.rs: spawn_terminal() → hardcoded Claude
    ↓
Claude CLI: claude "<prompt>" --dangerously-skip-permissions
```

### Proposed Architecture
```
User runs: scud spawn --tag auth
    ↓
spawn/mod.rs: get_ready_tasks()
    ↓
For each task:
    ↓
agents/mod.rs: load_agent(task.agent_type)
    ↓
spawn/agent.rs: generate_prompt(task, tag, agent)  // Uses agent's template
    ↓
spawn/terminal.rs: spawn_with_harness_and_model(harness, model)
    ↓
Claude CLI: claude "<prompt>" --dangerously-skip-permissions --model sonnet
   OR
OpenCode: opencode run --model grok-4 "<prompt>"
```

## Related Research

- Durable execution research in task context (CLAUDE.md)
- Descartes agent definitions (referenced in context)

## Open Questions

1. **Agent definition discovery**: Should agents be in `.scud/agents/` or also support `.claude/agents/` for compatibility with Descartes concepts?

2. **Default agent**: Should there be a default agent when none specified? Current behavior is effectively "builder with claude/default-model".

3. **Prompt inheritance**: Should agent prompts completely replace or augment the default prompt?

4. **Model validation**: Should we validate that the model name is valid for the harness before spawning?

5. **OpenCode model flags**: Need to verify exact CLI flags for opencode (may be `--model` or different).

6. **Cross-phase agent assignment**: If task `auth:1` depends on `api:2`, should agent types be inherited or independent?
