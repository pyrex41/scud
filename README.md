# SCUD Task Manager

<p align="center">
  <img src="README_SCREENSHOT.png" alt="SCUD Task Manager" width="600">
</p>

> *Inspired by the SCUD short-range ballistic missile system—lightweight, flexible, and powerful. Like its namesake, SCUD can be deployed quickly in a variety of contexts, delivering results with minimal overhead.*

A fast, AI-powered task management system with both CLI and desktop GUI interfaces. Parse PRDs into tasks, track dependencies, visualize parallel execution waves, and orchestrate AI agents.

## Components

### 🚀 SCUD CLI
Fast, lightweight command-line interface for task management and AI orchestration.

### 🖥️ Descartes GUI
Desktop application for visual task management and real-time AI agent control.

---

## Quick Start

### Choose Your Interface

#### CLI Interface (SCUD)
For developers who prefer terminal workflows and scripting.

#### GUI Interface (Descartes)
For visual task management and real-time agent control.

### Install

#### SCUD CLI
```bash
# From crates.io (recommended)
cargo install scud-cli
scud init

# From source
git clone https://github.com/pyrex41/scud.git
cd scud/scud-cli
cargo build --release
./target/release/scud init
```

#### Descartes GUI
```bash
# From crates.io (recommended)
cargo install descartes-gui

# From source
git clone https://github.com/pyrex41/scud.git
cd scud/descartes-gui
cargo build --release
./target/release/descartes-gui
```

### Basic Usage
```bash
# Initialize SCUD project
scud init

# Generate tasks from PRD (full pipeline: parse → expand → validate)
scud generate docs/feature.md --tag my-feature

# Or just parse PRD into initial tasks
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

#### Descartes GUI Workflow
```bash
# Initialize SCUD first (GUI reads from SCUD data)
scud init
scud generate docs/feature.md --tag my-feature

# Launch GUI
descartes-gui

# In GUI: View waves, start tasks, monitor progress
```

**SCUD CLI Quick Reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
**Orchestrator Pattern:** [docs/orchestrator.md](docs/orchestrator.md)
**Descartes GUI Details:** [descartes-gui/README.md](descartes-gui/README.md)

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

SCUD supports two execution strategies for parallel agent orchestration:

**Wave Mode** (default): Batches ready tasks into waves, spawns agents, waits for completion, then validates before next wave.
```bash
scud swarm --tag my-feature --swarm-mode wave
```

**Beads Mode**: Continuous polling for ready tasks. Spawns agents immediately when dependencies complete—no waiting for batch boundaries.
```bash
scud swarm --tag my-feature --swarm-mode beads
```

Both modes support tmux-based terminal spawning with live monitoring, SQLite event logging, and automatic salvo worktree provisioning. See [docs/orchestrator.md](docs/orchestrator.md) for details.

### Salvo Worktrees

When swarming a tag, SCUD automatically provisions an isolated git worktree so parallel swarms on different tags don't conflict:

```bash
scud swarm --tag backend     # Auto-creates ../myproject.salvo.backend/
scud swarm --tag frontend    # Auto-creates ../myproject.salvo.frontend/
# Both run in parallel without conflicts

scud salvo list              # See all active worktrees
scud salvo remove backend    # Clean up when done
```

Each worktree gets a filtered task file with full detail for its tag and collapsed stubs for others. Results auto-sync back to main on completion. See [docs/orchestrator.md](docs/orchestrator.md) for details.

---

## Descartes GUI

The desktop GUI provides a visual interface for task management and AI agent orchestration, complementing the SCUD CLI.

### Features

#### Wave Visualization
- **Interactive waves**: Click to start tasks in parallel execution waves
- **Dependency awareness**: Visual representation of task relationships
- **Status tracking**: Real-time progress indicators

#### Agent Control
- **One-click execution**: Start agents directly from the GUI
- **Real-time control**: Pause, resume, or cancel running agents
- **Live monitoring**: Stream agent output and progress

#### Task Management
- **Visual task board**: Drag-and-drop task organization
- **Status updates**: Mark tasks complete directly in the interface
- **Progress tracking**: Overall project completion statistics

### Setup
```bash
# Install GUI
cargo install descartes-gui

# Initialize SCUD project first
scud init
scud generate docs/feature.md --tag my-project

# Launch GUI
descartes-gui
```

The GUI reads from SCUD's `.scud/` storage directory and provides a user-friendly interface for the same underlying task management system.

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

### Desktop GUI (Descartes)
- **Wave visualization** - Interactive task waves with one-click execution
- **Real-time agent control** - Pause, resume, cancel running agents
- **Live output streaming** - Monitor agent progress in real-time
- **Visual task management** - Drag-and-drop task organization

### Orchestrator Support
- **Parallel agents** - spawn multiple Claude instances via tmux
- **Salvo worktrees** - automatic git worktree isolation per tag
- **SQLite event storage** - queryable event logs, transcript search, session history
- **Transcript capture** - real-time import of Claude Code conversation logs
- **Live monitoring** - heartbeat tracking, orphan detection, stale timeouts
- **Task locking** - `scud claim/release` prevents conflicts
- **Session monitoring** - `scud whois` tracks active work

---

## Documentation

**Getting Started:**
- [Quick Reference](docs/reference/QUICK_REFERENCE.md) - SCUD CLI command cheat sheet
- [SCG Format Spec](docs/reference/SCG_FORMAT_SPEC.md) - Task file format
- [Descartes GUI](descartes-gui/README.md) - Desktop interface guide

**Swarm & Orchestration:**
- [Orchestrator Pattern](docs/orchestrator.md) - Swarm modes, salvo worktrees, transcripts, and multi-agent workflows
- [Parallel Features](docs/features/PARALLEL_FEATURES.md) - Task locking & orchestration

**Development:**
- [Development Logs](log_docs/) - Implementation details & history

---

## Commands

### Setup & Orientation
```bash
scud init                          # Initialize SCUD in current directory
scud warmup                        # Quick session orientation (recent commits, active tasks)
scud tags                          # List/set active phase tags
```

### Core Commands (Instant)
```bash
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

### AI Commands (Requires API Key)
```bash
scud generate <file> --tag <tag>   # Full pipeline: parse → expand → validate
scud parse <file> --tag <tag>      # Parse PRD/doc into initial tasks
scud expand [--task <id>]          # Break down complex tasks into subtasks
scud analyze-complexity            # Analyze task complexity (AI-powered)
scud check-deps --tag <tag>        # Validate task dependencies
scud reanalyze-deps                # Suggest cross-tag dependencies
```

Default provider: `xai`, model: `xai/grok-code-fast-1`. Configure with `scud config`.

Project guidance files in `.scud/guidance/*.md` are automatically included in AI prompts.

### Swarm Commands (Parallel Execution)
```bash
scud swarm --tag <tag>             # Run wave-based parallel execution
scud swarm --swarm-mode beads      # Continuous polling mode
scud swarm --swarm-mode tmux       # Tmux-based spawning (default)
scud swarm --round-size 5          # Max tasks per round (default: 5)
scud swarm --harness opencode      # AI harness: claude, opencode
scud swarm --dry-run               # Show plan without executing
scud swarm --review                # Enable code review after each wave

# Swarm monitoring and retrospectives
scud retro [session-id]            # View session retrospective
scud watch --session <session>     # Watch running swarm via events

### Agent Spawning Commands
```bash
scud spawn --tag <tag>             # Spawn parallel agents in tmux
scud monitor --session <session>   # Monitor running spawn session
scud sessions                      # List active spawn sessions
scud attach <task-id>              # Attach to running agent
scud restart <task-id>             # Restart failed task
scud run <prompt>                  # Run single AI agent with prompt
```

### Salvo Commands (Worktree Isolation)
```bash
scud salvo list                    # List all salvo worktrees
scud salvo sync <tag>              # Sync worktree status back to main
scud salvo remove <tag>            # Remove worktree and git branch
```

### Transcript Commands
```bash
scud transcript search <query>     # Search transcript content in database
scud transcript stats              # Show aggregate statistics
scud transcript list               # List available transcripts
scud transcript view [--session]   # View transcript (latest if no session)
scud transcript import             # Import all transcripts for project
```

### Orchestrator Commands
```bash
scud assign <id> <name>            # Assign task to a developer
scud who-is [--tag <tag>]          # See who's working on what
scud next-batch [--limit 5]        # Get multiple ready tasks
scud doctor workflow [--tag <tag>] # Diagnose stuck task states
scud doctor scan-ext               # Scan and validate extensions
```

### Utilities
```bash
scud log <id> "message"            # Add log entry to task
scud log-show <id>                 # Show task log entries
scud commit [-m "msg"]             # Git commit with task context
```

### Task Cleanup
```bash
scud clean [--tag <tag>]           # Archive tasks (default, recoverable)
scud clean --list                  # List archived phases
scud clean --restore <name>        # Restore an archived phase
scud clean --keep tag1,tag2        # Archive all except specified tags
scud clean --delete                # Permanently delete (use with caution)
```

> **Note:** `scud clean` now archives by default instead of deleting. Use `--delete` for permanent removal.

---

## Example Workflow

```bash
# 1. Initialize
scud init

# 2. Generate tasks from PRD (parse → expand → validate in one command)
scud generate docs/feature.md --tag auth-system

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

### Combined CLI + GUI Workflow
```bash
# Use CLI for project setup and task generation
scud init
scud generate docs/feature.md --tag my-project
scud waves --tag my-project

# Use GUI for visual task management
descartes-gui
# In GUI: View task waves, start agents, monitor progress

# Use CLI for detailed development work
scud next --tag my-project
scud log 1 "Implemented JWT authentication with refresh tokens"
scud commit -m "Add JWT authentication system"
```

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
- Parallel agent spawning via tmux or headless modes
- Salvo worktrees for isolated parallel execution
- SQLite event storage with transcripts and session history
- Swarm execution with wave-based or continuous modes
- Live monitoring with retrospective analysis
- Task assignment and progress tracking

---

## Requirements

### SCUD CLI
- **Rust toolchain** (for building from source)
- **xAI API key** (for AI features only; core commands work offline)

### Descartes GUI
- **Rust toolchain** (for building from source)
- **System libraries** (Linux: `libxkbcommon-dev libwayland-dev`)

### API Keys
```bash
# For SCUD CLI AI features
export XAI_API_KEY=xai-...

# For Descartes GUI (Claude models)
export ANTHROPIC_API_KEY=sk-ant-...
```

**Alternative providers:** Anthropic (`ANTHROPIC_API_KEY`), OpenAI (`OPENAI_API_KEY`), OpenRouter (`OPENROUTER_API_KEY`). Configure with `scud config`.

---

## File Structure

```
.scud/
├── scud.db                   # SQLite database (events, transcripts, sessions)
├── tasks/tasks.scg           # All tasks in SCG format
├── config.toml               # Provider/model settings
├── active-tag                # Currently active tag
├── current-task              # Active task ID (for commits)
├── guidance/                 # Project guidance for AI prompts
│   └── *.md                  # Markdown files auto-loaded
├── archive/                  # Archived phases (from scud clean)
│   └── <tag>_<timestamp>.scg # Timestamped archive files
├── swarm/                    # Swarm execution data
│   ├── sessions/             # Session state and history
│   └── *.lock                # Session lock files
├── agents/                   # Extension manifests (experimental)
│   └── *.toml                # Custom agent definitions
└── logs/                     # Task log entries

# Salvo worktrees (created alongside project directory)
../<project>.salvo.<tag>/     # Isolated worktree per tag
├── .scud/
│   ├── tasks/tasks.scg       # Filtered: full detail for tag, stubs for others
│   ├── config.toml           # Copied from main
│   ├── active-tag            # Set to target tag
│   └── guidance/             # Copied from main
└── ...                       # Full project checkout on salvo/<tag> branch
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

### Building Both Components

```bash
# Clone repository
git clone https://github.com/pyrex41/scud.git
cd scud

# Build SCUD CLI
cd scud-cli
cargo build --release
./target/release/scud install  # Optional: install globally

# Build Descartes GUI
cd ../descartes-gui
cargo build --release

# Test both
./target/release/scud init
./target/release/descartes-gui
```

### System Dependencies

**macOS:** No additional dependencies required.

**Linux:**
```bash
# For Descartes GUI
sudo apt install libxkbcommon-dev libwayland-dev
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

- **SCUD CLI Quick Reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
- **SCG Format:** [docs/reference/SCG_FORMAT_SPEC.md](docs/reference/SCG_FORMAT_SPEC.md)
- **Orchestrator Pattern:** [docs/orchestrator.md](docs/orchestrator.md)
- **Parallel Features:** [docs/features/PARALLEL_FEATURES.md](docs/features/PARALLEL_FEATURES.md)
- **Descartes GUI:** [descartes-gui/README.md](descartes-gui/README.md)
- **Changelog:** [CHANGELOG.md](CHANGELOG.md)
- **Related:** [github.com/pyrex41/descartes](https://github.com/pyrex41/descartes)

**Happy building!**
