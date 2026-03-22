---
name: scud-guide
description: SCUD CLI reference and workflow guide. Use when working with scud task management, running scud commands, or when the user mentions tasks, waves, DAG, or project progress.
---

# SCUD CLI Guide

SCUD is a DAG-based task manager for AI-driven development. Tasks have dependencies, priorities, and complexity scores. Work flows through parallel waves.

## Session Workflow

```bash
scud warmup              # Orient: status, git history, next task
scud next                # Find next available task (deps satisfied)
scud set-status ID in-progress
# ... do the work ...
scud commit -m "message" # Auto-prefixes [TASK-ID]
scud set-status ID done
scud stats               # Check progress
```

## Commands

| Category | Command | Description |
|----------|---------|-------------|
| **Session** | `scud warmup` | Orient with status + next task |
| **View** | `scud list [--status pending]` | List tasks |
| | `scud show ID` | Task details |
| | `scud stats` | Completion statistics |
| **Work** | `scud next` | Next ready task |
| | `scud waves` | Parallel execution waves |
| | `scud set-status ID STATUS` | Update status |
| | `scud create --title "..."` | Create a task |
| | `scud assign ID @dev` | Assign task |
| **Git** | `scud commit -m "msg"` | [TASK-ID] prefixed commit |
| **AI** | `scud parse FILE` | Generate tasks from doc |
| | `scud expand ID` | Break into subtasks |
| | `scud heavy "query"` | Multi-agent reasoning ensemble |
| **Tags** | `scud tags` | List/switch phases |
| **Utils** | `scud mermaid` | Export DAG as Mermaid |
| | `scud doctor` | Diagnose issues |
| **Server** | `scud mcp-server` | Start MCP server for tool integration |

## Heavy Ensemble

Run a multi-agent reasoning ensemble with per-role model control:

```bash
# Default: all agents use same model
scud heavy "What does the auth module do?" -v

# Cost-optimized: cheap agents, smart synthesis
scud heavy "query" --model-agents grok-4.1-fast --model-synthesis grok-4.20-reasoning

# Hybrid: local file analysis + web research in parallel
scud heavy "query" --mode hybrid

# Modes: ensemble (default), native (xAI multi-agent), hybrid (both)
```

## MCP Server

Expose scud as tools for AI agents (Cowork, Claude Code):

```bash
# Start with core tools (6 tools, ~3K context tokens)
scud mcp-server

# Full tools including heavy and list
SCUD_TOOLS=full scud mcp-server

# Custom selection
SCUD_TOOLS=next,show,heavy scud mcp-server
```

Add to `.mcp.json` for Claude Code or `claude_desktop_config.json` for Cowork:
```json
{"mcpServers": {"scud": {"command": "scud", "args": ["mcp-server"]}}}
```

## Configuration

Config lives in `.scud/config.toml`. Key sections:

```toml
[heavy.models]
routing = "grok-4.1-fast"        # Captain routing (cheap)
agents = "grok-4.1-fast"         # Parallel agents (cheap)
synthesis = "grok-4.20-reasoning" # Synthesis (quality)

[swarm.tiers]
fast = "grok-code-fast-1"
standard = "grok-4.20-reasoning"
smart = "grok-4.20-reasoning"
```

Env overrides: `SCUD_HEAVY_MODEL_AGENTS`, `SCUD_HEAVY_MODEL_SYNTHESIS`, `SCUD_HEAVY_MODE`, etc.

## Task Statuses

`pending` | `in-progress` | `done` | `blocked` | `failed` | `review` | `expanded` | `deferred` | `cancelled`

## Key Concepts

- **DAG**: Tasks form a directed acyclic graph. A task is "ready" when all deps are done.
- **Waves**: Groups of tasks with no inter-dependencies, executable in parallel.
- **Tags**: Organize tasks into phases/features. Switch with `scud tags <name>`.
- **Complexity**: Fibonacci scores (1,2,3,5,8,13...) for effort estimation.
