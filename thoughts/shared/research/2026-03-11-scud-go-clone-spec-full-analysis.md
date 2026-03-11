---
date: 2026-03-11T17:51:20Z
researcher: claude
git_commit: b0d45a63a569372265d8560c11e73e416b71c039
branch: scud-go
repository: scud-go
topic: "SCUD Go Clone Spec - Full System Analysis"
tags: [research, codebase, scud, go-rewrite, data-model, scg-format, llm-client, attractor, swarm, wave-planner]
status: complete
last_updated: 2026-03-11
last_updated_by: claude
---

# Research: SCUD Go Clone Spec - Full System Analysis

**Date**: 2026-03-11T17:51:20Z
**Researcher**: claude
**Git Commit**: b0d45a63a569372265d8560c11e73e416b71c039
**Branch**: scud-go
**Repository**: scud-go

## Research Question

Comprehensive analysis of the SCUD system: the Go rewrite specification (`go.md`) cross-referenced with the existing Rust implementation at `../scud/`. The goal is to understand every subsystem in detail to inform the Go implementation.

## Summary

SCUD is a DAG-based task management system with AI generation capabilities and multi-agent swarm execution. The Go rewrite spec (`go.md`, 758 lines) covers 12 major subsystems. The existing Rust implementation spans ~4 crates with ~200 source files and ~1.5M+ bytes of source code. The Go rewrite intentionally drops several subsystems (TUI, web viewer, ZMQ, Weave, Salvo, multiple backends) to focus on core functionality with Rho-only agent execution.

**Current state of scud-go:** No Go source code exists yet. The repo contains only the spec, CLAUDE.md, .scud/ config, and agent definitions.

---

## Detailed Findings

### 1. Data Model (go.md SS1, Rust: scud-core/src/models/)

#### Task (go.md SS1.1)

The `Task` struct is the fundamental unit. Key fields:

| Field | Go Type | Notes |
|---|---|---|
| `ID` | `string` | Pattern: `[a-zA-Z0-9_\-:.]`, max 100. `:` = phase separator, `.` = parent.subtask |
| `Title` | `string` | Max 200 chars |
| `Description` | `string` | Max 5000 chars |
| `Details` | `string` | Optional implementation notes |
| `TestStrategy` | `string` | Optional |
| `Status` | `TaskStatus` | 9 states: P, I, D, R, B, F, C, X, ! |
| `Complexity` | `uint` | Fibonacci only: 0,1,2,3,5,8,13,21,34,55,89 |
| `Priority` | `Priority` | C, H, M, L |
| `Dependencies` | `[]string` | Cross-phase OK: `"auth:1"` |
| `ParentID` | `string` | Set if subtask |
| `Subtasks` | `[]string` | Child IDs |
| `AgentType` | `AgentType` | builder, reviewer, tester, planner, researcher, analyzer |
| `ModelTier` | `ModelTier` | fast, standard, smart, custom |
| `ModelOverride` | `string` | Explicit model ID when tier=custom |
| `AssignedTo` | `string` | |
| `CreatedAt` | `string` | RFC 3339 |
| `UpdatedAt` | `string` | RFC 3339 |

**Rust implementation details (scud-core/src/models/task.rs):**
- `TaskStatus` is an enum with 9 variants; single-char codes used in SCG format
- `Task::new()` records timestamps via `chrono` (UTC RFC 3339)
- `has_dependencies_met()` checks all deps exist in slice AND have status `Done`
- `get_effective_dependencies()` recursively inherits parent deps for subtasks (walk ParentID chain, collect, dedup)
- `would_create_cycle()` uses DFS with path tracking
- `needs_expansion()`: complexity >= 5, not already expanded, not a subtask
- `recommended_subtasks_for_complexity()`: 5-8 -> 2, 13+ -> 3
- `ClaudeTask` bridge type converts SCUD DAG to/from Claude's flat task list format

**Dependency resolution:** A task is "ready" when `Status == Pending` AND every dep resolves to `Done`. Missing dep = blocked. Subtasks inherit parent deps.

#### Phase (go.md SS1.4)

```go
type Phase struct {
    Name     string
    Tasks    []Task
    IDFormat string // "sequential" (default) | "uuid"
}
```

**Rust implementation (scud-core/src/models/phase.rs):**
- `Phase` also carries Weave metadata (b-threads, roles, partitions, locks, weave edges, node annotations) - **out of scope for Go**
- `find_next_task()`: first Pending task with all deps Done
- `find_next_task_cross_tag()`: same but with cross-phase dep resolution
- `get_stats()`: skips subtasks from total; expanded parents with all subtasks Done count as Done; expanded parent complexity excluded
- `get_actionable_tasks()`: excludes expanded parents; includes subtasks only when parent is Expanded
- Natural sort: `"1" < "1.1" < "1.2" < "1.10" < "2" < "10"` - splits on `.`, compares segments numerically; UUIDs fall back to lexicographic

#### Agent x Model Tier (go.md SS1.3)

Two orthogonal axes:
- **Agent type** = WHAT (role, system prompt, tools): builder, reviewer, tester, planner, researcher, analyzer
- **Model tier** = HOW SMART (cost/capability): fast, standard, smart, custom

Auto-assignment from complexity: 0-2 -> builder+fast, 3-5 -> builder+standard, 8+ -> builder+smart. LLM may override agent type based on task nature.

---

### 2. SCG File Format (go.md SS2, Rust: scud-core/src/formats/scg.rs)

The SCG (SCUD Graph) format is a custom line-oriented text format at `.scud/tasks/tasks.scg`. Multiple phases separated by `\n---\n`.

#### Grammar

```
# SCUD Graph v1
# Phase: <name>

@meta { name <n>; id_format sequential|uuid; updated <rfc3339> }
@nodes  — id | title | status | complexity | priority
@edges  — dependent -> dependency
@parents — parent: child1, child2
@assignments — id | assigned_to
@agents — id | agent_type | model_tier
@details — <id> | field_name |
  Indented content (2 spaces)
```

**Status codes:** P(ending) I(n-progress) D(one) R(eview) B(locked) F(deferred) C(ancelled) X(expanded) !(failed)
**Priority codes:** C(ritical) H(igh) M(edium) L(ow)
**Escaping:** `\|` `\\` `\n` in pipe-delimited fields
**Pipeline mode:** `mode pipeline` in @meta adds `@pipeline` section and extended `@edges`

**Rust implementation (80KB in scud-core, 50KB in scud-cli):**
- State machine parser: reads header, then iterates lines tracking `current_section`
- `@nodes`: pipe-split 5+ fields, 6th optional for node annotations
- `@edges`: `->` for dependency edges; `~~`, `>>`, `!=` for behavioral edges (Weave, **out of scope**)
- `@parents`: `parent: child1, child2`
- `@details`: multi-line with 2-space indent continuation
- `@meta`: brace-delimited, also supports `mode pipeline`
- After parsing: 5-pass reconciliation applies edges/parents/details/assignments/agents to Task structs
- `natural_sort_ids()` for consistent ordering
- `escape_text()`/`unescape_text()`: backslash-escapes `\`, `|`, `\n`
- `split_by_pipe()`: handles `\|` escaped pipes during field splitting

**Sections out of scope for Go:** `@weave`, `@roles`, `@partitions`, `@locks`, behavioral edges

---

### 3. Storage (go.md SS3, Rust: scud-core/src/storage.rs)

#### Layout

```
.scud/
+-- tasks/tasks.scg
+-- config.toml
+-- active-tag           # plain text
+-- scud.db              # SQLite (WAL mode)
+-- guidance/*.md        # AI context files
+-- archive/             # timestamped .scg snapshots
```

#### File Locking

**Rust implementation:**
- Uses `fs2` crate for `flock()` semantics
- Reads: `file.lock_shared()` (multiple concurrent readers)
- Writes: `acquire_lock_with_retry()` - exponential backoff 10ms -> 1s, 10 retries, `try_lock_exclusive()`
- `update_group()` is the most atomic operation: opens file with `read+write+no-truncate`, acquires exclusive lock, reads current, parses, replaces phase, serializes all phases, seeks to 0, truncates, writes, lock released on drop

**Go equivalent:** Use `syscall.Flock()` or a cross-platform file lock library. The key pattern is that `update_group()` re-reads inside the exclusive lock to avoid TOCTOU races.

#### Active Tag

Plain text in `.scud/active-tag`. Cached in memory (`RwLock<Option<Option<String>>>`). Invalidated on set/clear.

---

### 4. Configuration (go.md SS4, Rust: scud-cli/src/config.rs)

`.scud/config.toml` with two separate model routing systems:

1. **`[llm]` section** - for SCUD's own AI commands (parse-prd, expand, check-deps):
   - `provider`, `model` (default)
   - `smart_provider`, `smart_model` (for validation/analysis)
   - `fast_provider`, `fast_model` (for generation)
   - `max_tokens`

2. **`[swarm]` section** - for agent execution:
   - `harness = "rho"` (only option in Go)
   - `round_size`
   - `[swarm.tiers]`: fast/standard/smart -> actual model IDs
   - `[swarm.backpressure]`: commands, stop_on_failure, timeout_secs

**Env overrides:** `SCUD_PROVIDER`, `SCUD_MODEL`, `SCUD_SMART_*`, `SCUD_FAST_*`, `SCUD_MAX_TOKENS`, `SCUD_SWARM_MODEL`

**Auto-detect backpressure:** Cargo.toml -> cargo; package.json -> npm scripts; go.mod -> go build+test; pyproject.toml -> pytest

---

### 5. LLM Client & AI Generation Pipeline (go.md SS5, Rust: scud-cli/src/llm/)

#### LLM Client (Rust: llm/client.rs)

`LLMClient` wraps `reqwest::Client` + `Config`. Two tiers:
- `complete_fast()` -> config.fast_model/fast_provider
- `complete_smart()` -> config.smart_model/smart_provider

**Provider dispatch** in `complete_with_model()`:
- `"claude-cli"` -> subprocess: `claude -p --output-format json`
- `"codex"` -> subprocess: `codex` binary
- `"cursor"` -> subprocess: `agent` binary
- `"anthropic"` -> HTTPS POST to `api.anthropic.com/v1/messages` with `x-api-key`
- `"anthropic-oauth"` -> same endpoint with OAuth Bearer token from Keychain
- `"xai"`, `"openai"`, `"openrouter"` -> OpenAI-compatible chat completions

**JSON extraction** (`extract_json()`): tries in order: ` ```json ` block -> ` ``` ` block -> first `[...]` -> first `{...}` -> raw trimmed response.

#### OAuth (Rust: llm/oauth.rs)

- Reads from macOS Keychain: `security find-generic-password -s "Claude Code-credentials" -w`
- Token validity: `expires_at > now_ms + 300_000` (5-minute buffer)
- Prefers OAuth over `ANTHROPIC_API_KEY` env var

#### Agent Provider (Rust: llm/provider.rs)

`AgentProvider` enum: Anthropic, OpenAI, Xai, OpenRouter, OpenCodeZen. Each has:
- `endpoint()` - hardcoded URL
- `resolve_credential()` - reads OAuth or env var
- `normalize_model()` - strips provider prefix for native APIs, keeps for aggregators
- `format_tool_definitions()` - Anthropic vs OpenAI-compatible format
- `send_request()` - dispatches to `send_anthropic_request()` or `send_openai_request()`

#### Agent Loop (Rust: llm/agent.rs)

`run_agent_loop()`: up to MAX_TURNS=200 iterations:
1. Send messages to LLM provider (streaming)
2. Process response: text blocks, thinking blocks, tool calls
3. If tool calls: execute sequentially, send results back, repeat
4. If no tool calls: exit
5. Supports cancellation, context compaction, steering injection

**Tools** (Rust: llm/tools.rs): 6 tools: Read, Write, Edit, Bash, Search, Find. Parameter names match Claude Code's schema. Edit uses `replacen(..., 1)` requiring exactly 1 match.

#### AI Commands

**`scud parse <file>`** (Rust: commands/ai/parse_prd.rs):
- Sends PRD + guidance to fast model
- Returns JSON array: title, description, priority, complexity, dependencies (1-indexed), agent_type
- Dep remapping: 1-indexed integers -> actual task IDs; out-of-range silently dropped; cross-phase strings kept as-is
- Agent type: LLM can set reviewer/planner/tester; builder overridden by complexity (<=2 -> fast-builder, >=3 -> builder)

**`scud expand`** (Rust: commands/ai/expand.rs):
- Processes tasks with `needs_expansion()` == true
- Concurrency: `buffer_unordered(10)` with 3 retries, 1s backoff
- Subtask IDs: `{parent_id}.1`, `{parent_id}.2` (sequential) or fresh UUIDs
- All subtasks get complexity=0, agent_type="fast-builder"
- Parent status set to Expanded

**`scud analyze-complexity`** (Rust: commands/ai/analyze_complexity.rs):
- Concurrency: 5 parallel
- Returns `{ complexity, reasoning }`, updates task

**`scud reanalyze-deps`** (Rust: commands/ai/reanalyze_deps.rs):
- Builds Markdown summary of all phases/tasks
- Returns dependency change suggestions (add/remove with reasoning)
- Interactive apply/skip per suggestion unless --apply

**`scud check-deps`** (Rust: commands/check_deps.rs):
- Structural: no cycles, no missing deps, no self-refs
- With --prd: LLM validation (coverage_score, missing_requirements, etc.)
- With --fix: auto-applies LLM-suggested fixes

**`scud generate`** (Rust: commands/generate.rs):
- Standard mode: parse-prd -> expand -> check-deps (with PRD validation + auto-fix)
- Pipeline mode (`--pipeline`): PRD -> LLM pipeline generation -> SCG pipeline format

#### Prompt Templates (Rust: llm/prompts.rs)

Methods on `Prompts` struct:
- `parse_prd(content, num_tasks, guidance)`
- `analyze_complexity(title, desc, details)`
- `expand_task(title, desc, complexity, details, recommended_subtasks, guidance)`
- `reanalyze_dependencies(task_context, phases)`
- `validate_tasks_against_prd(prd_content, tasks_json)`
- `fix_prd_issues(prd_content, tasks_json, validation)`
- `generate_pipeline(prd_content, goal, shape, checkpoints, tools, model_tier)`

#### Guidance System

`.scud/guidance/*.md` files concatenated (sorted by filename), injected into AI prompts as project-specific context.

---

### 6. Attractor Pipeline Engine (go.md SS6, Rust: scud-cli/src/attractor/)

A DOT-graph workflow execution engine. 26 source files, ~15 handler types.

#### Processing Pipeline

```
DOT file -> Parser -> PipelineGraph -> Transforms -> Validator -> Runner
                                                                    |
                                          AgentBackend <- Handler Registry
```

#### Graph Model (Rust: attractor/graph.rs)

`PipelineGraph` wraps `petgraph::DiGraph<PipelineNode, PipelineEdge>` with node name lookup, pre-resolved start/exit nodes.

**Shape-to-handler mapping:**

| DOT Shape | Handler Type |
|---|---|
| `Mdiamond` | `start` |
| `Msquare` | `exit` |
| `box`/`rect`/`rectangle` | `codergen` |
| `hexagon` | `wait.human` |
| `diamond` | `conditional` |
| `component` | `parallel` |
| `tripleoctagon` | `parallel.fan_in` |
| `parallelogram` | `tool` |
| `house` | `stack.manager_loop` |

Explicit `type` attribute overrides shape-derived handler.

**PipelineNode attributes:**
- `ID`, `Title`, `HandlerType`, `MaxRetries`, `RetryTarget`, `FallbackRetryTarget`
- `GoalGate` (bool), `Timeout` (duration string)
- `Prompt`, `LlmModel`, `LlmProvider`, `ReasoningEffort`
- `ExtraAttrs` map

**PipelineEdge attributes:**
- `Label`, `Condition`, `Weight` (int), `Fidelity`, `ThreadId`, `LoopRestart`

#### DOT Parser (Rust: attractor/dot_parser.rs)

Hand-written recursive-descent parser for a subset of DOT:
- Only `digraph` (not `graph`)
- No nested subgraphs
- Edge chains `a -> b -> c` expanded into individual edges via `windows(2)`
- `AttrValue` typed enum: Str, Int, Float, Bool, Duration
- Duration strings: `"30s"`, `"5m"`, `"1h"` parsed transparently

#### Runner Execution Loop (Rust: attractor/runner.rs)

```
1. Start at start node (or checkpoint resume point)
2. Execute node handler -> Outcome
3. Apply context_updates from Outcome atomically
4. Save checkpoint (after EVERY node)
5. Select next edge (5-step algorithm)
6. At exit nodes: check goal gates -> retry_target if unsatisfied
7. Retry on failure: exponential backoff + jitter, up to max_retries
8. Repeat until exit or failure
```

**Edge Selection (5-step algorithm):**

| Step | Logic |
|---|---|
| 1 | Edges with conditions matching outcome/context -> if exactly 1, take it |
| 2 | Match outcome.preferred_label against edge labels (case-insensitive) |
| 3 | Match outcome.suggested_next IDs against edge targets |
| 4 | Highest weight among unconditional edges |
| 5 | Lexical tiebreak on target node ID |

**Goal gate check:** Reads `"goal_satisfied"` from context. If exists and true (bool or string), satisfied. Defaults to true if absent.

#### Checkpoint/Resume (Rust: attractor/checkpoint.rs)

`Checkpoint`: current_node, completed_nodes, node_retries, node_statuses, context snapshot, log entries. Serialized to JSON in run directory. Saved after every node. Resume re-executes current node (at-least-once semantics).

#### Execution Context (Rust: attractor/context.rs)

`Arc<RwLock<HashMap<String, serde_json::Value>>>`. Thread-safe. Handlers return `context_updates` in Outcome; runner applies them atomically after handler returns. `clone_isolated()` for parallel branch isolation.

#### Conditions (Rust: attractor/conditions.rs)

Expression format: `key=value`, `key!=value`, `cond1 && cond2`. Key namespace: `"outcome"` -> outcome status, `"preferred_label"` -> outcome label, `"context.key"` or bare key -> context lookup.

#### Handlers

| Handler | Implementation | Notes |
|---|---|---|
| `start` | No-op, returns success | |
| `exit` | No-op, returns success | Goal gate + termination in runner, not handler |
| `codergen` | Expands variables in prompt, calls AgentBackend | $goal, $context.{key} substitution |
| `tool` | `sh -c <command>`, captures stdout/stderr | Command from extra_attrs["tool_command"] |
| `wait.human` | Reads outgoing edge labels as choices | Currently auto-selects first choice |
| `conditional` | No-op | Routing handled by edge conditions in runner |
| `parallel` | Records branch topology in context | Sets parallel.branches and parallel.branch_count |
| `fan_in` | Reads parallel.results, finds best success | Sets parallel.best_result |
| `rho` | Spawns `rho-cli` subprocess, parses stream-json | Streaming events: text_delta, tool_start, tool_result, complete |
| `manager` | Stub, returns success | Intended for child pipeline orchestration |

#### Model Stylesheet (Rust: attractor/stylesheet.rs)

CSS-like syntax:
```
* { model: "claude-3-haiku"; reasoning_effort: "medium" }
codergen { model: "claude-sonnet-4-20250514" }
```

Selectors: `*` (universal, specificity 0), `.class` (specificity 1), `#id` (specificity 2). Applied only when node doesn't have an explicit value.

#### Transforms (Rust: attractor/transforms.rs)

Two pre-execution transforms:
1. `expand_goal_variables()`: static `$goal` replacement in all node prompts
2. `apply_stylesheet_transform()`: parse and apply model stylesheet

#### Validator (Rust: attractor/validator.rs)

Rules: exactly one start node, at least one exit node, all nodes reachable from start, retry/fallback targets exist, start has no incoming edges, exit has no outgoing edges, valid condition syntax, known handler types (warning), goal_gate nodes should have retry_target (warning), codergen nodes should have prompts (warning).

#### Run Directory Layout

```
runs/{run_id}/
    checkpoint.json
    manifest.json
    {sanitized_node_id}/
        prompt.md
        response.md
        status.json
    artifacts/
```

#### SCG Pipeline Bridge (Rust: attractor/scg_bridge.rs)

Bidirectional conversion between SCG format (with `mode pipeline`) and PipelineGraph. Standard SCG with additions:
- `@pipeline` section: `id | handler_type | max_retries | retry_target | goal_gate | timeout`
- `@edges` extended: `from -> to | label | condition | weight`

---

### 7. Transcript Database (go.md SS7, Rust: scud-cli/src/db/)

#### SQLite Schema (WAL mode)

| Table | Key Columns |
|---|---|
| `sessions` | session_id, session_name, tag, terminal_mode, working_dir, round_size, started_at, completed_at |
| `agent_runs` | session_id, task_id, tag, wave_number, round_number, harness, model, prompt, spawned_at, success, duration_ms |
| `events` | session_id, task_id, agent_run_id, kind, success, duration_ms, tool_name, file_path, reason, data |
| `transcript_messages` | claude_session_id, scud_session_id, task_id, role, content, model, input_tokens, output_tokens |
| `tool_calls` | message_id, claude_session_id, tool_id, tool_name, input_json |
| `tool_results` | message_id, claude_session_id, tool_use_id, content, is_error |
| `validation_runs` | session_id, wave_number, all_passed |
| `validation_commands` | validation_run_id, command, passed, exit_code, stdout, stderr, duration_secs |
| `schema_version` | version (currently 1) |

#### Transcript Watcher (Rust: scud-cli/src/transcript_watcher.rs)

Background process watches Claude Code JSONL files, parses messages/tool_calls/tool_results, imports into SQLite.

#### Transcript CLI

```
scud transcript view [-s session] [-f] [--json]
scud transcript list
scud transcript search <query>
scud transcript stats
scud transcript import
```

---

### 8. Wave Planner (go.md SS8, Rust: scud-core/src/waves.rs)

**Algorithm: Modified Kahn's topological sort**

```
1. Collect actionable tasks:
   - status == Pending only
   - skip expanded parents (work subtasks instead)
   - include subtasks only if parent is Expanded

2. Build dependency graph with in-degree counts
   - subtasks get inherited parent deps
   - cross-phase deps resolved from full task map

3. Modified Kahn's:
   a. Queue all zero-in-degree tasks
   b. Pull up to max_parallel from queue -> one "round"
   c. Mark done, decrement dependents' in-degrees
   d. Add newly-zero tasks to queue
   e. Group rounds into waves (one wave = one dependency layer)
```

**Rust implementation (waves.rs:30):**
- Builds `HashSet<String>` of all task IDs in set
- Computes `in_degree` (count of deps within set) and `dependents` (reverse map)
- Cross-phase deps referencing IDs not in slice silently ignored
- Kahn loop: find all zero-in-degree, remove, decrement, emit as Wave
- If remaining but none zero: circular deps detected
- Returns `WaveResult { waves: Vec<Wave>, circular_deps: Vec<String> }`
- `detect_id_collisions()`: finds local IDs shared across multiple phase tags

---

### 9. Swarm Executor (go.md SS9, Rust: scud-cli/src/commands/swarm/)

#### Flow

```
for each wave:
    for each round (<= round_size tasks):
        spawn N goroutines, one Rho agent per task
        each goroutine:
            mark task InProgress
            resolve model from task.ModelTier via config.swarm.tiers
            build Rho AgentLoopConfig
            run agent_loop
            mark Done or Failed
        wait for all goroutines in round

    -- wave boundary --

    run backpressure validation
        if ALL pass: continue
        if ANY fail:
            attribute failure to tasks (git blame)
            mark attributed tasks Failed
            spawn repair agents (up to max_repair_attempts)
            re-run validation
            if still failing: leave Failed, continue
```

#### Model Resolution

```go
func resolveModel(task Task, config SwarmConfig) string {
    switch task.ModelTier {
    case TierCustom:
        if task.ModelOverride != "" { return task.ModelOverride }
        return config.Tiers.Standard
    case TierFast:  return config.Tiers.Fast
    case TierSmart: return config.Tiers.Smart
    default:        return config.Tiers.Standard
    }
}
```

#### Rho Bridge

Per task, construct AgentLoopConfig:
- **System prompt**: agent-type-specific base + guidance + task context
- **User prompt**: task ID, title, description, details, test strategy, completed dep titles
- **Tools**: Rho builtins (read, write, edit, bash, grep, find) + custom `scud_set_status`
- **Model**: resolved via tier -> config mapping

#### Backpressure (Rust: scud-cli/src/backpressure.rs)

Wave-level validation (NOT inside Rho's PostToolsHook):
- Runs configured shell commands via `sh -c` with timeout (default 300s)
- Auto-detect: Cargo.toml -> cargo, package.json -> npm scripts, go.mod -> go, pyproject.toml -> pytest
- `stop_on_failure` option
- `CommandResult`: command, passed, exit_code, stdout (truncated 1000 chars), stderr, duration_secs

#### Failure Attribution (Rust: scud-cli/src/attribution.rs)

When wave validation fails:
1. Parse error output for `file:line` references (5 regex patterns: Rust `-->`, TypeScript `file(line,col)`, Go/generic `file:line:col`, Python `File "...", line N`, fallback)
2. `git blame -L <line>,<line> --porcelain <file>` -> extract commit summary
3. `extract_task_id_from_commit()`: matches first `[...]` pattern in commit message
4. Fallback: `git log` + `git diff-tree --name-only` to find tasks whose changed files overlap error files
5. Confidence: 1 task -> High, 2-3 -> Medium, 0 -> Low (all wave tasks suspect)

#### Repair Loop

1. Mark attributed tasks Failed
2. Spawn repair agent (Rho, with error output + original task context)
3. Re-run validation
4. Repeat up to `max_repair_attempts` (default 3)
5. If still failing: leave Failed, continue to next wave

#### Execution Backends (Rust: scud-cli/src/backend/)

**Go scope: Rho only (via direct API)**

`AgentBackend` trait with `execute(AgentRequest) -> AgentHandle`:
- `CliBackend`: spawns headless runner subprocess, bridges stream events
- `DirectApiBackend`: calls `run_agent_loop()` directly (resolves provider, spawns agent loop + bridge task)
- `SimulatedBackend`: immediate mock response for testing/dry-run

**Rust also has:** Tmux, Extensions, Server, Beads runtimes - all **out of scope for Go**.

#### Swarm Session State (Rust: scud-cli/src/commands/swarm/session.rs)

Hierarchy: `SwarmSession -> Vec<WaveState> -> Vec<RoundState>`
- `WaveState`: start_commit, ValidationResult, ReviewState, Vec<RepairAttempt>
- Session locking: file lock at `.scud/swarm/<tag>.lock`

#### Events (Rust: scud-cli/src/commands/swarm/events.rs)

`EventWriter`: optional SQLite + optional ZMQ. ~18 event kinds spanning lifecycle, tool, file, dependency, wave, validation, repair, heartbeat.

**Go scope:** SQLite events only (no ZMQ).

---

### 10. CLI Commands (go.md SS10)

30+ commands organized in categories:

**Core:** init, create, tags, list, show, set-status (single + bulk), next, next-batch, stats, waves, warmup, commit, clean, migrate, convert, mermaid, assign, whois, log, doctor

**AI Generation:** parse, generate (standard + pipeline), expand, analyze-complexity, reanalyze-deps, check-deps

**Execution:** swarm, run, test, restart

**Attractor:** attractor run/validate/import/export

**Transcripts:** transcript view/list/search/stats/import

All accept `--project <dir>`.

---

### 11. What's Out of Scope for Go

| Feature | Status | Notes |
|---|---|---|
| TUI monitor (ratatui) | OUT | Use stdout from `scud swarm` |
| Web viewer (HTML/JS) | OUT | Use `scud list` / `scud mermaid` |
| ZMQ pub/sub | OUT | No real-time monitoring |
| Weave b-threads | OUT | No behavioral programming |
| Salvo git worktrees | OUT | No parallel worktree management |
| Multiple harness backends | OUT | Rho only |
| OpenCode server | OUT | Rho handles it |
| descartes-gui | OUT | No native GUI |
| scud-eval | OUT | No benchmarking |
| Ralph mode | OUT | Can add later |

---

### 12. Rust Implementation Scale

For reference, the existing Rust codebase that the Go rewrite targets:

| Crate | Files | Notable sizes |
|---|---|---|
| scud-core | ~8 files | scg.rs: 80KB, task.rs: 22KB, phase.rs: 14KB, storage.rs: 16KB |
| scud-cli | ~130+ files | swarm/mod.rs: 95KB, storage/mod.rs: 57KB, extensions/loader.rs: 69KB |
| scud-cli/attractor | ~26 files | runner.rs, graph.rs, dot_parser.rs, 10 handlers |
| scud-cli/llm | 6 files | client.rs, provider.rs, prompts.rs, tools.rs, agent.rs, oauth.rs |
| scud-weave | 5 files | coordinator.rs: 48KB, bthread.rs: 23KB |
| descartes-gui | ~15 files | main.rs: 135KB, scud_bridge.rs: 121KB |

---

## Architecture Documentation

### Key Design Patterns

1. **Orthogonal Agent x Tier matrix**: Agent type (system prompt + tools) is fully independent from model tier (which LLM to use). Any combination is valid.

2. **Two-tier model routing**: `[llm]` config for SCUD's own AI commands vs `[swarm.tiers]` for agent execution. Different models for task management vs coding.

3. **Wave-level backpressure**: Validation runs AFTER all agents in a wave complete, not during. Clear pass/fail gate between waves. Simpler than per-tool-call validation.

4. **Read-modify-write with exclusive lock**: `update_group()` re-reads the file inside its exclusive lock to prevent TOCTOU races. This is the critical atomicity pattern.

5. **Checkpoint after every node**: Attractor saves checkpoint after each node execution. Resume re-executes current node (at-least-once semantics).

6. **Outcome-driven routing**: Handlers return `Outcome` with `preferred_label` and `suggested_next` rather than directly manipulating the graph. The runner's 5-step algorithm resolves routing.

7. **Context as shared mutable state**: Thread-safe key-value store. Handlers return updates; runner applies atomically. `clone_isolated()` for parallel branches.

8. **Natural sort for task ordering**: Dot-separated numeric segments compared numerically (`"1.10" > "1.2"`). UUIDs fall back to lexicographic.

### Data Flow: End-to-End

```
PRD file
  -> scud generate (orchestrator)
    -> scud parse (fast model) -> tasks with deps + agent types
    -> scud expand (fast model, 10x concurrent) -> subtasks for complexity >= 5
    -> scud check-deps (smart model) -> PRD coverage validation + auto-fix
  -> .scud/tasks/tasks.scg (multi-phase file)

.scud/tasks/tasks.scg
  -> scud swarm (executor)
    -> wave planner (Kahn's) -> waves of parallel tasks
    -> per wave:
      -> per round (round_size tasks):
        -> resolve model (tier -> config mapping)
        -> build Rho AgentLoopConfig (system prompt + tools + task context)
        -> run_agent_loop (up to 200 turns)
        -> mark Done/Failed
      -> wave boundary: backpressure validation
        -> on failure: git blame attribution -> repair agents -> retry
    -> SQLite event log
```

## Open Questions

1. **Rho dependency**: The spec assumes Rho as the agent backend. How will Rho be integrated in Go? As a subprocess? Or will the Go version implement its own agent loop using the LLM client directly (like `DirectApiBackend`)?

2. **go.mod dependencies**: No Go module initialized yet. Key dependency decisions: petgraph equivalent for DAG (gonum?), SQLite driver (modernc.org/sqlite vs mattn/go-sqlite3?), TOML parser, HTTP client for LLM APIs.

3. **Concurrency model**: Rust uses tokio tasks with `buffer_unordered`. Go equivalent is goroutines with semaphore/worker pool. The wave planner's round system naturally maps to `sync.WaitGroup` per round.

4. **File locking portability**: `fs2` in Rust wraps `flock()`. Go's `syscall.Flock()` is Linux/macOS only. May need platform abstraction or use `os.OpenFile` with `O_EXCL` as fallback.

5. **SCG parser approach**: The Rust parser is a hand-written state machine. Go could use the same approach or leverage `bufio.Scanner` with section tracking. The format is simple enough that no parser generator is needed.
