---
date: 2026-03-12T12:00:00-07:00
researcher: claude
git_commit: b0adb02f98c25671f0f8c0625f27333572df8ef1
branch: scud-go
repository: scud
topic: "Compare and contrast axe (jrswab/axe) with scud"
tags: [research, comparison, axe, scud, agent-orchestration]
status: complete
last_updated: 2026-03-12
last_updated_by: claude
---

# Research: Compare and Contrast axe vs scud

**Date**: 2026-03-12
**Git Commit**: b0adb02
**Branch**: scud-go
**Repository**: scud

## Research Question
Compare and contrast [jrswab/axe](https://github.com/jrswab/axe) with scud-go.

## Summary

axe and scud occupy different layers of the AI-assisted development stack. **axe** is a stateless agent *runner* — a thin, composable binary that executes a single LLM agent per invocation, defined declaratively in TOML. **scud** is a DAG-based task *manager and orchestrator* — it parses PRDs into dependency graphs, plans parallel execution waves, dispatches agents across waves with backpressure validation, and repairs failures. They are complementary rather than competitive: axe is the "run one agent" primitive; scud is the "plan, coordinate, and execute many agents across a project" system.

## Detailed Comparison

### Philosophy

| Dimension | axe | scud |
|---|---|---|
| **Core metaphor** | Unix utility — pipe in, get text out | Project manager — plan work, dispatch agents, validate results |
| **Unit of work** | Single agent invocation | DAG of tasks across phases |
| **State** | Stateless (optional memory logs) | Stateful (`.scud/tasks/tasks.scg`, SQLite sessions) |
| **Orchestration** | External (shell, cron, pipes) | Built-in (wave planner, swarm executor, repair loops) |
| **Scope** | Run one agent well | Manage an entire project's AI-driven development |

### Architecture

| Aspect | axe | scud |
|---|---|---|
| **Language** | Go | Go |
| **Binary size** | ~12MB | ~15-20MB |
| **Dependencies** | 2 (cobra, toml) | Several (cobra, toml, sqlite, errgroup, uuid) |
| **Agent definition** | TOML files in `$XDG_CONFIG_HOME/axe/agents/` | Task fields in `.scud/tasks/tasks.scg` (SCG format) |
| **Config** | `$XDG_CONFIG_HOME/axe/config.toml` | `.scud/config.toml` per project |
| **Storage** | Filesystem (TOML + markdown memory) | Custom SCG text format + SQLite |

### Task / Agent Model

**axe**: An "agent" is a TOML file specifying model, system prompt, tools, file context, optional memory, and optional sub-agents. There is no concept of tasks, statuses, dependencies, or queues. Each `axe run <agent>` is a fresh, isolated invocation. Sub-agents can be invoked dynamically via the `call_agent` tool (depth-limited to 5).

**scud**: A "task" is a node in a DAG with ID, title, description, status (`pending`/`in-progress`/`done`/`failed`/etc.), complexity (Fibonacci), priority, dependencies (other task IDs), agent type (`builder`/`fast-builder`/`reviewer`/etc.), and model tier (`fast`/`standard`/`smart`). Tasks are grouped into phases. The DAG determines execution order.

### Dependency Management

**axe**: None. Sequencing is done externally via shell pipes (`axe run step1 | axe run step2`) or scripts. The only internal composition mechanism is sub-agent delegation (LLM decides at runtime whether to call another agent).

**scud**: First-class DAG with `Dependencies []string` per task. Cross-phase dependencies supported (`"phase:id"` format). Cycle detection via DFS (`WouldCreateCycle`). Subtasks inherit parent dependencies. Kahn's algorithm computes parallel execution waves.

### Execution Model

**axe**:
1. Load TOML agent config
2. Resolve workdir, files, skill, stdin
3. Build system prompt (with memory if enabled)
4. Conversation loop (max 50 turns): LLM call → tool execution → repeat until no tool calls
5. Output text (or JSON envelope with `--json`)
6. Append memory entry if enabled

**scud** (swarm mode):
1. Load all phases from `.scud/tasks/tasks.scg`
2. Compute waves via Kahn's topological sort
3. Take wave 0 (all tasks with satisfied deps)
4. Execute tasks in parallel chunks (default 5) via errgroup
5. Each task: mark in-progress → build prompt → call `rho.Run()` → check status
6. Run backpressure validation (build + test commands)
7. On failure: attribute via git blame → reset attributed tasks → repair with smart model
8. Repeat for next wave

### AI/LLM Integration

| Feature | axe | scud |
|---|---|---|
| **Providers** | Anthropic, OpenAI, Ollama, OpenCode | xAI, OpenAI, OpenRouter, Anthropic, rho-cli fallback |
| **Model format** | `provider/model-name` | Config-driven tiers (`fast`/`standard`/`smart`) |
| **Tool use** | Built-in tools (read/write/edit/run/list/url_fetch/web_search) + MCP | Delegated to rho-cli subprocess (which has its own tool use) |
| **Sub-agents** | `call_agent` tool, depth-limited | 16-agent Heavy ensemble, xAI native multi-agent |
| **MCP support** | Yes (SSE + Streamable-HTTP transports) | No |
| **Memory** | Per-agent markdown logs with LLM-powered GC | No per-agent memory (session transcripts in SQLite) |

### AI-Powered Task Generation

**axe**: Does not generate tasks. Agents are hand-authored in TOML.

**scud**: Full AI pipeline:
1. **Parse PRD** — LLM reads a PRD document, produces a JSON array of tasks with complexity, priority, dependencies, and agent types
2. **Expand** — Complex tasks (Fibonacci ≥5) are expanded into subtasks by the LLM
3. **Reanalyze deps** — Smart model reviews and corrects the dependency graph
4. **Check deps** — Structural validation (cycles, missing refs, duplicates)

### CLI Surface

**axe** (~8 commands):
- `axe run <agent>` — execute an agent (with `--model`, `--dry-run`, `--json`, `-v` flags)
- `axe agents list|show|init|edit` — manage agent definitions
- `axe config init|path` — manage global config
- `axe gc <agent>` — LLM-assisted memory garbage collection
- `axe version`

**scud** (~25+ commands):
- Task CRUD: `create`, `show`, `list`, `set-status`, `assign`, `whois`
- DAG: `waves`, `next`, `next-batch`, `check-deps`, `reanalyze-deps`
- AI generation: `generate`, `parse`, `expand`, `analyze-complexity`
- Execution: `swarm`, `heavy`, `multiagent`, `attractor`
- Project: `init`, `tags`, `stats`, `warmup`, `doctor`, `clean`, `commit`, `log`
- Export: `mermaid`, `convert`

### Composability

**axe**: Designed for Unix composition. Stdin piping is a first-class feature. Agents are small, focused, and chainable. `--json` output enables programmatic consumption. Agents work in any project without project-specific setup.

**scud**: Self-contained project tool. Tasks, phases, config, and guidance live in `.scud/`. The swarm executor handles composition internally — you don't pipe scud commands together; you define a DAG and let scud orchestrate it.

### When You'd Use Each

| Scenario | Tool |
|---|---|
| Quick one-off AI task (review a diff, summarize a file) | axe |
| Chain 2-3 agents in a shell script | axe |
| Cron-driven recurring AI task | axe |
| Git hook agent (pre-commit review) | axe |
| Plan and execute a multi-task feature from a PRD | scud |
| Coordinate parallel AI work with dependency ordering | scud |
| Track task status across sessions | scud |
| Validate AI output with build/test gates | scud |
| Auto-repair failed AI work | scud |
| 16-agent ensemble reasoning | scud (heavy) |

### Overlap

The overlap is narrow:
- Both are Go CLI tools for running LLM agents
- Both support multiple providers (Anthropic, OpenAI, xAI/Ollama)
- Both use TOML for configuration
- Both can execute tool-using agents

But the intent and abstraction level differ fundamentally. axe is a building block; scud is a system built from such blocks (and in fact uses `rho-cli` — a similar agent-runner primitive — as its execution backend).

### Potential Complementarity

axe could theoretically serve as scud's execution backend instead of (or alongside) `rho-cli`. An axe agent TOML could map naturally to a scud task's agent definition. The composability model would be: scud manages the DAG and orchestration, axe runs individual task agents. This would give scud access to axe's MCP support, persistent memory, and sub-agent delegation within each task execution.

## Key Differentiators

1. **axe has no task management** — this is its biggest departure from scud. axe deliberately avoids project state.
2. **scud has no standalone agent runner** — it always operates in the context of a project's task DAG.
3. **axe's MCP support** gives it extensibility that scud lacks at the agent level.
4. **scud's backpressure validation** (build/test gates + git-blame attribution + repair loops) has no analogue in axe.
5. **scud's wave planning** (Kahn's algorithm) provides optimal parallelism; axe's parallelism is ad-hoc (sub-agents or external scripting).
6. **axe's memory system** provides cross-invocation learning; scud tasks are stateless between executions.

## Sources

- [GitHub — jrswab/axe](https://github.com/jrswab/axe)
- [Show HN: Axe — A 12MB binary that replaces your AI framework](https://news.ycombinator.com/item?id=47350516)
- scud-go source at `/Users/reuben/projects/scud-go/`
