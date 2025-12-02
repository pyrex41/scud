# SCUD

**Sprint Cycle Unified Development** - Fast, AI-powered task management for building software

> **Version 1.17.0** - Stable release. Fast Rust CLI with SCG format, DAG-driven execution, and Claude Code hook integration.

A lightweight DAG-driven task management system with automatic completion enforcement via Claude Code hooks.

---

## Quick Start

### Install

**Using pnpm (recommended):**
```bash
pnpm add -g scud-task
cd your-project
scud init
scud hooks install  # Enable automatic task completion
```

**Using npm:**
```bash
npm install -g scud-task
cd your-project
scud init
scud hooks install
```

**Using Bun:**
```bash
# Bun blocks postinstall scripts by default, so use npm or pnpm
# Or manually run the postinstall after bun install:
bun install -g scud-task
cd ~/.bun/install/global/node_modules/scud-task
node bin/postinstall.js
```

### Basic Usage
```bash
# Create tasks manually or parse from a PRD
scud parse-prd docs/feature.md --tag my-feature

# Find and work on next ready task
scud next --tag my-feature
scud set-status 1 in-progress

# When done, mark complete (or let hooks do it automatically)
scud set-status 1 done
```

**Full guide:** [docs/guides/COMPLETE_GUIDE.md](docs/guides/COMPLETE_GUIDE.md)
**Orchestrator pattern:** [docs/orchestrator.md](docs/orchestrator.md)

---

## Usage Modes

SCUD offers two ways to work with AI assistants, each with different trade-offs:

### Mode 1: Direct CLI (Recommended)

**How it works:**
- Use SCUD CLI directly via bash for manual task management
- Hooks enforce automatic task completion when Claude Code sessions end
- DAG execution ensures tasks become ready when dependencies complete

**Setup:**
```bash
pnpm add -g scud-task   # or: npm install -g scud-task
cd your-project
scud init
scud hooks install      # Critical: enables automatic completion
```

**Pros:**
- ✅ Full file system access (can edit tasks.scg directly if needed)
- ✅ Automatic task completion via hooks (prevents forgotten updates)
- ✅ More flexible - can use any tool/command
- ✅ Better error messages (sees full CLI output)
- ✅ Can combine SCUD with other tools seamlessly
- ✅ DAG-driven execution (tasks ready when deps complete)

**Cons:**
- ❌ Requires bash/shell access
- ❌ Must learn CLI commands

**Best for:** Individual developers, orchestrator patterns, automated workflows

---

### Mode 2: MCP Server (Universal Protocol)

**How it works:**
- Lightweight TypeScript server wraps SCUD CLI
- Exposes 20 MCP tools + 3 resources via standardized protocol
- Works with any MCP-compatible client (Claude Desktop, Cursor, Claude Code, etc.)

**Setup:**
```bash
pnpm add -g scud-task scud-mcp   # or: npm install -g scud-task scud-mcp

# Configure your MCP client (example for Claude Desktop):
# ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "scud": {
      "command": "scud-mcp",
      "env": {
        "ANTHROPIC_API_KEY": "sk-ant-..."
      }
    }
  }
}

# For Cursor/other clients, see scud-mcp/README.md for config
# Then use naturally: "Initialize SCUD and parse my PRD"
```

**Pros:**
- ✅ Structured protocol (well-defined tool schemas)
- ✅ Works across multiple AI clients (Claude Desktop, Cursor, etc.)
- ✅ Cleaner interface (named tools vs bash commands)
- ✅ Type-safe tool calls with validation
- ✅ Can add file access via MCP resources (extensible)

**Cons:**
- ❌ Requires MCP server installation (extra dependency)
- ❌ Less ad-hoc than direct bash (predefined tools only)
- ❌ Client must support MCP protocol

**Best for:** Multi-client usage, structured workflows, type-safe operations

---

### Which Mode Should You Use?

**Use Direct CLI if:**
- You want maximum flexibility (any bash command)
- You need DAG-driven execution with hooks
- You're building orchestrator patterns (parallel execution)
- You want zero external dependencies (just the CLI)
- You want automatic task completion enforcement

**Use MCP Server if:**
- You use multiple AI clients (Claude Desktop, Cursor, etc.)
- You want structured, type-safe tool calls
- You prefer protocol-based integration
- You want extensible architecture (can add custom tools/resources)

**Can you use both?** Yes! The MCP server wraps the CLI, so they're fully compatible. Both modes work in Claude Code, Cursor, and Claude Desktop.

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

### Automatic Completion
Claude Code hooks enforce task completion when sessions end. Set `SCUD_TASK_ID` env var and the hook marks it done automatically.

### Parallel Execution
Use orchestrator patterns to spawn multiple Claude Code agents in parallel, each working on a ready task.

---

## Key Features

### Fast Rust CLI
- ⚡ **50x faster** than JavaScript alternatives
- 🎯 **42x fewer tokens** (500 vs 21k)
- 📦 **Single binary** - no dependencies

### Hook-Enforced Completion
- 🔒 **Automatic task completion** via Claude Code hooks
- ✅ **Prevents forgotten updates** (solves 15% failure case)
- 🎯 **Session-based** - marks done when Claude session ends

### DAG-Driven Execution
- 📊 **Dependency graphs** - tasks ready when deps complete
- 🔀 **Parallel waves** - visualize concurrent work
- 🎯 **Smart scheduling** - next command finds ready tasks

### Orchestrator Support
- 👥 **Parallel agents** - spawn multiple Claude instances
- 🔒 **Task locking** - prevent conflicts
- 📊 **Session monitoring** - track active work

---

## Documentation

**Getting Started:**
- [Complete Guide](docs/guides/COMPLETE_GUIDE.md) - Comprehensive reference (25,000 words)
- [Orchestrator Pattern](docs/orchestrator.md) - Parallel execution guide
- [Migration Guide](docs/guides/MIGRATION.md) - Upgrading from BMAD-TM Lite

**Integration:**
- [MCP Server Guide](scud-mcp/README.md) - Model Context Protocol integration
- [Quick Reference](docs/reference/QUICK_REFERENCE.md) - Command cheat sheet

**Advanced:**
- [Parallel Features](docs/features/PARALLEL_FEATURES.md) - Task locking & orchestration

**Development:**
- [Development Logs](log_docs/) - Implementation details & history

---

## Commands

### Setup
```bash
scud init                          # Initialize SCUD
scud hooks install                 # Enable automatic completion
scud hooks status                  # Check hook status
```

### Core Commands (Instant)
```bash
scud tags                          # List all tags
scud list [--tag <tag>]           # List tasks
scud next [--tag <tag>]           # Find next ready task
scud set-status <id> <status>      # Update task
scud stats [--tag <tag>]          # Show statistics
scud waves [--tag <tag>]          # Show parallel waves
```

### AI Commands (Requires XAI_API_KEY)
```bash
scud parse-prd <file> --tag <tag>  # Parse PRD into tasks
scud analyze-complexity             # Analyze all tasks
scud expand --all                   # Break down complex tasks
```

Default: `grok-code-fast-1` via xAI. Configure with `scud config --provider <provider> --model <model>`.

### Orchestrator Commands
```bash
scud claim <id> --name <name>      # Claim task (lock)
scud release <id>                  # Release task lock
scud whois [--tag <tag>]          # See who's working on what
scud doctor [--tag <tag>]         # Check for stale locks
```

---

## Example Workflow

```bash
# 1. Initialize
scud init
scud hooks install

# 2. Create tasks from PRD
scud parse-prd docs/feature.md --tag auth-system
# Creates tasks with dependencies

# 3. View execution plan
scud waves --tag auth-system
# Shows which tasks can run in parallel

# 4. Work on next ready task
scud next --tag auth-system
# Returns: Task 1 is ready

# 5. Implement (manual or via orchestrator)
# Manual: Work on task, then mark done
scud set-status 1 done

# Or with orchestrator: Start Claude session with task ID
SCUD_TASK_ID=1 claude "Implement task 1"
# Hook auto-marks complete when session ends

# 6. Repeat until done
scud stats --tag auth-system
# Shows progress: 8/10 complete
```

See [docs/orchestrator.md](docs/orchestrator.md) for parallel execution patterns.

---

## Why SCUD?

**DAG-Driven:**
- Tasks become ready when dependencies complete
- No manual phase management
- Visualize parallel execution waves
- Smart scheduling finds ready work

**Hook-Enforced:**
- Automatic task completion
- Prevents forgotten status updates
- Session-based tracking
- Solves 15% agent failure case

**Fast & Simple:**
- Rust CLI is instant
- SCG format is human-readable and git-friendly
- Works offline (core commands)
- No vendor lock-in

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
├── tasks/tasks.scg           # All tasks in SCG format (see spec)
├── config.toml               # Active tag and settings
└── current-task              # Active task ID (for hooks)

.claude/
└── settings.local.json       # Claude Code hooks config

docs/
├── prd/                      # Product requirements
└── epics/                    # Feature descriptions
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

- **Complete Guide:** [docs/guides/COMPLETE_GUIDE.md](docs/guides/COMPLETE_GUIDE.md)
- **Orchestrator Pattern:** [docs/orchestrator.md](docs/orchestrator.md)
- **Quick Reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
- **Parallel Features:** [docs/features/PARALLEL_FEATURES.md](docs/features/PARALLEL_FEATURES.md)
- **Implementation Logs:** [log_docs/](log_docs/)

**Happy building!**
