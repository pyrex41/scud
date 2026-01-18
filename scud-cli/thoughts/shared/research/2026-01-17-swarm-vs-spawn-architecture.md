---
date: 2026-01-17T21:25:00-08:00
researcher: Claude
git_commit: a4361c64c07c82ea44ebe22af365a984e1d9937b
branch: master
repository: scud-cli
topic: "Swarm vs Spawn Architecture Comparison"
tags: [research, codebase, spawn, swarm, agents, terminal]
status: complete
last_updated: 2026-01-17
last_updated_by: Claude
---

# Research: Swarm vs Spawn Architecture Comparison

**Date**: 2026-01-17T21:25:00-08:00
**Researcher**: Claude
**Git Commit**: a4361c64c07c82ea44ebe22af365a984e1d9937b
**Branch**: master
**Repository**: scud-cli

## Research Question

Compare and contrast the `scud swarm` and `scud spawn` commands - how they relate, what each does, and the gap for running a single agent with an arbitrary prompt.

## Summary

**Swarm and Spawn are NOT orthogonal - Swarm USES Spawn's terminal functions.** The relationship is:

- **Spawn** = Low-level terminal spawning + basic multi-agent launcher
- **Swarm** = High-level orchestrator that uses spawn's terminal functions

Both currently require tasks from the SCUD task graph. Neither supports arbitrary prompts directly.

## Detailed Findings

### Spawn Module (`src/commands/spawn/`)

**Purpose**: Spawn multiple tmux windows with AI agents for SCUD tasks

**Architecture**:
```
spawn/
├── mod.rs       # Main entry point, task selection logic
├── agent.rs     # Prompt generation for tasks
├── terminal.rs  # Low-level tmux spawning (Harness enum, spawn_tmux)
├── hooks.rs     # Claude Code hooks for task completion
├── monitor.rs   # Session state tracking
└── tui/         # TUI monitor for spawn sessions
```

**Key Functions** (`terminal.rs:181-231`):
```rust
pub fn spawn_terminal(task_id, prompt, working_dir, session_name) -> Result<String>
pub fn spawn_terminal_with_harness(task_id, prompt, working_dir, session_name, harness) -> Result<String>
pub fn spawn_terminal_with_harness_and_model(task_id, prompt, working_dir, session_name, harness, model) -> Result<String>
```

**Flow** (`mod.rs:37-329`):
1. Load tasks from SCUD task graph
2. Filter to "ready" tasks (pending, dependencies met)
3. Spawn up to `--limit` agents in tmux windows
4. Optionally claim tasks (mark in-progress)
5. Optionally attach to tmux or start TUI monitor

**CLI Options**:
- `--limit <N>` - Max agents to spawn (default: 5)
- `--harness <H>` - AI harness: claude, opencode
- `--model <M>` - Model to use
- `--monitor` - Start TUI after spawn
- `--attach` - Attach to tmux session
- `--claim` - Mark tasks as in-progress
- `--dry-run` - Show plan without spawning

**Key Insight**: Spawn DOES accept a `prompt` parameter at the terminal level, but the CLI always generates prompts from tasks via `agent::generate_prompt()`.

### Swarm Module (`src/commands/swarm/`)

**Purpose**: Wave-based parallel execution with backpressure validation

**Architecture**:
```
swarm/
├── mod.rs       # Main orchestration loop
└── session.rs   # Session state (WaveState, RoundState, SwarmSession)
```

**Flow** (`mod.rs:47-446`):
```
Wave 1:
  [Research] Optional task analysis
  [Build] Round 1: spawn N agents, wait for completion
  [Build] Round 2: spawn N agents, wait for completion
  ...
  [Validate] Run backpressure (cargo build, test, etc)
  [Review] Optional reviewer agent
  [Repair] If validation failed, attribute + fix

Wave 2:
  ... repeat for next dependency wave
```

**Key Difference from Spawn**:
- **Waits for completion**: `wait_for_round_completion()` polls task status
- **Backpressure validation**: Runs build/test after each wave
- **Repair loop**: Attributes failures, spawns repair agents
- **Review agents**: Optional code review step

**Uses Spawn's Terminal Functions** (`mod.rs:600-606`):
```rust
match terminal::spawn_terminal_with_harness(
    &info.task.id,
    &prompt,
    working_dir,
    session_name,
    harness,
) { ... }
```

### Agent Prompt Generation (`agent.rs`)

**`generate_prompt(task, tag)`** (`agent.rs:9-77`):
```
You are working on SCUD task {id}: {title}

Tag: {tag}
Complexity: {complexity}
Priority: {priority}
Description: {description}
[Technical Details if present]
[Test Strategy if present]
[Dependencies if present]

Instructions:
1. Explore codebase to understand context
2. Implement following project conventions
3. Write tests if applicable
4. When complete: scud set-status {id} done
5. If blocked: scud set-status {id} blocked
```

Also has: `generate_minimal_prompt()`, `generate_prompt_with_template()`, `generate_review_prompt()`, `generate_repair_prompt()`

### Terminal Spawning (`terminal.rs`)

**Harness Enum** (`terminal.rs:12-70`):
```rust
pub enum Harness {
    Claude,   // claude "$(cat prompt)" --dangerously-skip-permissions [--model M]
    OpenCode, // opencode [--model M] run "$(cat prompt)"
}
```

**spawn_tmux()** (`terminal.rs:235-324`):
1. Create/check tmux session
2. Create new window with `-P -F "#{window_index}"` to capture index
3. Write prompt to temp file
4. Send harness command to window with PATH setup
5. Return window index for easy attachment

## Comparison Table

| Feature | Spawn | Swarm |
|---------|-------|-------|
| **Purpose** | Launch agents | Orchestrate full workflow |
| **Task source** | SCUD task graph | SCUD task graph |
| **Arbitrary prompts** | No (CLI level) | No |
| **Multiple agents** | Yes (--limit) | Yes (--round-size) |
| **Waits for completion** | No | Yes |
| **Backpressure validation** | No | Yes |
| **Repair on failure** | No | Yes |
| **Review step** | No | Yes |
| **TUI monitor** | Yes (--monitor) | No |
| **Claims tasks** | Optional (--claim) | Always |

## The Gap: Single Agent with Arbitrary Prompt

**Current state**: Neither command supports running a single agent with an arbitrary prompt like:
```bash
scud spawn -p "Write hello world in Rust"
# or
scud run --harness opencode "Write hello world in Rust"
```

**The low-level capability exists** in `terminal.rs`:
```rust
pub fn spawn_terminal_with_harness_and_model(
    task_id: &str,
    prompt: &str,        // <-- accepts arbitrary prompt
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
    model: Option<&str>,
) -> Result<String>
```

**What's missing**: A CLI entry point that:
1. Accepts a prompt directly (not from task graph)
2. Creates a "virtual" task ID (or uses "adhoc")
3. Calls the existing terminal spawning functions

## Code References

- `src/commands/spawn/mod.rs:37-329` - Spawn entry point
- `src/commands/spawn/terminal.rs:181-231` - Terminal spawn functions
- `src/commands/spawn/agent.rs:9-77` - Prompt generation
- `src/commands/swarm/mod.rs:47-446` - Swarm orchestration loop
- `src/commands/swarm/mod.rs:586-643` - execute_round() uses spawn terminal

## Architecture Documentation

```
User
  │
  ├── scud spawn ────► spawn/mod.rs ────► get_ready_tasks() ────► generate_prompt() ────┐
  │                                                                                      │
  │                                                                                      ▼
  │                                                                           terminal::spawn_tmux()
  │                                                                                      │
  └── scud swarm ────► swarm/mod.rs ────► compute_waves() ────► execute_round() ────────┘
                           │
                           ├── wait_for_round_completion()
                           ├── run_validation()
                           ├── spawn_reviewer()
                           └── run_repair_loop()
```

## Open Questions

1. Should `scud spawn` gain a `-p/--prompt` flag for arbitrary prompts?
2. Should there be a new `scud run` command for single-agent execution?
3. How should "virtual" task IDs work for ad-hoc prompts?
