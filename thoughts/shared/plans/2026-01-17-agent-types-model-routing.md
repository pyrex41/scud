# Agent Types with Model Routing Implementation Plan

## Overview

Add agent/sub-agent types with model routing to SCUD so that:
1. Tasks can specify which agent type should run them (builder, reviewer, planner, etc.)
2. Each agent type has a prompt template and default model (sonnet/opus/haiku for Claude, grok for xAI via OpenCode)
3. Agent assignments are stored in the task graph (.scg files)
4. Spawn command routes tasks to the right model/harness

## Current State Analysis

**What exists:**
- Task struct (`models/task.rs:74-114`) - has `assigned_to` but no `agent_type`
- SCG format (`formats/scg.rs`) - supports `@assignments` but no `@agents` section
- Harness enum (`spawn/terminal.rs:12-65`) - Claude and OpenCode, but no model parameter
- Spawn command (`spawn/mod.rs`) - hardcodes `spawn_terminal()` with no model routing

**Key Discoveries:**
- `Harness::command()` at `terminal.rs:51-64` generates CLI command without `--model` flag
- `spawn_terminal_with_harness()` at `terminal.rs:283-301` already supports harness selection
- SCG parser at `formats/scg.rs:166-186` handles unknown sections gracefully (just skips them)
- Swarm command already has `-H/--harness` flag for global harness selection

## Desired End State

After implementation:
1. Tasks have an optional `agent_type` field that can be set via CLI or imported
2. `.scg` files have an `@agents` section mapping task IDs to agent types
3. Agent definitions live in `.scud/agents/<name>.toml` with prompt templates and model defaults
4. `scud spawn` reads task's agent type, loads definition, and spawns with correct harness/model
5. Both Claude CLI (`--model sonnet`) and OpenCode (`--model grok-4`) are supported

**Verification:**
```bash
# Create a task with agent type
scud add "Review auth code" --agent reviewer

# Verify in SCG file
grep -A5 "@agents" .scud/tasks/tasks.scg

# Spawn should use opus model (from reviewer agent definition)
scud spawn --dry-run
# Output should show: Harness: claude, Model: opus
```

## What We're NOT Doing

- **NOT** adding prompt inheritance/augmentation - agents completely replace the default prompt
- **NOT** adding model validation - we trust the user to provide valid model names
- **NOT** adding agent definition inheritance - each agent is standalone
- **NOT** adding CLI commands to manage agents - users edit TOML files directly
- **NOT** changing existing swarm command behavior - it keeps its `-H` flag

---

## Phase 1: Core Data Model

### Overview
Add `agent_type` field to Task struct and `@agents` section to SCG format.

### Changes Required:

#### 1.1 Task Struct

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add `agent_type` field

```rust
// After line 113 (assigned_to field), add:
    /// Agent type for model routing (e.g., "builder", "reviewer", "planner")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
```

Update `Task::new()` at line 125-143:
```rust
    pub fn new(id: String, title: String, description: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Task {
            id,
            title,
            description,
            status: TaskStatus::Pending,
            complexity: 0,
            priority: Priority::Medium,
            dependencies: Vec::new(),
            parent_id: None,
            subtasks: Vec::new(),
            details: None,
            test_strategy: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
            assigned_to: None,
            agent_type: None,  // Add this
        }
    }
```

#### 1.2 SCG Format - Parser

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Add `@agents` section parsing

In `parse_scg()` function, add after line 149:
```rust
    let mut agent_types: HashMap<String, String> = HashMap::new();
```

Add to section detection at line 177-186:
```rust
            current_section = Some(match trimmed {
                "@meta {" | "@meta" => "meta",
                "@nodes" => "nodes",
                "@edges" => "edges",
                "@parents" => "parents",
                "@assignments" => "assignments",
                "@agents" => "agents",  // Add this
                "@details" => "details",
                _ => continue,
            });
```

Add section handler after line 274 (after `Some("assignments")` block):
```rust
            Some("agents") => {
                // Parse "id | agent_type"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 && !parts[1].is_empty() {
                    agent_types.insert(parts[0].clone(), parts[1].clone());
                }
            }
```

Apply agent types after line 343 (after assignments are applied):
```rust
    // Apply agent types
    for (id, agent_type) in agent_types {
        if let Some(task) = tasks.get_mut(&id) {
            task.agent_type = Some(agent_type);
        }
    }
```

#### 1.3 SCG Format - Serializer

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Add `@agents` section serialization

In `serialize_scg()` function, add after line 487 (after assignments section):
```rust
    // Agents section
    let agents: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.agent_type.is_some())
        .collect();

    if !agents.is_empty() {
        output.push_str("@agents\n");
        output.push_str("# id | agent_type\n");
        for task in agents {
            output.push_str(&format!(
                "{} | {}\n",
                task.id,
                task.agent_type.as_deref().unwrap_or("")
            ));
        }
        output.push('\n');
    }
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] All existing tests pass: `cargo test -p scud-cli`
- [ ] New SCG round-trip test passes (add test for agent_type)

#### Manual Verification:
- [ ] Edit a `.scg` file to add `@agents` section manually
- [ ] Run `scud list` - should parse without errors
- [ ] Run `scud show <id>` - should display agent type if present

---

## Phase 2: Harness Model Support

### Overview
Add `--model` flag support to terminal spawning so Claude/OpenCode receive model parameter.

### Changes Required:

#### 2.1 Harness Command Generation

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add model parameter to `Harness::command()`

Replace `command()` method at lines 51-64:
```rust
    /// Generate the command to run with a prompt and optional model
    pub fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
        match self {
            Harness::Claude => {
                let model_flag = model
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                format!(
                    r#"'{}' "$(cat '{}')" --dangerously-skip-permissions{}"#,
                    binary_path,
                    prompt_file.display(),
                    model_flag
                )
            }
            Harness::OpenCode => {
                let model_flag = model
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                format!(
                    r#"'{}'{} run "$(cat '{}')""#,
                    binary_path,
                    model_flag,
                    prompt_file.display()
                )
            }
        }
    }
```

#### 2.2 Update spawn_terminal Functions

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Thread model parameter through all spawn functions

Update `spawn_terminal()` at lines 271-280:
```rust
pub fn spawn_terminal(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
) -> Result<()> {
    spawn_terminal_with_harness_and_model(
        terminal, task_id, prompt, working_dir, session_name,
        Harness::Claude, None
    )
}
```

Add new function after `spawn_terminal_with_harness()`:
```rust
/// Spawn a new terminal window/pane with specific harness and model
pub fn spawn_terminal_with_harness_and_model(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
    model: Option<&str>,
) -> Result<()> {
    let binary_path = find_harness_binary(harness)?;

    match terminal {
        Terminal::Kitty => spawn_kitty(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::Wezterm => spawn_wezterm(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::ITerm2 => spawn_iterm2(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::Zellij => spawn_zellij(task_id, prompt, working_dir, session_name, binary_path, harness, model),
        Terminal::Tmux => spawn_tmux(task_id, prompt, working_dir, session_name, binary_path, harness, model),
    }
}
```

Update `spawn_terminal_with_harness()` to call the new function:
```rust
pub fn spawn_terminal_with_harness(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
) -> Result<()> {
    spawn_terminal_with_harness_and_model(
        terminal, task_id, prompt, working_dir, session_name, harness, None
    )
}
```

#### 2.3 Update Individual Terminal Spawn Functions

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add model parameter to each spawn function

Update signatures for all spawn functions:
- `spawn_kitty()` - add `model: Option<&str>` parameter
- `spawn_wezterm()` - add `model: Option<&str>` parameter
- `spawn_iterm2()` - add `model: Option<&str>` parameter
- `spawn_zellij()` - add `model: Option<&str>` parameter
- `spawn_tmux()` - add `model: Option<&str>` parameter

In each function, update the `harness.command()` call:
```rust
// Change from:
let harness_cmd = harness.command(binary_path, &prompt_file);
// To:
let harness_cmd = harness.command(binary_path, &prompt_file, model);
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] All existing tests pass: `cargo test -p scud-cli`

#### Manual Verification:
- [ ] Spawn a task with `--dry-run` and verify no errors
- [ ] Manually call `spawn_terminal_with_harness_and_model()` with model and verify command output

---

## Phase 3: Agent Definitions Module

### Overview
Create a new module for loading agent definitions from `.scud/agents/<name>.toml` files.

### Changes Required:

#### 3.1 Create Agents Module

**File**: `scud-cli/src/agents/mod.rs` (NEW)
**Changes**: New file with agent definition structs and loading logic

```rust
//! Agent definitions for model routing
//!
//! Agent definitions specify which harness and model to use for a task,
//! along with an optional custom prompt template.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::commands::spawn::terminal::Harness;

/// Agent definition loaded from .scud/agents/<name>.toml
#[derive(Debug, Clone, Deserialize)]
pub struct AgentDef {
    pub agent: AgentMeta,
    pub model: ModelConfig,
    #[serde(default)]
    pub prompt: PromptConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Harness to use: "claude" or "opencode"
    #[serde(default = "default_harness")]
    pub harness: String,
    /// Model name to pass to CLI (e.g., "sonnet", "opus", "grok-4")
    #[serde(default)]
    pub model: Option<String>,
}

fn default_harness() -> String {
    "claude".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PromptConfig {
    /// Inline prompt template (supports {task.title}, {task.description}, etc.)
    pub template: Option<String>,
    /// Path to prompt template file (relative to .scud/agents/)
    pub template_file: Option<String>,
}

impl AgentDef {
    /// Load agent definition from .scud/agents/<name>.toml
    pub fn load(name: &str, project_root: &Path) -> Result<Self> {
        let path = project_root
            .join(".scud")
            .join("agents")
            .join(format!("{}.toml", name));

        if !path.exists() {
            anyhow::bail!(
                "Agent definition '{}' not found at {}",
                name,
                path.display()
            );
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read agent file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse agent file: {}", path.display()))
    }

    /// Try to load agent definition, return None if not found
    pub fn try_load(name: &str, project_root: &Path) -> Option<Self> {
        Self::load(name, project_root).ok()
    }

    /// Get the harness for this agent
    pub fn harness(&self) -> Result<Harness> {
        Harness::parse(&self.model.harness)
    }

    /// Get the model name for this agent (if specified)
    pub fn model(&self) -> Option<&str> {
        self.model.model.as_deref()
    }

    /// Get the prompt template (if specified)
    pub fn prompt_template(&self, project_root: &Path) -> Option<String> {
        // Try inline template first
        if let Some(ref template) = self.prompt.template {
            return Some(template.clone());
        }

        // Try template file
        if let Some(ref template_file) = self.prompt.template_file {
            let path = project_root
                .join(".scud")
                .join("agents")
                .join(template_file);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Some(content);
            }
        }

        None
    }

    /// Create a default agent (Claude with sonnet, no custom prompt)
    pub fn default_builder() -> Self {
        AgentDef {
            agent: AgentMeta {
                name: "builder".to_string(),
                description: "Default code implementation agent".to_string(),
            },
            model: ModelConfig {
                harness: "claude".to_string(),
                model: Some("sonnet".to_string()),
            },
            prompt: PromptConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_agent_definition() {
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_file = agents_dir.join("reviewer.toml");
        let mut file = std::fs::File::create(&agent_file).unwrap();
        writeln!(
            file,
            r#"
[agent]
name = "reviewer"
description = "Code review agent"

[model]
harness = "claude"
model = "opus"
"#
        )
        .unwrap();

        let agent = AgentDef::load("reviewer", temp.path()).unwrap();
        assert_eq!(agent.agent.name, "reviewer");
        assert_eq!(agent.model.harness, "claude");
        assert_eq!(agent.model.model, Some("opus".to_string()));
    }

    #[test]
    fn test_agent_not_found() {
        let temp = TempDir::new().unwrap();
        let result = AgentDef::load("nonexistent", temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_default_builder() {
        let agent = AgentDef::default_builder();
        assert_eq!(agent.agent.name, "builder");
        assert_eq!(agent.model.model, Some("sonnet".to_string()));
    }
}
```

#### 3.2 Register Module

**File**: `scud-cli/src/lib.rs` (or `main.rs` depending on structure)
**Changes**: Add module declaration

```rust
pub mod agents;
```

#### 3.3 Create Example Agent Definitions

**File**: `.scud/agents/builder.toml` (example, not shipped)
```toml
[agent]
name = "builder"
description = "Fast code implementation agent"

[model]
harness = "claude"
model = "sonnet"

[prompt]
# Optional: custom prompt template
# template = "You are a fast implementation agent..."
```

**File**: `.scud/agents/reviewer.toml` (example)
```toml
[agent]
name = "reviewer"
description = "Code review agent using smarter model"

[model]
harness = "claude"
model = "opus"

[prompt]
template = """
You are a thorough code reviewer. Review the following task carefully:

Task: {task.title}
Description: {task.description}

Focus on:
- Code correctness
- Security issues
- Performance concerns
- Code style and maintainability
"""
```

**File**: `.scud/agents/xai-builder.toml` (example for xAI)
```toml
[agent]
name = "xai-builder"
description = "Fast builder using xAI Grok"

[model]
harness = "opencode"
model = "grok-4"
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] New agent tests pass: `cargo test -p scud-cli agents`

#### Manual Verification:
- [ ] Create `.scud/agents/test.toml` with valid content
- [ ] Verify `AgentDef::load("test", &project_root)` works in test

---

## Phase 4: Wire Up Spawn Command

### Overview
Update spawn command to read agent type from task, load agent definition, and route to correct harness/model.

### Changes Required:

#### 4.1 Update Spawn Logic

**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Load agent definition and use for spawning

Add import at top:
```rust
use crate::agents::AgentDef;
use crate::commands::spawn::terminal::Harness;
```

Update the spawn loop starting at line 145. Replace:
```rust
    for info in &ready_tasks {
        let prompt = agent::generate_prompt(info.task, &info.tag);

        match terminal::spawn_terminal(
            &terminal,
            &info.task.id,
            &prompt,
            &working_dir,
            &session_name,
        ) {
```

With:
```rust
    for info in &ready_tasks {
        // Load agent definition if task has agent_type
        let agent_def = info.task.agent_type.as_ref()
            .and_then(|agent_type| AgentDef::try_load(agent_type, &working_dir))
            .unwrap_or_else(AgentDef::default_builder);

        // Get harness and model from agent definition
        let harness = agent_def.harness().unwrap_or(Harness::Claude);
        let model = agent_def.model();

        // Generate prompt (use agent's template if available, otherwise default)
        let prompt = match agent_def.prompt_template(&working_dir) {
            Some(template) => agent::generate_prompt_with_template(info.task, &info.tag, &template),
            None => agent::generate_prompt(info.task, &info.tag),
        };

        match terminal::spawn_terminal_with_harness_and_model(
            &terminal,
            &info.task.id,
            &prompt,
            &working_dir,
            &session_name,
            harness,
            model,
        ) {
```

#### 4.2 Update Agent Prompt Generation

**File**: `scud-cli/src/commands/spawn/agent.rs`
**Changes**: Add function to generate prompt from template

Add new function:
```rust
/// Generate prompt using a custom template
/// Supports placeholders: {task.id}, {task.title}, {task.description}, {tag}
pub fn generate_prompt_with_template(task: &Task, tag: &str, template: &str) -> String {
    template
        .replace("{task.id}", &task.id)
        .replace("{task.title}", &task.title)
        .replace("{task.description}", &task.description)
        .replace("{tag}", tag)
        .replace("{task.details}", task.details.as_deref().unwrap_or(""))
        .replace("{task.test_strategy}", task.test_strategy.as_deref().unwrap_or(""))
}
```

#### 4.3 Update Spawn Display

**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Show harness/model in spawn output

Update the spawn output around line 156-161:
```rust
            Ok(()) => {
                let model_info = model.map(|m| format!(" ({})", m)).unwrap_or_default();
                println!(
                    "  {} Spawned: {} | {} [{}{}]",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed(),
                    harness.name(),
                    model_info.dimmed(),
                );
                // ... rest unchanged
```

#### 4.4 Add --agent Flag to spawn command (optional enhancement)

**File**: `scud-cli/src/commands/spawn/mod.rs` (clap args)
**Changes**: Allow overriding agent type from command line

This is optional but useful for testing. Add to clap args:
```rust
    /// Override agent type for all spawned tasks
    #[clap(long, help = "Override agent type (e.g., 'reviewer', 'builder')")]
    agent: Option<String>,
```

Then in the spawn loop, use this override if provided:
```rust
        let agent_type = cli_agent.as_ref().or(info.task.agent_type.as_ref());
        let agent_def = agent_type
            .and_then(|t| AgentDef::try_load(t, &working_dir))
            .unwrap_or_else(AgentDef::default_builder);
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] All tests pass: `cargo test -p scud-cli`
- [ ] Lint passes: `cargo clippy -p scud-cli`

#### Manual Verification:
- [ ] Create `.scud/agents/reviewer.toml` with `model = "opus"`
- [ ] Create a task with `@agents` section assigning it to "reviewer"
- [ ] Run `scud spawn --dry-run` and verify output shows correct harness/model
- [ ] Actually spawn and verify Claude CLI receives `--model opus`

---

## Testing Strategy

### Unit Tests:
- `Task` serialization/deserialization with `agent_type` field
- SCG round-trip with `@agents` section
- `AgentDef::load()` with valid/invalid files
- `Harness::command()` with model parameter

### Integration Tests:
- End-to-end: Create task with agent → spawn → verify model flag in command
- Backward compatibility: Existing SCG files without `@agents` still parse

### Manual Testing Steps:
1. Create `.scud/agents/reviewer.toml`:
   ```toml
   [agent]
   name = "reviewer"
   [model]
   harness = "claude"
   model = "opus"
   ```
2. Add `@agents` section to a `.scg` file:
   ```
   @agents
   1 | reviewer
   ```
3. Run `scud spawn --dry-run` and verify output
4. Run actual spawn and check terminal command includes `--model opus`

---

## Performance Considerations

- Agent definition loading is done per-task, but files are small (~100 bytes) - no caching needed
- No additional network calls or heavy computation introduced

## Migration Notes

- No migration needed - new fields are optional and default to current behavior
- Existing SCG files without `@agents` section continue to work unchanged
- Existing spawn commands without agent definitions use `default_builder()` (Claude/sonnet)

## References

- Research document: `thoughts/shared/research/2026-01-16-agent-types-model-routing.md`
- Related: CLAUDE.md durable execution research
- Claude CLI model flag: `claude --model <model>`
- OpenCode model flag: `opencode --model <model> run`
