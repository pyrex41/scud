# SCUD Task Manager

<p align="center">
  <img src="scud.jpeg" alt="SCUD" width="400">
</p>

> *Inspired by the SCUD short-range ballistic missile system—lightweight, flexible, and powerful. Like its namesake, SCUD can be deployed quickly in a variety of contexts, delivering results with minimal overhead.*

A fast, AI-powered task management system. Parse PRDs into tasks, track dependencies, and visualize parallel execution waves.

---

## Quick Start

### Install

**From crates.io (recommended):**
```bash
cargo install scud-cli
scud init
```

**From source:**
```bash
# Clone the repository
git clone https://github.com/pyrex41/scud.git
cd scud

# Build the CLI
cargo build --release

# Install globally (optional)
./target/release/scud install

# Or use directly
./target/release/scud init
```

**Quick install script:**
```bash
curl -fsSL https://raw.githubusercontent.com/pyrex41/scud/main/install.sh | bash
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
scud view
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

### Pure Rust CLI
- **50x faster** than JavaScript alternatives
- **42x fewer tokens** (500 vs 21k)
- **Single binary** - zero runtime dependencies
- **Self-contained** - embedded assets, no external files

### DAG-Driven Execution
- **Dependency graphs** - tasks ready when deps complete
- **Parallel waves** - visualize concurrent work with `scud waves`
- **Smart scheduling** - `scud next` finds ready tasks

### Web Dashboard
- **Visual task board** - `scud view` opens browser dashboard
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
scud view                          # Open task viewer in browser
scud mermaid [--tag <tag>]         # Generate Mermaid diagram
```

### AI Commands (Requires XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD/doc into tasks
scud parse <file> --tag <tag> --no-guidance  # Parse without project guidance
scud analyze-complexity            # Analyze task complexity
scud expand --all                  # Break down complex tasks
scud expand --all --no-guidance    # Expand without project guidance
```

Default model: `grok-code-fast-1`. Configure with `scud config set-provider <provider> --model <model>`.

Project guidance files in `.scud/guidance/*.md` are automatically included in AI prompts.

### Orchestrator Commands
```bash
scud assign <id> <name>            # Assign task to a developer
scud who-is [--tag <tag>]          # See who's working on what
scud next-batch [--limit 5]        # Get multiple ready tasks
scud doctor [--tag <tag>]          # Diagnose stuck task states
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
scud view
# Opens task viewer in browser
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

- **Rust toolchain** (for building from source)
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
├── guidance/                 # Project guidance for AI prompts
│   └── *.md                  # Markdown files auto-loaded
└── logs/                     # Task log entries
```

### Project Guidance

You can provide project-specific context that will be automatically included in AI prompts. Create markdown files in `.scud/guidance/`:

```bash
# Example: Add coding standards
echo "# Coding Standards
- Use TypeScript strict mode
- All functions must have JSDoc comments
- Maximum function length: 50 lines" > .scud/guidance/coding-standards.md

# Example: Add architecture notes
echo "# Architecture
- Frontend: React with hooks
- Backend: Express.js
- Database: PostgreSQL" > .scud/guidance/architecture.md
```

All `.md` files in this folder are automatically loaded when running `scud parse` or `scud expand`. Use `--no-guidance` to skip loading guidance.

---

## Development

```bash
# Build Rust CLI
cd scud-cli
cargo build --release

# Install globally
./target/release/scud install

# Or use directly from build directory
./target/release/scud init
```

**Publishing:** See [PUBLISHING.md](PUBLISHING.md) for CI/CD setup and release process.

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
