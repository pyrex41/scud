# SCUD Core — Clone Spec (Go + Rho Harness)

**What's in:** Data model, storage, SCG format, AI generation pipeline, attractor pipelines, transcript DB, wave planner, swarm executor, backpressure/repair, LLM client.

**What's out:** TUI monitor, web viewer, ZMQ pub/sub, Weave b-threads, Salvo worktrees, multiple harness backends (tmux/extensions/server/beads — Rho only), descartes-gui, scud-eval.

---

## 1. Data Model

### 1.1 Task

```go
type TaskStatus string
const (
    Pending    TaskStatus = "pending"
    InProgress TaskStatus = "in-progress"
    Done       TaskStatus = "done"
    Failed     TaskStatus = "failed"
    Blocked    TaskStatus = "blocked"
    Review     TaskStatus = "review"
    Expanded   TaskStatus = "expanded"
    Deferred   TaskStatus = "deferred"
    Cancelled  TaskStatus = "cancelled"
)

type Priority string
const (
    Critical Priority = "critical"
    High     Priority = "high"
    Medium   Priority = "medium"
    Low      Priority = "low"
)

type AgentType string
const (
    AgentBuilder    AgentType = "builder"
    AgentReviewer   AgentType = "reviewer"
    AgentTester     AgentType = "tester"
    AgentPlanner    AgentType = "planner"
    AgentResearcher AgentType = "researcher"
    AgentAnalyzer   AgentType = "analyzer"
)

type ModelTier string
const (
    TierFast     ModelTier = "fast"      // cheap/fast model for simple tasks
    TierStandard ModelTier = "standard"  // balanced model (default)
    TierSmart    ModelTier = "smart"     // most capable model for complex tasks
    TierCustom   ModelTier = "custom"    // user-specified model override
)

type Task struct {
    ID           string
    Title        string       // max 200 chars
    Description  string       // max 5000 chars
    Details      string       // optional implementation notes
    TestStrategy string       // optional
    Status       TaskStatus
    Complexity   uint         // Fibonacci: 0,1,2,3,5,8,13,21,34,55,89
    Priority     Priority
    Dependencies []string     // cross-phase OK: "auth:1"
    ParentID     string       // set if subtask
    Subtasks     []string     // child IDs
    AgentType    AgentType    // WHAT it does: builder, reviewer, tester, planner, researcher, analyzer
    ModelTier    ModelTier    // HOW smart: fast, standard, smart, custom
    ModelOverride string      // explicit model ID when tier=custom
    AssignedTo   string
    CreatedAt    string       // RFC 3339
    UpdatedAt    string       // RFC 3339
}
```

**ID rules:** `[a-zA-Z0-9_\-:.]`, max 100. `:` separates phase:local. `.` separates parent.subtask.

**Dependency resolution:** Task is "ready" when `Status == Pending` AND every dep resolves to `Done`. Missing dep = blocked. Subtasks inherit parent deps (walk `ParentID` chain, collect, dedup).

**Cycle detection:** DFS with path tracking. `would_create_cycle(new_dep, all_tasks) → error | ok`.

**Expansion rules:**
- complexity 0–3 → 0 subtasks (no expansion)
- complexity 5–8 → 2 subtasks
- complexity 13+ → 3 subtasks
- `needs_expansion()` = complexity ≥ 5, not already expanded, not itself a subtask

### 1.3 Agent × Model Tier System

Agent type and model tier are **orthogonal axes**. Agent type describes WHAT the agent does (its role, system prompt, tool set). Model tier describes HOW smart the model is (cost/capability tradeoff). They combine at spawn time to select the actual LLM model.

**Agent types** (role-based):

| Agent | Role | System prompt focus |
|---|---|---|
| `builder` | Code implementation | Write code, run tests, commit |
| `reviewer` | Code review, quality | Read code, find issues, suggest fixes |
| `tester` | Test writing/automation | Write tests, validate coverage |
| `planner` | Design, architecture | Analyze requirements, produce plans |
| `researcher` | Investigation, research | Study codebase, find patterns, document |
| `analyzer` | Complexity/dependency analysis | Evaluate structure, suggest improvements |

**Model tiers** (capability-based):

| Tier | Use case | Default model |
|---|---|---|
| `fast` | Simple tasks, high throughput | `grok-code-fast-1` |
| `standard` | Balanced (default) | `grok-4.20-0309-reasoning` |
| `smart` | Complex reasoning, multi-step | `grok-4.20-multi-agent-0309` |
| `custom` | User override | `task.ModelOverride` |

**Resolution:** Any agent × any tier is valid. The model is resolved at spawn time:

```
resolved_model = config.model_for_tier(task.ModelTier)
if task.ModelTier == "custom" && task.ModelOverride != "" {
    resolved_model = task.ModelOverride
}
```

The tier-to-model mapping is configured in `.scud/config.toml` (see §4). The same `builder` agent can run on `fast` for a trivial config change or `smart` for a complex multi-file integration — the system prompt stays the same, only the model changes.

**Auto-assignment from complexity** (used by `scud parse` and `scud generate`):
- complexity 0–2 → `AgentBuilder` + `TierFast`
- complexity 3–5 → `AgentBuilder` + `TierStandard`
- complexity 8+ → `AgentBuilder` + `TierSmart`
- LLM may override agent type to `reviewer`/`planner`/`tester` based on task nature

**SCG serialization:** The `@agents` section stores both: `id | agent_type | model_tier` (third field optional, defaults to `standard`).

### 1.4 Phase

```go
type Phase struct {
    Name     string
    Tasks    []Task
    IDFormat string // "sequential" (default) | "uuid"
}
```

**Stats:** Subtasks excluded from `total`. Expanded parents with all subtasks Done count as Done. Expanded parent complexity excluded.

**Natural sort:** `"1" < "1.1" < "1.2" < "1.10" < "2" < "10"` (numeric by dot-segments, UUIDs lexicographic).

---

## 2. SCG File Format

### 2.1 Multi-Phase File

At `.scud/tasks/tasks.scg`. Phases separated by `\n---\n`.

### 2.2 Grammar

```
# SCUD Graph v1
# Phase: <name>

@meta {
  name <name>
  id_format sequential|uuid
  updated <RFC3339>
}

@nodes
# id | title | status | complexity | priority
1 | Implement login | P | 5 | H

@edges
# dependent -> dependency
2 -> 1

@parents
# parent: child1, child2
1: 1.1, 1.2

@assignments
# id | assigned_to
1 | alice

@agents
# id | agent_type | model_tier
1 | builder | smart

@details
<id> | description |
  Indented content (2 spaces)
<id> | details |
  More indented content
<id> | test_strategy |
  Test plan
```

**Status codes:** P I D R B F C X !  
**Priority codes:** C H M L  
**Escaping:** `\|` `\\` `\n` in pipe-delimited fields  
**Pipeline mode:** `mode pipeline` in @meta adds `@pipeline` section and extended `@edges` (label | condition | weight). See §6.

---

## 3. Storage

### 3.1 Layout

```
.scud/
├── tasks/tasks.scg
├── config.toml
├── active-tag           # plain text
├── scud.db              # SQLite (WAL mode)
├── guidance/*.md        # AI context files
└── archive/             # timestamped .scg snapshots
```

### 3.2 Locking

Reads: `flock(LOCK_SH)`. Writes: `flock(LOCK_EX)` with exponential backoff (10ms→1s, 10 retries). `UpdateGroup()` holds exclusive lock across full read→parse→modify→serialize→truncate→write→flush.

### 3.3 Active Tag

Plain text in `.scud/active-tag`. Cache in memory, invalidate on set/clear.

---

## 4. Configuration

`.scud/config.toml` (TOML):

```toml
[llm]
provider = "xai"                    # xai|anthropic|openai|openrouter|claude-cli
model = "xai/grok-code-fast-1"
smart_provider = "claude-cli"       # for AI generation commands (check-deps, etc.)
smart_model = "opus"
fast_provider = "xai"               # for AI generation commands (parse-prd, expand)
fast_model = "xai/grok-code-fast-1"
max_tokens = 16000

[swarm]
harness = "rho"
round_size = 5

# Model tier → actual model mapping (for agent execution)
[swarm.tiers]
fast     = "grok-code-fast-1"
standard = "grok-4.20-0309-reasoning"
smart    = "grok-4.20-multi-agent-0309"

[swarm.backpressure]
commands = ["cargo build", "cargo test"]
stop_on_failure = true
timeout_secs = 300
```

**Two separate model routing systems:**
- `[llm]` tiers (`smart_model`/`fast_model`): Used by AI generation commands (parse-prd, expand, check-deps, reanalyze-deps, analyze-complexity). These are SCUD's own LLM calls for task management.
- `[swarm.tiers]` (`fast`/`standard`/`smart`): Used by the swarm executor when spawning Rho agents to work on tasks. These are the models the coding agents use.

Env overrides: `SCUD_PROVIDER`, `SCUD_MODEL`, `SCUD_SMART_PROVIDER`, `SCUD_SMART_MODEL`, `SCUD_FAST_PROVIDER`, `SCUD_FAST_MODEL`, `SCUD_MAX_TOKENS`, `SCUD_SWARM_MODEL`.

**LLM tiers** (for SCUD's own AI commands): "fast" for generation tasks (parse-prd, expand). "smart" for validation/analysis (check-deps, reanalyze-deps, complexity analysis).

**Swarm tiers** (for agent execution): "fast", "standard", "smart" map to specific models via `[swarm.tiers]` config. Each task's `ModelTier` field selects which model the Rho agent uses.

Auto-detect backpressure: `Cargo.toml` → cargo build+test; `package.json` → npm scripts; `go.mod` → go build+test.

---

## 5. LLM Client & AI Generation Pipeline

### 5.1 LLM Client

Multi-provider HTTP client. Anthropic Messages API vs OpenAI-compatible Chat Completions. All AI operations return typed JSON (`complete_json_fast()` / `complete_json_smart()`).

**Providers:** xAI (`api.x.ai`), Anthropic (`api.anthropic.com`), OpenAI (`api.openai.com`), OpenRouter (`openrouter.ai`), claude-cli (subprocess).

### 5.2 `scud parse <file> -t tag -n N`

Sends PRD markdown + optional guidance to LLM (fast model). Returns JSON array of tasks:

```json
[{
  "title": "...",
  "description": "...",
  "priority": "high|medium|low",
  "complexity": 5,
  "dependencies": ["1", "2"],      // 1-indexed refs, NEVER "0"
  "agent_type": "builder",          // role: builder|reviewer|tester|planner|researcher|analyzer
  "model_tier": "standard"          // optional: fast|standard|smart (auto-assigned from complexity if omitted)
}]
```

Dependency refs are 1-indexed integers remapped to actual task IDs (sequential or UUID). Cross-phase deps kept as-is (`"auth:3"`).

**Auto-assignment** (when LLM doesn't specify or for post-processing):
- Agent type: `builder` by default, unless LLM identifies task as review/test/planning work
- Model tier from complexity: 0–2 → `fast`, 3–5 → `standard`, 8+ → `smart`

### 5.3 `scud expand [-i task] [-t tag]`

For tasks with complexity ≥ 5: sends task + guidance to LLM (fast model). Returns subtask array. Creates child tasks with `ParentID`, updates parent `Subtasks` list and `Status = Expanded`. Subtask IDs: `{parent_id}.1`, `{parent_id}.2`, etc.

Recommended subtask count: complexity 5–8 → 2, complexity 13+ → 3.

### 5.4 `scud analyze-complexity [-i task] [-t tag]`

Sends task title/description/details to LLM (smart model). Returns `{ "complexity": N, "reasoning": "..." }`. Updates task complexity.

### 5.5 `scud reanalyze-deps [-t tag] [--apply] [--dry-run]`

Sends full task context across all phases to LLM (smart model). Returns dependency change suggestions:

```json
[{
  "task_id": "api:3",
  "add_dependencies": ["auth:1"],
  "remove_dependencies": [],
  "reasoning": "..."
}]
```

Interactive apply/skip per suggestion unless `--apply`.

### 5.6 `scud check-deps [-t tag] [--prd <file>] [--fix]`

Structural check: validates no circular deps, no missing deps, no self-refs.

With `--prd`: sends PRD + tasks to LLM (smart model) for coverage validation. Returns:

```json
{
  "coverage_score": 85,
  "missing_requirements": [...],
  "incomplete_coverage": [...],
  "misaligned_tasks": [...],
  "agent_type_issues": [...],
  "summary": "..."
}
```

With `--fix`: sends issues back to LLM for auto-fix, applies `update_task`, `update_dependency`, `update_agent_type` actions.

### 5.7 `scud generate <file> -t tag -n N [--pipeline]`

**Standard mode:** Orchestrates the full pipeline: parse-prd → expand → check-deps (with PRD validation + auto-fix). Each phase can be skipped (`--no-expand`, `--no-check-deps`).

**Pipeline mode (`--pipeline`):** Sends PRD to LLM with pipeline generation prompt. Returns JSON defining an Attractor pipeline (nodes + edges + handler types). Converts to SCG pipeline format and saves.

### 5.8 Prompt Templates

All prompts live in a `Prompts` struct with these methods (reproduce verbatim from source):
- `parse_prd(content, num_tasks, guidance)` 
- `analyze_complexity(title, desc, details)`
- `expand_task(title, desc, complexity, details, recommended_subtasks, guidance)`
- `reanalyze_dependencies(task_context, phases)`
- `validate_tasks_against_prd(prd_content, tasks_json)`
- `fix_prd_issues(prd_content, tasks_json, validation)`
- `generate_pipeline(prd_content, goal, shape, checkpoints, tools, model_tier)`

### 5.9 Guidance System

`.scud/guidance/*.md` files are concatenated (sorted by filename) and injected into AI prompts as project-specific context. Loaded by `storage.load_guidance()`.

---

## 6. Attractor Pipeline Engine

### 6.1 Overview

DOT-graph workflow engine. Pipelines defined as directed graphs where nodes are processing steps and edges are transitions with optional conditions.

```
DOT file → Parser → PipelineGraph → Transforms → Validator → Runner
                                                                 ↓
                                           AgentBackend ← Handler Registry
```

### 6.2 Pipeline Node Types

| Handler | Description |
|---|---|
| `start` | Entry point, no processing |
| `codergen` | LLM-powered code generation/analysis (Rho agent) |
| `tool` | Runs a shell command |
| `wait.human` | Pauses for human approval |
| `parallel` | Fork into parallel branches |
| `fan_in` | Merge parallel branches |
| `conditional` | Branching based on context |
| `manager` | Orchestration logic |
| `exit` | Pipeline completion; `goal_gate` + `retry_target` for looping |

### 6.3 Pipeline Node Attributes

```go
type PipelineNode struct {
    ID                  string
    Title               string
    HandlerType         string
    MaxRetries          uint
    RetryTarget         string  // node ID to jump to on retry exhaustion
    FallbackRetryTarget string  // secondary fallback
    GoalGate            bool    // if true, checked at exit nodes
    Timeout             string  // e.g. "5m"
    ExtraAttrs          map[string]string
}
```

### 6.4 Edge Model

```go
type PipelineEdge struct {
    Label     string // human-readable
    Condition string // e.g. "outcome=success"
    Weight    int    // higher = preferred
}
```

### 6.5 Execution (Runner)

Core loop:

```
1. Start at start node
2. Execute node handler → get Outcome
3. Apply context_updates from Outcome
4. Save checkpoint
5. Select next edge (5-step algorithm)
6. At exit nodes: check goal gates → retry_target if unsatisfied
7. Retry on failure: exponential backoff + jitter, up to max_retries
8. Repeat until exit or failure
```

### 6.6 Edge Selection (5-step algorithm)

```
Step 1: Find edges with conditions that match current outcome/context → if exactly 1, take it
Step 2: Match outcome.preferred_label against edge labels (case-insensitive)
Step 3: Match outcome.suggested_next IDs against edge targets
Step 4: Highest weight among unconditional edges
Step 5: Lexical tiebreak on target node ID
```

### 6.7 Checkpoint/Resume

`Checkpoint` serialized to JSON in run directory. Contains: `current_node`, completed node statuses, context snapshot, retry counts. `--resume <path>` resumes from checkpoint.

### 6.8 Execution Context

Thread-safe key-value store (`PipelineContext`). Handlers read/write context. Context snapshots saved with checkpoints.

### 6.9 Conditions

Expression format: `key=value`, `outcome=success`, `outcome=failure`. Parsed and evaluated against outcome + context snapshot.

### 6.10 Model Stylesheet

CSS-like syntax for model/provider configuration:

```
* { model: "claude-3-haiku"; reasoning_effort: "medium" }
codergen { model: "claude-sonnet-4-20250514" }
```

### 6.11 SCG Pipeline Format

Standard SCG with `mode pipeline` in `@meta`. Adds:
- `@pipeline` section: `id | handler_type | max_retries | retry_target | goal_gate | timeout`
- `@edges` extended: `from -> to | label | condition | weight`

### 6.12 CLI

```
scud attractor run <file> [--resume F] [--headless] [--simulated] [--model M] [--provider P]
scud attractor validate <file>
scud attractor import <file> [-o output]
scud attractor export <file> [--format dot] [-o output]
```

---

## 7. Transcript Database

### 7.1 SQLite Schema

At `.scud/scud.db` (WAL mode). Full schema:

**sessions:** session_id (PK), session_name, tag, terminal_mode, working_dir, round_size, started_at, completed_at

**agent_runs:** id (PK), session_id, task_id, tag, wave_number, round_number, harness, model, prompt, window_name, spawned_at, completed_at, success, duration_ms, exit_code

**events:** id (PK), timestamp, session_id, task_id, agent_run_id, kind, success, duration_ms, tool_name, file_path, dependency_id, reason, data

**transcript_messages:** id (PK), claude_session_id, scud_session_id, task_id, timestamp, uuid, parent_uuid, role, content, model, input_tokens, output_tokens

**tool_calls:** id (PK), message_id, claude_session_id, timestamp, tool_id, tool_name, input_json

**tool_results:** id (PK), message_id, claude_session_id, timestamp, tool_use_id, content, is_error

**validation_runs:** id (PK), session_id, wave_number, all_passed, started_at, completed_at

**validation_commands:** id (PK), validation_run_id, command, passed, exit_code, stdout, stderr, duration_secs

**schema_version:** version (PK) — currently 1

### 7.2 Transcript Watcher

Background process watches Claude Code JSONL files and imports into SQLite. Parses messages, tool calls, and tool results from JSONL entries.

### 7.3 CLI

```
scud transcript view [-s session] [-f] [--json]
scud transcript list
scud transcript search <query>
scud transcript stats
scud transcript import
```

---

## 8. Wave Planner

### 8.1 Algorithm

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
   b. Pull up to max_parallel from queue → one "round"
   c. Mark done, decrement dependents' in-degrees
   d. Add newly-zero tasks to queue
   e. Group rounds into waves (one wave = one dependency layer)
```

---

## 9. Swarm Executor (Rho-Powered)

### 9.1 Flow

```
for each wave:
    for each round (≤ round_size tasks):
        spawn N goroutines, one Rho agent per task
        each goroutine:
            mark task InProgress
            resolve model from task.ModelTier via config.swarm.tiers
            build Rho AgentLoopConfig (agent-type-specific system prompt + tools)
            run agent_loop
            mark Done or Failed based on outcome
        wait for all goroutines in round

    ── wave boundary ──
    
    run backpressure validation (unless --no-validate)
        execute configured shell commands (cargo build, cargo test, etc.)
        if ALL pass: continue to next wave
        if ANY fail:
            attribute failure to specific tasks (git blame)
            mark attributed tasks as Failed
            spawn repair agents (unless --no-repair, up to max_repair_attempts)
            re-run validation after repair
            if still failing: leave tasks Failed, continue to next wave
```

**Key design decision:** Backpressure runs at the SCUD wave level, NOT inside Rho's `PostToolsHook` system. This is simpler and gives a clear pass/fail gate between waves. Rho agents run to completion without interruption; validation happens after the dust settles.

### 9.2 Model Resolution

At spawn time, each task's model is resolved from the agent×tier matrix:

```go
func resolveModel(task Task, config SwarmConfig) string {
    switch task.ModelTier {
    case TierCustom:
        if task.ModelOverride != "" {
            return task.ModelOverride
        }
        return config.Tiers.Standard  // fallback
    case TierFast:
        return config.Tiers.Fast
    case TierSmart:
        return config.Tiers.Smart
    default: // TierStandard
        return config.Tiers.Standard
    }
}
```

The agent type determines the **system prompt and tool set** (builder gets write/edit/bash focused prompts; reviewer gets read-heavy analysis prompts; tester gets test-focused prompts). The tier determines the **model** (fast/standard/smart). They're fully independent.

### 9.3 Rho Bridge

Per task, construct `AgentLoopConfig`:

- **System prompt**: Agent-type-specific base instructions + concatenated project guidance + task context
- **User prompt**: Task ID, title, description, details, test strategy, completed dependency titles, implementation instructions
- **Tools**: Rho builtins (read, write, edit, bash, grep, find) + custom `scud_set_status` tool
- **Model**: Resolved via §9.2 (task.ModelTier → config.swarm.tiers → actual model ID)
- **PostToolsHooks**: None for backpressure (handled at wave level). Rho's hooks are available for other uses (e.g., lint-on-save) but SCUD doesn't inject backpressure here.

### 9.4 Rho Agent Loop (What It Does)

1. Send system prompt + messages to LLM provider (streaming)
2. Receive response: text, thinking blocks, tool calls
3. If tool calls: execute sequentially, send results back, repeat
4. If no tool calls: check follow-up messages, otherwise exit
5. Supports cancellation, context compaction, steering injection

### 9.5 Wave-Level Backpressure

After ALL agents in a wave complete, SCUD runs validation:

```go
type ValidationResult struct {
    AllPassed bool
    Failures  []string
    Results   []CommandResult
}
type CommandResult struct {
    Command     string
    Passed      bool
    ExitCode    *int
    Stdout      string  // truncated 1000 chars
    Stderr      string
    DurationSec float64
}
```

Commands run via `sh -c` with configurable timeout (default 300s). If `stop_on_failure` is true, stop at first failing command.

### 9.6 Failure Attribution

When wave-level validation fails:
1. Parse error output for `file:line` references
2. Run `git blame -L <line>,<line> <file>`
3. Extract task IDs from commit message prefixes (`[TASK-ID] message`)
4. Confidence: High (1 task matches), Medium (2–3 tasks), Low (can't determine → all wave tasks suspect)

### 9.7 Repair Loop

On validation failure:
1. Mark attributed tasks as `Failed`
2. Spawn repair agent (Rho, with error output + original task context injected into prompt)
3. Re-run wave-level validation
4. Repeat up to `max_repair_attempts` (default 3)
5. If still failing: leave tasks as Failed, log details, continue to next wave

---

## 10. CLI Commands

All accept `--project <dir>`.

### Core

| Command | Description |
|---|---|
| `scud init [--provider P]` | Create .scud/ structure |
| `scud create <title> [--agent A] [--tier T] [-t tag]` | Create task with agent+tier |
| `scud tags [name]` | List/set active tag |
| `scud list [-t tag] [-s status] [--json]` | List tasks |
| `scud show <id> [-t tag]` | Task details |
| `scud set-status <id> <status> [-t tag]` | Update status |
| `scud set-status --from S --to S` | Bulk transition |
| `scud next [-t tag] [--all-tags]` | Next ready task |
| `scud next-batch [-t tag] [-l limit]` | Multiple ready tasks |
| `scud stats [-t tag]` | Phase statistics |
| `scud waves [-t tag] [-n max] [--all-tags]` | Execution plan |
| `scud warmup` | Session orientation |
| `scud commit [-m msg] [-a]` | Task-aware git commit |
| `scud clean [-t tag] [--force] [--delete] [--list] [--restore]` | Archive/delete |
| `scud migrate [--dry-run]` | Format migration |
| `scud convert --from F --to T` | JSON↔SCG |
| `scud mermaid [-t tag] [--all-tags]` | Mermaid diagram |
| `scud assign <id> <person>` | Assign task |
| `scud whois [-t tag]` | Show assignments |
| `scud log <id> <summary>` | Write log entry |
| `scud doctor workflow [--fix]` | Diagnose stuck tasks |

### AI Generation

| Command | Description |
|---|---|
| `scud parse <file> -t tag [-n N] [--model M] [--append]` | PRD → tasks |
| `scud generate <file> -t tag [-n N] [--pipeline] [--no-expand] [--no-check-deps]` | Full pipeline |
| `scud expand [-i task] [-t tag] [--model M]` | Expand complex tasks |
| `scud analyze-complexity [-i task] [-t tag]` | Analyze complexity |
| `scud reanalyze-deps [-t tag] [--apply] [--dry-run]` | Cross-tag dep suggestions |
| `scud check-deps [-t tag] [--prd F] [--fix] [--all-tags]` | Validate deps |

### Execution

| Command | Description |
|---|---|
| `scud swarm [-t tag] [-n size] [--dry-run] [--no-validate] [--no-repair] [--all-tags]` | Wave execution |
| `scud run <prompt> [-H harness] [-M model]` | Single agent |
| `scud test [-c cmd] [-n max] [-a agent]` | Test + auto-repair |
| `scud restart <id>` | Re-spawn task |

### Attractor

| Command | Description |
|---|---|
| `scud attractor run <file> [--resume] [--headless] [--simulated]` | Execute pipeline |
| `scud attractor validate <file>` | Validate pipeline |
| `scud attractor import <file>` | Import pipeline |
| `scud attractor export <file>` | Export pipeline |

### Transcripts

| Command | Description |
|---|---|
| `scud transcript view [-s session]` | View transcript |
| `scud transcript list` | List transcripts |
| `scud transcript search <query>` | Search content |
| `scud transcript stats` | Statistics |
| `scud transcript import` | Import to DB |

---

## 11. What's Explicitly Out of Scope

- **TUI monitor** (ratatui) — use stdout from `scud swarm`
- **Web viewer** (HTML/JS/Mermaid) — use `scud list` / `scud mermaid`
- **ZMQ event publishing** — no real-time monitoring infrastructure
- **Weave b-thread coordination** — no behavioral programming engine
- **Salvo git worktrees** — no parallel worktree management
- **Multiple harness backends** (tmux, extensions, server, beads) — Rho only
- **OpenCode server integration** — Rho handles it
- **descartes-gui** — no native GUI
- **scud-eval** — no benchmarking framework
- **Ralph mode** — can add later (sequential loop variant of swarm)

All can be added later without core architecture changes.

---

## 12. Implementation Order

```
Week 1:   Task/Phase model, SCG parser/serializer, storage with locking
Week 2:   Dependency engine, wave planner, core CLI (init, list, next, set-status, stats, waves)
Week 3:   LLM client (multi-provider), prompt templates
Week 4:   AI commands: parse-prd, expand, analyze-complexity, check-deps, reanalyze-deps, generate
Week 5:   Rho bridge, single-task agent execution, swarm executor (parallel rounds + waves)
Week 6:   Backpressure, failure attribution, repair loop
Week 7:   Attractor engine: DOT parser, graph, runner, handlers, checkpoint/resume
Week 8:   SQLite schema, transcript watcher, transcript CLI
Week 9:   Polish: remaining CLI commands, edge cases, error messages
Week 10:  Testing, documentation
```
