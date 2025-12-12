# SCUD Task Manager

<p align="center">
  <img src="scud.jpeg" alt="SCUD" width="400">
</p>

> *Inspired by the SCUD short-range ballistic missile system—lightweight, flexible, and powerful. Like its namesake, SCUD can be deployed quickly in a variety of contexts, delivering results with minimal overhead.*

A fast, AI-powered task management system. Parse PRDs into tasks, track dependencies, and visualize parallel execution waves.

---

## Quick Start

### Install

**Using pnpm (recommended):**
```bash
pnpm add -g scud-task
cd your-project
scud init
```

**Using npm:**
```bash
npm install -g scud-task
cd your-project
scud init
```

### Basic Usage
```bash
# Create tasks from a PRD or feature doc
scud parse docs/feature.md --tag my-feature

# View tasks and dependencies
scud list --tag my-feature
scud waves --tag my-feature    # Show parallel execution plan

# Find and work on next ready task
scud next --tag my-feature
scud set-status 1 in-progress

# When done, mark complete
scud set-status 1 done

# Visualize in browser
scud serve
```

**Quick reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
**Orchestrator pattern:** [docs/orchestrator.md](docs/orchestrator.md)

---

## Core Concepts

### SCG Format

Tasks are stored in **SCG (SCUD Graph)** format—a token-efficient, human-readable text format that achieves ~75% token reduction compared to JSON. SCG explicitly represents the task dependency graph with sections for nodes, edges, and metadata. Inspired in part by Nikolai Mushegian's [JAMS spec](https://nikolai.fyi/jams/) ([GitHub](https://github.com/nmushegian/jams)).

```
@nodes
auth:1 | Design auth system | X | 13 | H
auth:1.1 | Implement JWT | D | 5 | H

@edges
auth:1.1 -> auth:1
```

**Full spec:** [docs/reference/SCG_FORMAT_SPEC.md](docs/reference/SCG_FORMAT_SPEC.md)

### DAG-Driven Execution
Tasks become ready when their dependencies complete. No manual phase management required.

```
Task 1 ──┐
         ├──> Task 3 ──> Task 5
Task 2 ──┘      │
                └──> Task 4
```

### Tags
Group related tasks together (e.g., `auth-system`, `payment-flow`). Each tag has its own task graph.

### Parallel Execution
Use orchestrator patterns to spawn multiple Claude Code agents in parallel, each working on a ready task. See [docs/orchestrator.md](docs/orchestrator.md).

---

## Key Features

### Fast Rust CLI
- **50x faster** than JavaScript alternatives
- **42x fewer tokens** (500 vs 21k)
- **Single binary** - no dependencies

### DAG-Driven Execution
- **Dependency graphs** - tasks ready when deps complete
- **Parallel waves** - visualize concurrent work with `scud waves`
- **Smart scheduling** - `scud next` finds ready tasks

### Web Dashboard
- **Visual task board** - `scud serve` opens browser dashboard
- **Mermaid diagrams** - dependency graph visualization
- **Real-time stats** - progress tracking

### Orchestrator Support
- **Parallel agents** - spawn multiple Claude instances
- **Task locking** - `scud claim/release` prevents conflicts
- **Session monitoring** - `scud whois` tracks active work

---

## Documentation

**Getting Started:**
- [Quick Reference](docs/reference/QUICK_REFERENCE.md) - Command cheat sheet
- [SCG Format Spec](docs/reference/SCG_FORMAT_SPEC.md) - Task file format

**Patterns:**
- [Orchestrator Pattern](docs/orchestrator.md) - Parallel execution guide
- [Parallel Features](docs/features/PARALLEL_FEATURES.md) - Task locking & orchestration

**Development:**
- [Development Logs](log_docs/) - Implementation details & history

---

## Commands

### Setup
```bash
scud init                          # Initialize SCUD in current directory
scud warmup                        # Quick session orientation
```

### Core Commands (Instant)
```bash
scud tags                          # List all tags
scud tags <tag>                    # Set active tag
scud list [--tag <tag>]            # List tasks
scud show <id>                     # Show task details
scud next [--tag <tag>]            # Find next ready task
scud set-status <id> <status>      # Update task status
scud stats [--tag <tag>]           # Show statistics
scud waves [--tag <tag>]           # Show parallel execution waves
```

### Visualization
```bash
scud serve                         # Start web dashboard (port 3000)
scud mermaid [--tag <tag>]         # Generate Mermaid diagram
```

### AI Commands (Requires XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD/doc into tasks
scud analyze-complexity            # Analyze task complexity
scud expand --all                  # Break down complex tasks
```

Default model: `grok-3-mini`. Configure with `scud config --provider <provider> --model <model>`.

### Orchestrator Commands
```bash
scud claim <id> --name <name>      # Claim task (lock)
scud release <id>                  # Release task lock
scud whois [--tag <tag>]           # See who's working on what
scud doctor [--tag <tag>]          # Check for stale locks
```

### Utilities
```bash
scud log <id> "message"            # Add log entry to task
scud log-show <id>                 # Show task log entries
scud commit [-m "msg"]             # Git commit with task context
scud clean [--tag <tag>]           # Clear tasks (with confirmation)
```

---

## Example Workflow

```bash
# 1. Initialize
scud init

# 2. Create tasks from PRD
scud parse docs/feature.md --tag auth-system
# Creates tasks with dependencies

# 3. View execution plan
scud waves --tag auth-system
# Shows which tasks can run in parallel

# 4. Work on next ready task
scud next --tag auth-system
# Returns: Task 1 is ready

scud set-status 1 in-progress
# ... do the work ...
scud set-status 1 done

# 5. Track progress
scud stats --tag auth-system
# Shows progress: 8/10 complete

# 6. Visualize
scud serve
# Opens web dashboard with task graph
```

See [docs/orchestrator.md](docs/orchestrator.md) for parallel execution patterns.

---

## Why SCUD?

**DAG-Driven:**
- Tasks become ready when dependencies complete
- Visualize parallel execution waves
- Smart scheduling finds ready work

**Fast & Simple:**
- Rust CLI is instant (<50ms)
- SCG format is human-readable and git-friendly
- Works offline (core commands)
- No vendor lock-in

**Visual:**
- Web dashboard with task board
- Mermaid dependency diagrams
- Real-time progress tracking

**Orchestrator-Ready:**
- Spawn parallel Claude agents
- Task locking prevents conflicts
- Monitor active sessions
- Doctor command finds stale work

---

## Requirements

- **Node.js 16+** (for pnpm/npm package wrapper)
- **xAI API key** (for AI features only; core commands work offline)

```bash
export XAI_API_KEY=xai-...
```

Alternative providers: Anthropic (`ANTHROPIC_API_KEY`), OpenAI (`OPENAI_API_KEY`), OpenRouter (`OPENROUTER_API_KEY`). Configure with `scud config`.

---

## File Structure

```
.scud/
├── tasks/tasks.scg           # All tasks in SCG format
├── config.toml               # Provider/model settings
├── active-tag                # Currently active tag
├── current-task              # Active task ID (for commits)
└── logs/                     # Task log entries
```

---

## Development

```bash
# Build Rust CLI
cd scud-cli
cargo build --release

# The binary will be at:
# scud-cli/target/release/scud
```

---

## Contributing

Issues and PRs welcome at [github.com/pyrex41/scud](https://github.com/pyrex41/scud)

---

## License

MIT

---

## Learn More

- **Quick Reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
- **SCG Format:** [docs/reference/SCG_FORMAT_SPEC.md](docs/reference/SCG_FORMAT_SPEC.md)
- **Orchestrator Pattern:** [docs/orchestrator.md](docs/orchestrator.md)
- **Parallel Features:** [docs/features/PARALLEL_FEATURES.md](docs/features/PARALLEL_FEATURES.md)

**Happy building!**
