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
| `warmup` | Session orientation (status + next task) |
| `doctor` | Diagnose project issues |
| `mermaid` | Export dependency graph as Mermaid diagram |

## Task model

Tasks have hierarchical IDs (`1`, `1.1`, `1.1.1`), statuses (`pending`, `in-progress`, `done`, `blocked`, `failed`, `review`, `deferred`, `cancelled`), Fibonacci complexity scores, and priority levels.

Dependencies form a DAG — a task is "ready" when all its dependencies are `done`. SCUD's wave planner groups ready tasks into parallel execution batches.

## Agent integration

SCUD is designed to work with AI coding agents. Use `scud warmup` at the start of a session, `scud next` to pick up work, and `scud commit` for task-aware git commits.

For multi-agent execution, `scud swarm` runs tasks in parallel waves with configurable agent types and model tiers.

## License

MIT
