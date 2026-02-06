# B-Thread Coordination Layer for SCUD — v2

## Mission

Build `scud-weave`: a behavioral programming coordination layer that integrates natively into SCUD's DAG-based task scheduler and SCG file format. The system implements David Harel's b-thread synchronization model — request/wait/block at synchronization points — to prevent inter-agent conflicts, enforce structured backpressure, enable agent specialization with guardrails, and allow the human operator to engineer away failure domains one b-thread at a time.

This sits between SCUD's macro-scheduling (what work exists, what's ready) and the actual execution by Claude Code agents — whether solo ralph-loops, parallel orchestrator sessions, or Claude Agent Teams.

---

## Part I: Theory & Motivation

### The Coordination Gap

Three systems exist for orchestrating AI coding agents. Each solves part of the problem. None solves all of it.

**SCUD** models work as a DAG. Tasks have dependencies, status, complexity, and priority. Wave analysis computes which tasks can run in parallel. `claim`/`release` provides task-level locking. But within a wave of ready tasks, there's no coordination — three "ready" tasks might be independently safe to start but have emergent conflicts when pursued simultaneously: touching the same module, making incompatible architectural assumptions, introducing duplicate abstractions.

**Claude Agent Teams** provide parallel execution with inbox-based messaging and a shared task list. The lead agent coordinates, teammates work independently. But there's no mechanism for one agent to *prevent* another from doing something. Coordination is reactive — the docs explicitly say "check in on teammates' progress, redirect approaches that aren't working." By the time you redirect, work is wasted.

**The ralph loop pattern** (Huntley) argues for monolithic single-agent execution: one process, one task per loop, watch the loop, engineer away failure modes. Backpressure — tests, linting, type checking, pre-commit hooks — rejects bad generations. But backpressure today is binary (pass/fail) and global. There's no *conditional* backpressure: "this action is fine unless someone else is simultaneously doing that."

### What B-Threads Add

The b-thread model (Harel, Marron & Weiss, CACM 2012) provides exactly the missing primitive: **compositional behavioral constraints evaluated at synchronization points.**

Each b-thread independently declares, for a given event:
- **Request**: I want this to happen
- **Wait-for**: Tell me when this happens
- **Block**: Prevent this from happening

A coordinator collects all declarations and allows only events that are requested by someone and blocked by no one. This gives you:

1. **Incremental authoring**: Watch the loop. See a failure mode. Add a b-thread. Don't touch existing ones.
2. **Compositional safety**: B-threads compose by intersection — adding a thread can only restrict behavior, never allow previously blocked events.
3. **Deterministic coordination**: Same state + same event = same decision. Always.
4. **Conditional backpressure**: "Commit is fine, unless you've modified files since last TestPass" or "FileWrite to schema.rs is fine, unless another agent holds the schema mutex."

### Lessons from the C Compiler Case Study (Carlini, Anthropic 2026)

Nicholas Carlini ran 16 parallel Claude agents for 2 weeks to build a 100,000-line C compiler. Every coordination failure maps to a b-thread pattern:

| Problem | What happened | B-thread solution |
|---|---|---|
| **Task convergence** | All 16 agents fixed same kernel bug, overwrote each other | `Partition` rule: deterministic work sharding |
| **Regression** | New features broke existing tests; discovered after push | `Require` rule: gate commits on TestPass |
| **Merge conflicts** | Frequent with 16 agents modifying overlapping files | `Mutex` rule: per-file mutual exclusion |
| **Context pollution** | Test output overwhelmed context windows | Design principle: `scud weave check` returns one line |
| **Time blindness** | Agents spent hours on tests | `Timeout` rule: kill operations exceeding budget |
| **Accidental self-kill** | Claude ran `pkill -9 bash` | `BlockAlways` rule: unconditional guard |
| **Role drift** | Specialized agents drifted into other domains | `@roles` section: scope-restricted b-thread bundles |
| **Fresh-container orientation** | New sessions wasted time on project state | `scud weave summary`: structured orientation from event log |
| **Architecture decisions** | Some decisions needed human, not autonomous action | `escalate` flag: block + notify operator |

Critical insight: **problems escalated through a predictable sequence** — file conflicts → task duplication → semantic conflicts → regression → quality plateau. Each level maps to a b-thread added incrementally. This is exactly how the model is designed to work.

---

## Part II: SCG Format Extension

### Current SCG Format (v1)

SCG is a pipe-separated, section-based text format optimized for token efficiency (~75% reduction vs JSON). Current spec:

```
# SCUD Graph v1
# Phase: <tag>

@meta {
  name <tag>
  updated <iso8601>
}

@nodes
<id> | <title> | <status> | <complexity> | <priority>

@edges
<dependent> -> <dependency>

@parents
<parent_id>: <subtask_id>, <subtask_id>

@assignments
<id> | <assigned_to> | <locked_by> | <locked_at>

@details
<id> | description |
  <multiline content indented 2 spaces>
```

Status codes: P (Pending), I (InProgress), D (Done), R (Review), B (Blocked), F (Deferred), C (Cancelled), X (Expanded).

ID format: `<phase>:<local_id>[.<subtask>...]` — e.g., `auth:1`, `auth:1.1`, `api:2.3.1`.

Phases separated by `---`.

### Design Principle for Extensions

**Keep the existing format intact. Add new optional sections.** An SCG file with `@weave` sections is valid to a parser that doesn't understand them (unknown sections are ignored). An SCG file without them works exactly as before. The weave sections are per-phase, just like everything else.

### New Section: `@weave`

Defines b-threads inline in the SCG file. Behavioral constraints live alongside the tasks they govern.

```
@weave
# id | name | priority | enabled | rule_type | rule_spec
w:1 | File mutex | 5 | Y | Mutex | kind=FileWrite key=file:{target}
w:2 | Test gate | 10 | Y | Require | trigger=Commit prereq=TestPass reset=FileWrite
w:3 | Schema lock | 3 | Y | Mutex | kind=SchemaChange key=schema-global
w:4 | Commit rate | 50 | Y | RateLimit | kind=Commit max=5 window=120
w:5 | API review | 15 | N | BlockUntil | trigger=ApiChange block=Build until=ApiReviewApproved
w:6 | No self-kill | 1 | Y | BlockAlways | kind=DangerousCommand
w:7 | Time budget | 30 | Y | Timeout | kind=TestRun max_secs=300
```

Format: `id | name | priority | enabled | rule_type | rule_spec`

- **id**: `w:<n>` namespace to avoid collision with task IDs.
- **priority**: Lower = higher priority. Evaluated in order.
- **enabled**: `Y` or `N`. Disabled threads are ignored by coordinator.
- **rule_type**: One of the defined rule types (see below).
- **rule_spec**: `key=value` pairs, space-separated. Token-efficient and greppable.

For complex rules requiring multiple sub-rules, use the `@details` pattern:

```
@details
w:8 | rules |
  type=Require trigger=Commit prereq=TestPass reset=FileWrite
  type=Require trigger=Commit prereq=LintPass reset=FileWrite
```

This allows one b-thread to contain multiple rules using the existing multiline mechanism.

### Rule Types

| Rule type | Purpose | Key parameters |
|---|---|---|
| `Mutex` | Only one agent can hold a resource at a time | `kind`, `key` (template: `{target}`, `{agent}`) |
| `Require` | Action X requires prior event Y | `trigger`, `prereq`, `reset` |
| `BlockUntil` | Block event Y after event X until event Z | `trigger`, `block`, `until` |
| `BlockAlways` | Unconditionally block matching events | `kind`, (optional: `target`, `agent`) |
| `RateLimit` | Max N events in time window | `kind`, `max`, `window` (seconds) |
| `Timeout` | Kill operations exceeding time budget | `kind`, `max_secs`, `action` (kill/warn) |
| `Partition` | Deterministically shard work across agents | `kind`, `target`, `strategy`, `count` |

### New Section: `@roles`

Defines agent roles with scope constraints. Each role generates implicit `BlockAlways` b-threads.

```
@roles
# role_id | name | allow_pattern | deny_pattern
r:impl | Implementer | src/** | docs/**
r:test | Test writer | tests/** src/test_*.rs | src/main.rs src/lib.rs
r:docs | Documenter | docs/** *.md | src/**
r:perf | Optimizer | src/optimize/** src/codegen/** | src/parser/** src/lexer/**
r:review | Reviewer | - | -
```

Format: `role_id | name | allow_pattern | deny_pattern`

- `role_id`: `r:<name>` namespace.
- `allow_pattern`: Glob patterns the agent CAN write to. `-` = everything.
- `deny_pattern`: Glob patterns the agent CANNOT write to. `-` = nothing denied.
- Multiple patterns are space-separated.

Semantics: If an agent with role `r:docs` attempts `FileWrite` to `src/main.rs`, the coordinator blocks it because `src/**` is in the deny pattern. This is enforced as an implicit `BlockAlways` — no explicit b-thread entry needed.

### New Section: `@partitions`

Deterministic work sharding — the b-thread formalization of Carlini's GCC oracle trick.

```
@partitions
# partition_id | scope_pattern | strategy | agent_count
p:1 | src/codegen/** | hash | 4
p:2 | src/parser/** | round-robin | 3
p:3 | tests/** | directory | 4
```

Format: `partition_id | scope_pattern | strategy | agent_count`

Strategies:
- `hash`: Deterministic hash of file path modulo agent count → agent assignment.
- `round-robin`: Files sorted alphabetically, assigned round-robin.
- `directory`: Top-level subdirectories assigned to agents.

A partition generates implicit `Mutex` b-threads where each agent can only write to files in its assigned partition. Agent identity is mapped at runtime: agent-0 gets partition slice 0, agent-1 gets slice 1, etc.

### New Section: `@locks`

Runtime state — current mutex locks held by the coordinator. This section is *written by the coordinator*, not authored by humans.

```
@locks
# lock_key | holder_agent | task_id | acquired_at | ttl_secs
file:src/db/schema.rs | agent-1 | auth:1.1 | 2026-02-06T10:23:15Z | 3600
schema-global | agent-1 | auth:1.1 | 2026-02-06T10:23:15Z | 7200
```

Format: `lock_key | holder_agent | task_id | acquired_at | ttl_secs`

This complements the existing `@assignments` section. `@assignments` tracks task-level assignment ("who is working on this task"). `@locks` tracks resource-level mutual exclusion ("who holds the lock on this file/schema/module"). Different granularity, both needed.

**TTL prevents deadlocks from crashed agents.** When TTL expires, the lock is automatically released. Default TTL: 3600s (1 hour) for file locks, configurable per b-thread.

Why in the SCG file rather than a separate state file? Because `@assignments` already puts runtime state (locked_by, locked_at) in the SCG file. Following that precedent keeps coordination state co-located and git-visible.

### Extended `@nodes` with Weave Annotations

The existing node format gains optional trailing `key=value` pairs:

```
@nodes
auth:1   | Design auth system      | X | 13 | H
auth:1.1 | Implement JWT tokens    | I | 5  | H | role=r:impl scope=src/auth/**
auth:1.2 | Add rate limiting       | P | 8  | M | role=r:impl scope=src/middleware/**
auth:1.3 | Write auth tests        | P | 5  | M | role=r:test scope=tests/auth/**
auth:1.4 | Document auth API       | P | 3  | L | role=r:docs scope=docs/auth.md
```

New optional trailing field after priority: `key=value` pairs, space-separated.

- `role=r:impl`: Agent working this task must have the Implementer role.
- `scope=src/auth/**`: This task's "territory" — files the agent is expected to modify. Used by the coordinator for automatic partition inference and conflict detection.

Parsers that don't understand weave annotations ignore everything after the 5th pipe-separated field. **Full backward compatibility.**

### Extended `@edges` with Behavioral Edge Types

Currently edges represent task dependencies: `child -> parent` means child depends on parent. Add new edge types:

```
@edges
# Dependency edges (existing — these are unchanged)
auth:1.1 -> auth:1
auth:1.2 -> auth:1

# Conflict edges (new): these tasks conflict if run simultaneously
auth:1.1 ~~ auth:1.2 | reason=shared-middleware

# Sequence edges (new): behavioral ordering, no data dependency
auth:1.1 >> auth:1.3 | reason=tests-need-impl-first

# Exclusion edges (new): must NOT run on same agent
auth:1.1 != auth:1.4 | reason=different-specialization
```

| Edge | Operator | Semantics | Generated b-thread |
|---|---|---|---|
| Dependency | `->` | A depends on B (existing) | None (handled by DAG) |
| Conflict | `~~` | A and B conflict if concurrent | `Mutex` on scope intersection |
| Sequence | `>>` | A must complete before B starts | `Require` on A's TaskComplete |
| Exclusion | `!=` | A and B must run on different agents | `BlockAlways` if same agent |

The `| reason=...` suffix is optional metadata for `scud weave explain`.

Conflict and sequence edges express constraints *within* a wave. Two tasks might both be "ready" (no data dependency) but have a behavioral conflict that prevents truly concurrent execution. This is the gap between DAG scheduling and behavioral coordination.

### Interaction Between `@assignments` and `@locks`

Current `@assignments` section:
```
@assignments
auth:1.2 | alice | alice | 2025-01-15T09:00:00Z
```

This tracks: who is *assigned* to this task, who has it *locked* (claimed), and when.

Weave's `@locks` tracks: what *resources* are locked, by whom, for which task.

The relationship:
- When an agent runs `scud claim auth:1.2`, it creates an `@assignments` entry.
- When that agent's first `FileWrite` to `src/middleware/rate_limit.rs` passes through the coordinator, it creates an `@locks` entry for `file:src/middleware/rate_limit.rs`.
- When the agent runs `scud release auth:1.2`, the `@assignments` entry is removed AND all `@locks` held by that agent for that task are released.

This means `scud release` needs to be weave-aware: it should call `scud weave release-all --agent <agent> --task <task_id>` to clean up resource locks.

---

## Part III: Full SCG Example with Weave

```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
  updated 2026-02-06T10:30:00Z
}

@nodes
auth:1   | Design auth system      | X | 13 | H
auth:1.1 | Implement JWT tokens    | I | 5  | H | role=r:impl scope=src/auth/**
auth:1.2 | Add rate limiting       | P | 8  | M | role=r:impl scope=src/middleware/**
auth:1.3 | Write auth tests        | P | 5  | M | role=r:test scope=tests/auth/**
auth:1.4 | Document auth API       | P | 3  | L | role=r:docs scope=docs/auth.md
auth:2   | Rate limiting system    | P | 8  | M
auth:2.1 | Design rate limit algo  | P | 5  | H | role=r:impl scope=src/middleware/**

@edges
auth:1.1 -> auth:1
auth:1.2 -> auth:1
auth:1.3 -> auth:1
auth:1.4 -> auth:1
auth:2.1 -> auth:2

# Behavioral edges
auth:1.1 >> auth:1.3 | reason=tests-need-impl
auth:1.1 ~~ auth:1.2 | reason=shared-config-module
auth:1.1 != auth:1.4 | reason=different-specialization

@parents
auth:1: auth:1.1, auth:1.2, auth:1.3, auth:1.4
auth:2: auth:2.1

@assignments
auth:1.1 | agent-1 | agent-1 | 2026-02-06T10:00:00Z

@roles
r:impl | Implementer | src/** | docs/**
r:test | Test writer | tests/** | src/lib.rs
r:docs | Documenter  | docs/** *.md | src/**

@weave
w:1 | File mutex   | 5  | Y | Mutex      | kind=FileWrite key=file:{target}
w:2 | Test gate    | 10 | Y | Require    | trigger=Commit prereq=TestPass reset=FileWrite
w:3 | Schema lock  | 3  | Y | Mutex      | kind=SchemaChange key=schema-global
w:4 | Commit rate  | 50 | Y | RateLimit  | kind=Commit max=5 window=120
w:5 | Time budget  | 30 | Y | Timeout    | kind=TestRun max_secs=300
w:6 | No self-kill | 1  | Y | BlockAlways| kind=DangerousCommand

@locks
file:src/auth/jwt.rs | agent-1 | auth:1.1 | 2026-02-06T10:05:00Z | 3600

@details
auth:1 | description |
  Design the authentication system architecture.
  Must support OAuth2 and API keys.
auth:1.1 | description |
  Implement JWT token generation and validation.
  Use jsonwebtoken crate with RS256.
auth:1.1 | test_strategy |
  Unit tests for token generation, validation, and expiry.
w:2 | rules |
  type=Require trigger=Commit prereq=TestPass reset=FileWrite
  type=Require trigger=Commit prereq=LintPass reset=FileWrite
```

### What the Coordinator Sees

When `agent-2` attempts `FileWrite` to `src/auth/jwt.rs`:

1. **Check `@weave` w:1 (File mutex)**: `file:src/auth/jwt.rs` is in `@locks`, held by `agent-1`. → **BLOCKED**.
2. Response: `BLOCKED by "File mutex": agent-1 holds file:src/auth/jwt.rs (auth:1.1, since 10:05:00Z)`

When `agent-1` attempts `Commit`:

1. **Check `@weave` w:2 (Test gate)**: Last `FileWrite` at 10:05:00Z. Last `TestPass`? Not found since that write. → **WAIT**.
2. Response: `WAIT for "Test gate": TestPass required after FileWrite at 10:05:00Z`

When `agent-1` attempts `FileWrite` to `docs/auth.md`:

1. **Check `@roles`**: agent-1 is working auth:1.1 with `role=r:impl`. `r:impl` has `deny_pattern=docs/**`. → **BLOCKED**.
2. Response: `BLOCKED by role "Implementer": docs/** denied for this role`

---

## Part IV: Where State Lives

A key design question: what goes in the SCG file vs. separate files?

### In the SCG file (`.scud/tasks/tasks.scg`)
- `@weave`: B-thread definitions (low-churn, human-authored)
- `@roles`: Role definitions (low-churn, human-authored)
- `@partitions`: Partition definitions (low-churn, human-authored)
- `@locks`: Current mutex locks (medium-churn, coordinator-written)
- Extended `@nodes`: Role/scope annotations (low-churn, human-authored)
- Extended `@edges`: Behavioral edges (low-churn, human-authored)

**Rationale**: These are all part of the project's coordination specification. Keeping them in SCG makes `scud list`, `scud waves`, and `scud next-batch --weave` fast (one file read). The `@locks` section follows the precedent set by `@assignments` (which also puts runtime state in SCG).

### Separate file: Event log (`.scud/weave/events.jsonl`)
- High-churn runtime data (every `FileWrite`, `TestPass`, `Commit`, etc.)
- Rotated: coordinator keeps last N events for rule evaluation, archives older ones
- Not in SCG because event frequency would cause excessive git noise

### Separate file: Coordinator config (`.scud/weave/config.toml`)
- Global coordinator settings: default TTL, log rotation size, gate command timeout
- Hook configuration templates
- Not in SCG because it's project-level, not phase-level

```
.scud/
├── tasks/tasks.scg          # Existing + new @weave, @roles, @partitions, @locks sections
├── weave/
│   ├── config.toml           # Coordinator config (TTL, log rotation, etc.)
│   └── events.jsonl          # Event log (high-churn, rotated)
├── config.toml               # Existing scud config
├── active-tag                # Existing
├── current-task              # Existing
├── guidance/                 # Existing
│   └── *.md
└── logs/                     # Existing task logs
```

### The "Dual Source of Truth" Question

With b-threads defined both in SCG (`@weave`) and potentially in a separate `threads.toml`, which is authoritative?

**Resolution: SCG is the single source of truth for the active phase.** There is no `threads.toml`. B-threads live in `@weave` inside the SCG file, scoped to their phase. This means:
- `scud weave add` appends to the `@weave` section of the active phase's SCG
- `scud weave list` reads from the SCG
- `scud weave enable/disable` flips Y/N in the SCG
- Cross-phase b-threads (rare) can be defined in each phase's SCG independently

This eliminates sync issues between two files and keeps everything in one place. The SCG file was already the source of truth for tasks; now it's also the source of truth for behavioral constraints on those tasks.

---

## Part V: Implementation Plan

### Phase 1: Core Coordinator Engine (Rust, in scud-cli)

#### 1.1 Event Model

```rust
// src/weave/event.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Event {
    pub kind: EventKind,
    pub agent: Option<String>,
    pub target: Option<String>,       // file path, module, endpoint
    pub task_id: Option<String>,      // scud task ID
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventKind {
    FileWrite,
    FileCreate,
    DependencyAdd,
    DependencyRemove,
    SchemaChange,
    ApiChange,
    ConfigChange,
    TestRun,
    TestPass,
    TestFail,
    LintPass,
    LintFail,
    Commit,
    Build,
    TaskClaim,
    TaskComplete,
    DangerousCommand,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPattern {
    pub kind: Option<EventKind>,
    pub agent: Option<String>,
    pub target: Option<GlobPattern>,
    pub task_id: Option<String>,
    pub negate_agent: bool,           // match any agent EXCEPT this one
    pub target_not: Vec<GlobPattern>, // match any target NOT in this list
}
```

#### 1.2 Rule Types

```rust
// src/weave/bthread.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BThread {
    pub id: String,          // w:1, w:2, etc.
    pub name: String,
    pub priority: u32,       // lower = higher priority
    pub enabled: bool,
    pub rules: Vec<BThreadRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BThreadRule {
    Mutex {
        scope: EventPattern,
        key: String,             // template: "file:{target}", "schema-global"
        ttl_secs: Option<u64>,
    },
    Require {
        trigger: EventPattern,
        prerequisite: EventPattern,
        reset: Option<EventPattern>,
    },
    BlockUntil {
        trigger: EventPattern,
        block: Vec<EventPattern>,
        until: EventPattern,
        escalate: bool,
        escalation_message: Option<String>,
    },
    BlockAlways {
        scope: EventPattern,
    },
    RateLimit {
        scope: EventPattern,
        max: u32,
        window_secs: u64,
    },
    Timeout {
        scope: EventPattern,
        max_duration_secs: u64,
        action: TimeoutAction,     // Kill, Warn
    },
    Partition {
        scope: EventPattern,
        strategy: PartitionStrategy,  // Hash, RoundRobin, Directory
        agent_count: u32,
    },
}
```

#### 1.3 Coordinator

```rust
// src/weave/coordinator.rs

pub struct Coordinator {
    threads: Vec<BThread>,
    roles: Vec<Role>,
    partitions: Vec<PartitionDef>,
    active_locks: HashMap<String, ActiveLock>,
    event_log: Vec<TimestampedEvent>,        // from events.jsonl
}

#[derive(Debug)]
pub enum Decision {
    Proceed,
    Wait { reason: String, thread_id: String },
    Blocked { reason: String, thread_id: String },
}

impl Coordinator {
    /// Load from SCG file's @weave, @roles, @partitions, @locks sections
    /// plus events.jsonl for recent history.
    pub fn from_scg(scg: &ScgFile, events_path: &Path) -> Self { ... }

    /// Core evaluation: given a proposed event, check all b-threads.
    /// Checks in order: BlockAlways → Partition → Mutex → Require → BlockUntil → RateLimit → Timeout
    /// Returns first blocking decision found (by priority).
    pub fn evaluate(&self, event: &Event) -> Decision { ... }

    /// Record that an event occurred. Updates locks, writes to events.jsonl.
    pub fn record(&mut self, event: &Event) -> Result<()> { ... }

    /// Release a mutex lock.
    pub fn release(&mut self, lock_key: &str, agent: &str) -> Result<()> { ... }

    /// Release all locks held by an agent for a task.
    pub fn release_all(&mut self, agent: &str, task_id: &str) -> Result<()> { ... }

    /// Write @locks section back to SCG file.
    pub fn persist_locks(&self, scg: &mut ScgFile) -> Result<()> { ... }
}
```

#### 1.4 SCG Parser Extensions

Extend the existing SCG parser to handle new sections:

```rust
// In the existing SCG parser

pub struct ScgFile {
    pub meta: Meta,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,          // now includes ~~, >>, != types
    pub parents: Vec<ParentDef>,
    pub assignments: Vec<Assignment>,
    pub details: Vec<Detail>,
    // New:
    pub weave_threads: Vec<BThread>,
    pub roles: Vec<Role>,
    pub partitions: Vec<PartitionDef>,
    pub locks: Vec<ActiveLock>,
}

pub enum EdgeType {
    Dependency,    // ->
    Conflict,      // ~~
    Sequence,      // >>
    Exclusion,     // !=
}

pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub reason: Option<String>,
}
```

The parser must:
- Handle `@weave` lines: split by `|`, parse `rule_spec` as `key=value` pairs
- Handle `@roles` lines: split by `|`, parse patterns
- Handle `@partitions` lines: split by `|`
- Handle `@locks` lines: split by `|`, parse timestamps
- Handle extended `@nodes`: after 5th field, parse trailing `key=value` pairs
- Handle extended `@edges`: recognize `~~`, `>>`, `!=` operators; parse optional `| reason=...`
- **Ignore unknown sections gracefully** (existing behavior, but verify)

### Phase 2: CLI Interface

New subcommands under `scud weave`:

```
scud weave init                          # Add starter @weave section to active phase SCG
scud weave list                          # Show all b-threads with status
scud weave add <id> <name> <rule_type>   # Add a b-thread to @weave section
scud weave enable <id>                   # Flip N → Y
scud weave disable <id>                  # Flip Y → N
scud weave remove <id>                   # Remove from @weave section

scud weave check <event-json>            # Evaluate: would this event be allowed?
scud weave record <event-json>           # Record that an event occurred
scud weave release <lock-key> [agent]    # Release a specific mutex lock
scud weave release-all --agent <a> [--task <t>]  # Release all locks for agent/task
scud weave status                        # Show active locks, pending waits, partition map
scud weave log [--tail N]                # Show recent events from events.jsonl
scud weave explain <event-json>          # Explain WHY an event is blocked
scud weave summary                       # Generate orientation for fresh containers

scud weave template list                 # Show available b-thread templates
scud weave template apply <name>         # Add template b-thread to @weave
```

**Critical command: `scud weave check`**

```bash
$ scud weave check '{"kind":"FileWrite","agent":"agent-2","target":"src/auth/jwt.rs"}'
BLOCKED by w:1 "File mutex": agent-1 holds file:src/auth/jwt.rs (auth:1.1, 10:05Z)

$ scud weave check '{"kind":"Commit","agent":"agent-1"}'
WAIT for w:2 "Test gate": TestPass required after FileWrite at 10:05Z

$ scud weave check '{"kind":"FileWrite","agent":"agent-1","target":"src/middleware/rate.rs"}'
PROCEED
```

Output is always one line. Details in `scud weave explain`.

**`scud weave summary`** — generates orientation for fresh agent sessions:

```
$ scud weave summary
=== WEAVE STATUS (auth phase, 2026-02-06T10:30:00Z) ===
Active agents: agent-1 (auth:1.1, since 10:00Z)
Locks held: file:src/auth/jwt.rs (agent-1)
Recent events (last 5):
  10:05 agent-1 FileWrite src/auth/jwt.rs
  10:08 agent-1 TestRun
  10:09 agent-1 TestFail
  10:12 agent-1 FileWrite src/auth/jwt.rs
  10:15 agent-1 TestRun
Constraints: 6 active, 0 disabled
Roles defined: r:impl, r:test, r:docs
Available work (no conflicts): auth:2.1
```

### Phase 3: Claude Code Integration

#### 3.1 PreToolUse Hook

The highest-leverage integration point. Place in `.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "type": "command",
        "command": "scud weave gate --tool $TOOL_NAME --input '$TOOL_INPUT'",
        "timeout": 5000
      }
    ]
  }
}
```

The `scud weave gate` command:
1. Parses tool name + input to determine event type
2. Maps Write tool → FileWrite/FileCreate/SchemaChange based on path patterns
3. Maps Bash tool → parses command for git commit, cargo add, npm install, pkill, etc.
4. Calls coordinator evaluate()
5. Exit 0 = allow, Exit 2 = block (reason on stderr)

Mapping heuristics:
```
Write(src/db/migration_*.sql)   → SchemaChange
Write(src/api/routes.rs)        → FileWrite (+ ApiChange if pub fn sig changed)
Write(docs/*)                   → FileWrite
Bash("git commit ...")          → Commit
Bash("cargo add serde")        → DependencyAdd(target="serde")
Bash("cargo test")             → TestRun (allow, record start)
Bash("pkill ...")              → DangerousCommand
Bash("kill -9 ...")            → DangerousCommand
Bash("rm -rf /")               → DangerousCommand
```

#### 3.2 CLAUDE.md Integration

Add to the project's CLAUDE.md:

```markdown
## B-Thread Coordination (scud-weave)

This project uses behavioral coordination via scud-weave.

**If you hit a BLOCKED or WAIT from the PreToolUse hook:**
1. Read the reason. It will tell you which b-thread blocked you and why.
2. Run `scud weave status` to see what's locked.
3. Work on something else — run `scud next` to find unblocked work.
4. Do NOT try to bypass weave checks.

**After completing significant work:**
- After tests pass: `scud weave record '{"kind":"TestPass","agent":"YOUR_ID"}'`
- After lint passes: `scud weave record '{"kind":"LintPass","agent":"YOUR_ID"}'`
- When done with a file: `scud weave release file:<path> YOUR_ID`

**Orientation:** Run `scud weave summary` to see current project state.
```

#### 3.3 Enhanced `scud next-batch --weave`

Annotate ready tasks with behavioral constraints:

```
$ scud next-batch --limit 5 --weave
Ready tasks with behavioral constraints:
  auth:1.2  Add rate limiting         [WAIT: ~~ auth:1.1 (shared-config)]
  auth:1.3  Write auth tests          [WAIT: >> auth:1.1 (tests-need-impl)]
  auth:1.4  Document auth API         [no conflicts]
  auth:2.1  Design rate limit algo    [no conflicts]

Recommend parallel: auth:1.4 + auth:2.1 (no conflicts)
Waiting on: auth:1.1 completion (unblocks auth:1.2, auth:1.3)
```

This gives the operator (or lead agent) a dispatch plan that accounts for both DAG readiness and behavioral constraints.

### Phase 4: Backpressure Pattern Library

Templates included with `scud weave init` (all disabled by default):

```
$ scud weave template list
  commit-gate        Require tests+lint before commit
  file-mutex         Per-file mutual exclusion
  schema-singleton   Single-writer for migrations
  api-review-gate    Block builds after API changes until review
  dep-change-gate    Block after dependency changes until lockfile verified
  rate-limit-commits Prevent commit storms
  no-self-kill       Block pkill, kill -9, rm -rf, shutdown
  test-timeout       Kill test runs exceeding 5 minutes
  build-serializer   Serialize builds (only one at a time)

$ scud weave template apply commit-gate
Applied "commit-gate" → @weave in .scud/tasks/tasks.scg (disabled)
Enable with: scud weave enable w:N
```

---

## Part VI: Design Decisions

### Why `@weave` in SCG, not a separate `threads.toml`?

v1 of this design put b-threads in `.scud/weave/threads.toml`. v2 moves them into the SCG file. Reasons:

1. **Single file, single parse.** The coordinator already needs to read the SCG for task state. Reading b-threads from the same file avoids a second file open/parse.
2. **Phase-scoped.** B-threads often relate to specific tasks. Putting them in the same phase makes the relationship explicit. Different phases can have different constraints.
3. **One source of truth.** No sync between threads.toml and tasks.scg. No confusion about which is authoritative.
4. **Familiar mechanism.** `@details` already supports multiline content for complex rule definitions. No new file format to learn.
5. **git-visible.** Changes to b-threads show up in the same diff as task changes.

Tradeoff: The SCG file gets larger. But SCG is already designed for token efficiency, and b-thread definitions are compact (one line each in `@weave`).

### Why declarative rules, not imperative b-thread coroutines?

The original Harel model uses imperative coroutines (Java threads that yield at bSync points). For agent coordination, most constraints fall into seven patterns: Mutex, Require, BlockUntil, BlockAlways, RateLimit, Timeout, Partition. Declarative rules in `key=value` format are easier to author, faster to evaluate, and more predictable. If a user needs full imperative b-threads, they can write a custom `Custom` event type and a script.

### Why gate model, not event selection?

Harel's coordinator *selects* which event to trigger from the requested set. Agents don't submit menus of options — they attempt a specific action and need go/no-go. The coordinator is a gate, not a selector. Simpler, fits the actual workflow.

### Why resource locks (`@locks`) separate from task assignments (`@assignments`)?

`@assignments` answers: "who is working on task auth:1.1?" 
`@locks` answers: "who holds the write lock on src/auth/jwt.rs?"

One task may touch multiple files. Multiple tasks may need to coordinate on a shared resource (like a schema). These are different granularities that both need tracking.

### Interaction with Huntley's ralph loop

B-threads are compatible with both single-agent and multi-agent modes:
- **Single agent**: Coordinator acts as structured backpressure (commit-gate, test-requirement). More expressive than bare pre-commit hooks.
- **Multi agent**: Additionally provides inter-agent constraints (mutex, partition, exclusion).
- **In both cases**: Operator watches the loop, identifies failure domains, adds b-threads. The b-thread is the unit of "engineering away a failure mode."

---

## Part VII: Implementation Order

1. **SCG parser extensions** — Parse `@weave`, `@roles`, `@partitions`, `@locks`, extended `@nodes` and `@edges`
2. **Event model** (`weave/event.rs`, `weave/matcher.rs`) — Events, patterns, glob matching
3. **B-thread model** (`weave/bthread.rs`) — Thread definitions, all seven rule types
4. **Coordinator** (`weave/coordinator.rs`) — Evaluation logic, state management
5. **Event log** (`weave/events.rs`) — Read/write events.jsonl, rotation
6. **CLI: check, record, release, status** — Core coordination commands
7. **CLI: init, add, enable, disable, remove, list** — Thread management
8. **CLI: gate** — Claude Code PreToolUse hook integration
9. **CLI: explain** — Human-readable block explanations
10. **CLI: summary** — Fresh-container orientation
11. **CLI: template** — Built-in backpressure patterns
12. **Modify `scud next-batch`** — `--weave` flag for annotated output
13. **Modify `scud release`** — Weave-aware lock cleanup
14. **Tests** — Unit tests for coordinator, integration tests with SCG files
15. **Documentation** — CLAUDE.md snippets, hook setup guide

---

## Part VIII: Testing Strategy

### Unit Tests (Coordinator Logic)

| Test | Input | Expected |
|---|---|---|
| Mutex: same file | agent-1 holds lock, agent-2 writes same file | BLOCKED |
| Mutex: different files | agent-1 writes A, agent-2 writes B | PROCEED |
| Require: no prereq | Commit without prior TestPass | WAIT |
| Require: prereq then reset | TestPass, then FileWrite, then Commit | WAIT (reset) |
| Require: prereq valid | TestPass, then Commit (no FileWrite between) | PROCEED |
| RateLimit: under | 3 commits in 120s, attempt 4th | PROCEED (if under max) |
| RateLimit: over | 5 commits in 120s, attempt 6th | BLOCKED |
| BlockUntil: triggered | ApiChange occurred, attempt Build | BLOCKED |
| BlockUntil: resolved | ApiChange, then ApiReviewApproved, attempt Build | PROCEED |
| BlockAlways | DangerousCommand always blocked | BLOCKED |
| Partition: in partition | agent-0 writes file in partition slice 0 | PROCEED |
| Partition: out of partition | agent-0 writes file in partition slice 1 | BLOCKED |
| Timeout: expired | TestRun started 400s ago, max 300s | Timeout event emitted |
| Role: allowed | r:impl writes to src/** | PROCEED |
| Role: denied | r:impl writes to docs/** | BLOCKED |
| Conflict edge | Two tasks with ~~ edge, both claimed | Second BLOCKED |
| Priority: higher wins | Two threads, different priorities, one blocks | Higher-priority block wins |
| Disabled thread | Thread with enabled=N | Ignored |
| TTL expiry | Lock acquired 4000s ago, TTL 3600s | Lock auto-released |

### Integration Tests

- Parse full SCG with @weave → evaluate event → verify decision
- Record event → verify events.jsonl updated
- Record event → verify @locks section updated in SCG
- Full cycle: check → proceed → record → release → check again
- `scud weave gate` with mock tool input → correct event mapping
- `scud next-batch --weave` → correct constraint annotations
- `scud release` clears both @assignments and @locks
- Phase separation: b-threads in phase A don't affect phase B

### Property Tests

- No event is both PROCEED and BLOCKED for the same coordinator state
- Adding a b-thread can only restrict behavior (never allows previously blocked events)
- Removing a b-thread can only relax behavior
- Lock TTL expiry always releases (no permanent deadlocks)

---

## Part IX: Validation Rules for Extended SCG

Add to the existing SCG validation:

- **@weave IDs**: Must match `w:\d+`, unique within phase
- **@weave rule_spec**: Must be valid `key=value` pairs for the declared `rule_type`
- **@roles IDs**: Must match `r:\w+`, unique within phase
- **@roles patterns**: Must be valid globs
- **@partitions IDs**: Must match `p:\d+`, unique within phase
- **@partitions agent_count**: Must be > 0
- **@locks**: Timestamps must be valid ISO 8601, TTL > 0
- **Extended @nodes**: `role=` must reference existing `@roles` ID, `scope=` must be valid glob
- **Behavioral edges**: Both endpoints must be valid node IDs. `~~` is symmetric. `>>` and `!=` are ordered.
- **Edge reasons**: Max 100 chars, alphanumeric + `-_`
- **No cycles in `>>` edges**: Sequence edges combined with `->` edges must still form a DAG

---

## Part X: Success Criteria

1. `scud weave check` evaluates in **<5ms** (must not slow agent loops)
2. SCG files with @weave sections parse correctly by existing scud commands (backward compat)
3. SCG files without @weave sections work exactly as before (no weave overhead)
4. A file-mutex b-thread prevents two concurrent agents from writing the same file
5. A commit-gate b-thread blocks commits until tests pass
6. `scud weave explain` gives a human-readable reason for any block
7. Adding a new b-thread requires only appending to @weave section — zero code changes
8. PreToolUse hook integration gates tool use without agents needing explicit weave awareness
9. `scud next-batch --weave` annotates ready tasks with behavioral constraints
10. All coordinator logic is deterministic: same state + same event = same decision, always
11. `scud release` cleans up both @assignments and @locks atomically

---

## References

- **Harel, Marron & Weiss** — "Behavioral Programming" (CACM 2012): Foundational b-thread model.
- **Harel, Marron & Weiss** — "Programming Coordinated Behavior in Java" (ECOOP 2010): BPJ implementation.
- **Scud** — https://github.com/pyrex41/scud: Host system. SCG format spec, DAG execution, wave analysis, claim/release.
- **SCG Format Spec** — `docs/reference/SCG_FORMAT_SPEC.md` in scud repo: The format this document extends.
- **Claude Agent Teams** — https://code.claude.com/docs/en/agent-teams: Experimental multi-agent. Inbox messaging, shared task list.
- **Geoffrey Huntley** — "everything is a ralph loop" (ghuntley.com/loop): Monolithic loop philosophy.
- **Huntley/Moss** — "don't waste your backpressure" (ghuntley.com/pressure → banay.me): Backpressure discipline.
- **Carlini (Anthropic)** — "Building a C compiler in 2 weeks" (anthropic.com/engineering/building-c-compiler): 16-agent coordination at scale.
- **Nikolai Mushegian** — JAMS spec (nikolai.fyi/jams): Inspiration for SCG's pipe-separated format.
