---
date: 2026-03-12T13:00:00-07:00
researcher: claude
git_commit: b0adb02f98c25671f0f8c0625f27333572df8ef1
branch: scud-go
repository: scud
topic: "Compare rho-cli vs axe as scud's agent execution backend"
tags: [research, rho-cli, axe, scud, agent-backend, architecture]
status: complete
last_updated: 2026-03-12
last_updated_by: claude
---

# Research: rho-cli vs axe as scud's Agent Execution Backend

**Date**: 2026-03-12
**Git Commit**: b0adb02
**Branch**: scud-go

## Research Question
Compare and contrast rho-cli and axe for the specific purpose of serving as scud's agent execution backend.

## Summary

scud uses rho-cli as a subprocess agent runner through a thin wrapper (`internal/rho/rho.go`). The interface is narrow: 6 flags (`--model`, `--output-format`, `-C`, `--system-append`, `--tools`, plus a positional prompt) across ~10 call sites. axe covers most of this surface but has critical gaps — no `--system-append` flag and no per-invocation `--tools` flag — that would break scud's heavy ensemble. axe brings capabilities scud doesn't currently use (MCP, memory, sub-agents, JSON envelope, structured exit codes) that could add value. A swap is feasible but not drop-in.

## What scud Actually Needs from Its Backend

Based on every rho-cli call site in the codebase:

### Flags Used in Production

| Flag | Used By | Required? |
|---|---|---|
| `--model <string>` | All call sites | Yes — every call specifies a model |
| `<prompt>` (positional) | All call sites | Yes — always the last arg |
| `-C <workdir>` | Swarm, heavy (with workdir), attractor, cmd/run | Yes — sandboxes agent to project dir |
| `--system-append <string>` | Heavy ensemble only (all 5 phases) | Yes for heavy — injects per-agent system prompts |
| `--tools <comma-list>` | Heavy ensemble agent execution only | Yes for heavy — restricts tools per specialist agent |
| `--output-format <string>` | Defined but **never set** by any caller | No |

### Behavioral Requirements

| Behavior | How scud depends on it |
|---|---|
| stdout = agent's text output | Swarm discards it; heavy reads it as agent output; RunJSON parses JSON from it |
| stderr = separate stream | Only surfaced in error messages |
| Non-zero exit ≠ Go error | `rho.Run()` returns `nil` error with `ExitCode` in result; callers branch on exit code |
| JSON extractable from stdout | `RunJSON` uses regex to find `[...]` or `{...}` in plain text output |
| Tool use happens internally | Agent uses tools (Bash, Read, etc.) inside the conversation loop; scud never sees tool calls |
| Agent can call `scud set-status` | During swarm execution, the agent runs shell commands that modify scud's own state |

## Head-to-Head Comparison

### Flag Mapping

| scud needs | rho-cli | axe | Gap? |
|---|---|---|---|
| `--model <provider/model>` | `--model <string>` | `--model <provider/model>` | **No** — same semantics, axe enforces `provider/model` format |
| Positional prompt | Last positional arg | Stdin pipe | **Minor** — axe takes user message via stdin, not positional arg. Scud would pipe prompt instead of passing as arg |
| `-C <workdir>` | `-C <dir>` (sets both flag and `cmd.Dir`) | `--workdir <dir>` | **No** — equivalent, different flag name |
| `--system-append <string>` | `--system-append <string>` | ❌ **No flag** | **BLOCKER for heavy** — axe has no system prompt override flag. System prompt is TOML-only |
| `--tools <list>` | `--tools <comma-list>` | ❌ **No flag** | **BLOCKER for heavy** — axe tools are TOML-only, not per-invocation |
| `--output-format` | `--output-format <string>` | ❌ Not needed | **No gap** — scud never uses this flag |

### Capability Comparison

| Capability | rho-cli | axe | Scud benefit |
|---|---|---|---|
| **Multi-provider** | Yes (model string routing) | Yes (anthropic, openai, ollama, opencode) | Equivalent |
| **Tool use loop** | Internal (transparent to scud) | Internal (max 50 turns) | Equivalent |
| **Workdir sandboxing** | Via `-C` flag | Via `--workdir` flag + strict path validation | axe is stricter (rejects `..` traversal) |
| **MCP support** | Unknown | Yes (SSE + Streamable-HTTP) | **New capability** — agents could use external MCP tools |
| **Persistent memory** | Unknown | Yes (per-agent markdown logs) | **New capability** — agents could learn across invocations |
| **Sub-agent delegation** | Unknown | Yes (`call_agent`, depth-limited) | **New capability** — task agents could delegate subtasks |
| **JSON output envelope** | Not used by scud | `--json` flag with token counts, duration, tool details | **New capability** — scud could get token usage metrics |
| **Structured exit codes** | Opaque integer | 0=success, 1=agent failure, 2=config error, 3=provider error | **Improvement** — scud could distinguish failure types |
| **Refusal detection** | Unknown | Built-in heuristic in JSON output | **New capability** — detect when LLM refuses a task |
| **Dry-run** | Unknown | `--dry-run` shows resolved context | **New capability** — debug prompt construction |
| **Streaming** | Unknown | No | Neither streams to scud |

### Where axe Falls Short

#### 1. No `--system-append` flag (BLOCKER)

scud's heavy ensemble passes per-agent system prompts via `--system-append` at 5 call sites (`ensemble.go:253,310,364,398,440`). Each of the 16 agents has a distinct `SystemPrompt` string injected this way. axe's system prompt comes exclusively from the TOML file — there is no CLI flag to override or append to it.

**Workaround options:**
- Create 16 separate TOML agent files (one per heavy agent) with hardcoded system prompts → loses dynamic prompt composition
- Inject system prompt via the skill override: `--skill <tempfile>` where scud writes the system prompt to a temp file → hacky but functional since skill content is injected into the system prompt
- Prepend the system prompt to stdin (the user message) → semantically wrong; system vs user message distinction matters for LLM behavior
- Patch axe to add `--system-prompt` or `--system-append` flag → best long-term solution

#### 2. No per-invocation `--tools` flag (BLOCKER)

scud's heavy ensemble restricts tools per agent: Harper gets `Bash,Read,Grep,Glob`, Benjamin gets `Bash,Read`, Captain gets none. This is passed via `--tools` at runtime (`ensemble.go:315`). axe's `tools` array is TOML-only.

**Workaround options:**
- Create per-agent TOML files with the correct tools baked in → viable but rigid
- Set `XDG_CONFIG_HOME` to a temp directory with dynamically-generated TOMLs → viable but complex
- Patch axe to add `--tools` flag → best long-term solution

#### 3. No positional prompt argument

rho-cli takes the prompt as the last positional arg. axe takes user input via stdin. This is a minor integration change — scud would pipe the prompt to axe's stdin instead of appending it to the arg list.

**Impact:** Trivial. Change `cmd.Args` to `cmd.Stdin` in `rho.go`.

#### 4. Agent TOML must live in XDG config dir

axe loads agents from `$XDG_CONFIG_HOME/axe/agents/<name>.toml` only. There's no `--agent-file` flag. scud doesn't currently use agent definition files (the prompt IS the agent), but if axe required a TOML for every invocation, scud would need to manage TOML files.

**Workaround:** Use a generic "passthrough" TOML agent with minimal config, override everything via flags. Or set `XDG_CONFIG_HOME` per-invocation.

### Where axe Adds Value

#### 1. MCP Tool Extensibility

axe agents can connect to external MCP servers for additional tools. A swarm task agent could use project-specific MCP tools (database queries, API calls, deployment triggers) without scud needing to know about them. This is configured per-agent in TOML.

#### 2. Persistent Memory

axe's per-agent memory system stores past inputs/outputs as timestamped markdown. A `builder` agent that runs across multiple tasks could accumulate project knowledge — coding conventions, past mistakes, architectural patterns. The LLM-powered GC (`axe gc`) prevents unbounded growth.

#### 3. Sub-Agent Delegation

axe's `call_agent` tool lets an agent invoke another agent mid-task. A `builder` agent working on a complex task could delegate research to a `researcher` agent or code review to a `reviewer` agent, all within a single scud task execution. This is depth-limited (max 5) to prevent runaway costs.

#### 4. Structured Error Information

axe's `--json` envelope provides token counts, duration, tool call details, and refusal detection. Scud could use this for:
- Cost tracking per task/wave/phase
- Detecting stuck agents (high token count, no useful output)
- Identifying refused tasks for human review
- Performance profiling of the swarm

#### 5. Stricter Sandboxing

axe rejects absolute paths and `..` traversal in file tools. rho-cli's sandboxing behavior is unknown from scud's perspective. axe provides a stronger guarantee that agents stay within their workdir.

## Integration Effort Estimate

### Drop-in Replacement: Not Possible

The two blockers (`--system-append` and `--tools`) prevent axe from replacing rho-cli without either patching axe or restructuring scud's heavy ensemble.

### Partial Replacement: Feasible

axe could replace rho-cli for **swarm execution** and **generate pipeline** call sites, which don't use `--system-append` or `--tools`. This covers:
- `swarm/executor.go` — normal task execution and repair (2 call sites)
- `generate/parse_prd.go` — PRD parsing (1 call site)
- `generate/expand.go` — task expansion (1 call site)
- `generate/reanalyze_deps.go` — dep reanalysis (1 call site)
- `attractor/handlers.go` — codergen nodes (1 call site)
- `cmd/run.go` — manual single-task run (1 call site)

That's 7 of 10 call sites. The 5 heavy ensemble call sites (`ensemble.go`) would still need rho-cli.

### Changes Required in scud's `internal/rho/rho.go`

For an axe backend (swarm/generate/attractor only):

```
1. Binary name: "rho-cli" → "axe"
2. Arg structure: positional prompt → stdin pipe
3. Flag names: -C → --workdir
4. Add: agent name as positional arg (e.g., "axe run scud-task")
5. Manage: a generic TOML agent file, or set XDG_CONFIG_HOME
6. Optional: parse --json output for token metrics
```

### Changes Required in axe (for full replacement)

```
1. Add --system-prompt or --system-append flag
2. Add --tools flag for per-invocation tool restriction
3. (Optional) Add --agent-file flag for arbitrary TOML paths
```

These are straightforward additions to axe's cobra command setup and would not require architectural changes.

## Architecture Decision Factors

| Factor | Favor rho-cli | Favor axe |
|---|---|---|
| Works today, no changes | ✅ | ❌ |
| Active open-source project | Unknown | ✅ (v1.2.0, active development) |
| MCP ecosystem access | ❌ | ✅ |
| Agent memory across tasks | ❌ | ✅ |
| Sub-agent composition | ❌ | ✅ |
| Token/cost observability | ❌ | ✅ (`--json`) |
| Sandboxing guarantees | Unknown | ✅ (strict path validation) |
| Heavy ensemble support | ✅ | ❌ (needs patches) |
| Community/ecosystem | Unknown | ✅ (Show HN, growing) |
| Dependency weight | Unknown | Minimal (2 deps, 12MB binary) |

## Conclusion

axe is a credible candidate to replace rho-cli as scud's execution backend, but not as a drop-in swap. The path forward would be:

1. **Short term:** Use axe for swarm/generate/attractor (7 of 10 call sites) while keeping rho-cli for heavy ensemble
2. **Medium term:** Contribute `--system-prompt` and `--tools` flags upstream to axe (or fork)
3. **Long term:** Full replacement, gaining MCP, memory, sub-agents, and structured observability

The alternative is to treat rho-cli as the "dumb pipe" and axe as the "smart agent" — using rho-cli where scud needs fine-grained control (heavy ensemble) and axe where scud wants rich agent capabilities (swarm task execution with memory and MCP).
