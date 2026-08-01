# SCUD

DAG-based task management with AI generation and swarm execution.

SCUD models your project as a directed acyclic graph of tasks with dependencies, priorities, and complexity scores. It integrates with AI coding agents (Claude Code, etc.) to generate, expand, and execute tasks in parallel waves.

## Install

### Binary (recommended)

```sh
curl -sSf https://raw.githubusercontent.com/pyrex41/scud/master/install.sh | sh
```

This detects your OS/architecture and downloads the latest release binary to `~/.local/bin`.

### From source

```sh
go install github.com/reuben/scud/cmd/scud@latest
```

### Manual download

Prebuilt binaries for Linux, macOS, and Windows (amd64/arm64) are available on the [Releases](https://github.com/pyrex41/scud/releases) page.

## Quick start

```sh
# Initialize in your project
scud init

# Add tasks from a requirements doc
scud parse prd.md

# See what's ready to work on
scud next

# Start working
scud set-status <id> in-progress

# Mark done
scud set-status <id> done

# View progress
scud stats
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize SCUD in the current project |
| `list` | Show all tasks with status |
| `show <id>` | View task details |
| `next` | Get the next available task |
| `set-status <id> <status>` | Update task status |
| `stats` | View completion statistics |
| `waves` | Show parallel execution waves |
| `create` | Create a new task |
| `parse` | Generate tasks from a document |
| `expand` | Break a task into subtasks |
| `generate` | Regenerate task files from the graph |
| `check-deps` | Validate dependency graph |
| `tags` | Manage task tags |
| `assign` | Assign tasks to agents |
| `commit` | Task-aware git commit |
| `swarm` | Execute tasks in parallel with AI agents |
| `heavy` | Multi-agent reasoning ensemble (16 agents) |
| `warmup` | Session orientation (status + next task) |
| `doctor` | Diagnose project issues |
| `mermaid` | Export dependency graph as Mermaid diagram |
| `mcp-server` | Start MCP stdio server for tool integration |

## Task model

Tasks have hierarchical IDs (`1`, `1.1`, `1.1.1`), statuses (`pending`, `in-progress`, `done`, `blocked`, `failed`, `review`, `deferred`, `cancelled`), Fibonacci complexity scores, and priority levels.

Dependencies form a DAG — a task is "ready" when all its dependencies are `done`. SCUD's wave planner groups ready tasks into parallel execution batches.

## scud as a library

Public Go packages live under `pkg/` for tools that embed scud directly instead of shelling out to the CLI:

```go
import (
    "github.com/reuben/scud/pkg/model"
    "github.com/reuben/scud/pkg/scg"
)
```

The public surface currently includes `pkg/model`, `pkg/scg`, `pkg/wave`, `pkg/attractor`, `pkg/heavy`, `pkg/swarm`, `pkg/llm`, and `pkg/generate`. APIs are still stabilizing while scud is pre-1.0, so breaking library changes may happen in later `v0.x` releases.

Swarm scheduling also accepts a harness-neutral `pkg/executor.Runner`. Existing
callers continue to use the legacy Rho CLI adapter by default; embedders can opt
into the versioned `rho.run/v1` JSONL adapter. See
[`pkg/executor`](pkg/executor/README.md) for the protocol integration boundary.

The opt-in v2 core is a smaller embeddable harness: deterministic reducer and
reconciler, DAG bridge, bounded runtime, and replaceable policy, execution, and
event-store adapters. `v2/adapters/tdhttp` persists its canonical event stream
in td under the ticket's live attempt/execution fence and obtains
Shen-authorized BIP340 execution witnesses. `v2/adapters/rho` executes the same
provider-neutral request through Rho, which supports Anthropic, OpenAI, and
xAI. SCUD owns goal/DAG orchestration, td owns identity, leases, policy and
durability, and Rho owns bounded provider/tool execution.

To try the protocol adapter without changing project configuration:

```sh
scud run 1 --executor rho-v1 --provider anthropic --model claude-sonnet-4-5
scud swarm --executor rho-v1 --provider xai
```

## Heavy ensemble

`scud heavy` runs a multi-agent reasoning ensemble where a Captain agent routes your query to specialist agents, they analyze in parallel, and the Captain synthesizes a unified answer.

```sh
# Basic query
scud heavy "What does the wave planner do?" -v

# Use cheap models for agents, expensive for synthesis
scud heavy "Analyze the auth system" \
  --model-agents grok-4.1-fast \
  --model-synthesis grok-4.20-reasoning

# Hybrid mode: rho agents (file tools) + xAI native (web search) in parallel
scud heavy "Compare our auth approach to industry best practices" --mode hybrid
```

### Modes

| Mode | Description |
|------|-------------|
| `ensemble` | (default) rho agents with local file tools |
| `native` | xAI native multi-agent with web search |
| `hybrid` | both in parallel, captain synthesizes all |

### Per-role model overrides

Use cheap models for bulk work and reserve expensive models for synthesis:

```toml
# .scud/config.toml
[heavy.models]
routing = "grok-4.1-fast"
agents = "grok-4.1-fast"
synthesis = "grok-4.20-0309-reasoning"
debate = "grok-4.1-fast"
native = "grok-4.20-multi-agent-0309"
```

Or via CLI flags (`--model-agents`, `--model-synthesis`) or env vars (`SCUD_HEAVY_MODEL_AGENTS`, etc).

## MCP server

`scud mcp-server` starts a [Model Context Protocol](https://modelcontextprotocol.io) server over stdio, exposing scud commands as tools for AI agents (Claude Code, Cowork, etc).

### Setup for Claude Code / Cowork

Add to `.mcp.json` (project) or `claude_desktop_config.json` (Cowork):

```json
{
  "mcpServers": {
    "scud": {
      "command": "scud",
      "args": ["mcp-server"],
      "env": {
        "SCUD_TOOLS": "full"
      }
    }
  }
}
```

### Tool tiers

Control context token usage with `SCUD_TOOLS`:

| Tier | Tools | Tokens |
|------|-------|--------|
| `core` (default) | warmup, next, show, set-status, stats, commit | ~3K |
| `full` | core + list, heavy, create | ~5K |
| Custom | `SCUD_TOOLS=next,show,heavy` | varies |

## Agent integration

SCUD is designed to work with AI coding agents. Use `scud warmup` at the start of a session, `scud next` to pick up work, and `scud commit` for task-aware git commits.

For multi-agent execution, `scud swarm` runs tasks in parallel waves with configurable agent types and model tiers.

## License

MIT
