---
date: 2025-11-27T10:18:49Z
researcher: reuben
git_commit: 6e8a23b5b054336495c7adda2751453f7d932943
branch: master
repository: bmad-tm
topic: "SCUD vs Beads: Comparative Analysis of AI Task Management Systems"
tags: [research, codebase, comparison, scud, beads, task-management, ai-agents]
status: complete
last_updated: 2025-11-27
last_updated_by: reuben
---

# Research: SCUD vs Beads - Comparative Analysis

**Date**: 2025-11-27T10:18:49Z
**Researcher**: reuben
**Git Commit**: 6e8a23b5b054336495c7adda2751453f7d932943
**Branch**: master
**Repository**: bmad-tm

## Research Question

Compare and contrast the SCUD task management system with the Beads project to understand their architectural differences, shared concepts, and unique features.

## Summary

**SCUD** (Sprint Cycle Unified Development) and **Beads** (`bd`) are both task management systems designed for AI-supervised coding workflows, but they take fundamentally different approaches:

| Aspect | SCUD | Beads |
|--------|------|-------|
| **Philosophy** | Phase-gated workflow with agent personas | Flexible issue tracker with dependency graphs |
| **Storage** | SCG format + JSON mirror (file-based) | SQLite + JSONL (git-synced) |
| **Implementation** | Rust CLI + Node.js wrapper | Go CLI |
| **Workflow Model** | 5-phase waterfall (ideation→retrospective) | Free-form with blocking dependencies |
| **Parallelization** | Wave computation + task locking | Optional via MCP Agent Mail |
| **ID System** | Namespaced (epic:task_id) | Hash-based (collision-resistant) |

Both share a core design philosophy: **tasks are first-class citizens managed by AI agents through CLI commands and slash commands**.

---

## Detailed Findings

### 1. Core Data Models

#### SCUD Task Model
**Location**: `/Users/reuben/bmad-tm/scud-cli/src/models/task.rs:69-115`

```rust
struct Task {
    id: String,                    // Format: "epic:local_id" (e.g., "phase1:10.1")
    title: String,                 // Max 200 chars
    description: String,           // Max 5000 chars
    status: TaskStatus,            // pending|in-progress|done|review|blocked|deferred|cancelled|expanded
    complexity: u32,               // Fibonacci: 0,1,2,3,5,8,13,21,34,55,89
    priority: Priority,            // high|medium|low
    dependencies: Vec<String>,     // Task IDs this depends on
    parent_id: Option<String>,     // For subtasks
    subtasks: Vec<String>,         // Child task IDs
    details: Option<String>,       // Implementation details
    test_strategy: Option<String>, // Testing approach
    assigned_to: Option<String>,   // Developer assignment
    locked_by: Option<String>,     // Exclusive claim
    locked_at: Option<String>,     // Lock timestamp
    created_at: Option<String>,
    updated_at: Option<String>,
}
```

**Key Characteristics**:
- Namespaced IDs (epic tag + local ID separated by `:`)
- Fibonacci complexity for estimation
- Parent-child relationships for task expansion
- Lock fields for parallel execution support

#### Beads Issue Model
**Derived from CLI commands and documentation**

```go
type Issue struct {
    ID          string    // Hash-based (e.g., "bd-a3f8e9")
    Title       string
    Description string
    Status      string    // open|in_progress|blocked|closed
    Priority    int       // 0-4 (0=critical, 4=backlog)
    Type        string    // bug|feature|task|epic|chore
    Assignee    string
    Labels      []string
    Design      string    // Mutable design notes
    Notes       string    // Session handoff notes
    AcceptanceCriteria string
    CreatedAt   time.Time
    UpdatedAt   time.Time
    ClosedAt    time.Time
}
```

**Key Characteristics**:
- Hash-based collision-resistant IDs
- Numeric priority (more granular than SCUD)
- Separate fields for design notes and acceptance criteria
- Labels for cross-cutting metadata

#### Comparison: Data Model

| Feature | SCUD | Beads |
|---------|------|-------|
| ID Format | `epic:task_id` namespaced | Hash-based (`bd-a3f2`) |
| Status Values | 8 states incl. `expanded`, `review` | 4 states |
| Complexity | Fibonacci numbers | No built-in (uses priority) |
| Priority | 3 levels (high/medium/low) | 5 levels (0-4) |
| Subtasks | Built-in parent-child | Via `parent-child` dependency type |
| Locking | Built-in `locked_by`/`locked_at` | Via Agent Mail (optional) |
| Design Notes | In `details` field | Dedicated `design` field |
| Labels | Not supported | First-class feature |
| Issue Types | Implicit via epic tags | Explicit (bug/feature/task/epic/chore) |

---

### 2. Storage Mechanisms

#### SCUD Storage
**Location**: `/Users/reuben/bmad-tm/scud-cli/src/storage/mod.rs`

```
.scud/
├── tasks/
│   ├── tasks.scg          # Primary storage (SCG format)
│   └── tasks.json         # Mirror for Node.js tooling
├── workflow-state.json    # Workflow phase tracking
└── config.toml            # LLM provider settings
```

**Characteristics**:
- **SCG Format**: Custom text format, multi-phase separated by `\n---\n`
- **File Locking**: Uses `fs2` crate with exponential backoff (line 32)
- **Dual Storage**: SCG for Rust, JSON mirror for Node.js compatibility
- **No Git Integration**: File-based only, no automatic sync

#### Beads Storage
**Three-tier architecture**

```
.beads/
├── beads.db               # SQLite (gitignored, fast queries)
├── beads.jsonl            # JSONL (git-tracked, one entity per line)
├── bd.sock                # Daemon socket (Unix)
└── config.yaml            # Tool configuration
```

**Characteristics**:
- **SQLite**: Fast local queries with indexes (gitignored)
- **JSONL**: Git-friendly, merge-friendly (one line per issue)
- **Automatic Sync**: SQLite ↔ JSONL with 5-second debounce
- **Git Integration**: Auto-commit, auto-push via daemon
- **Per-Workspace Daemon**: LSP-like architecture for isolation

#### Comparison: Storage

| Aspect | SCUD | Beads |
|--------|------|-------|
| Primary Format | SCG (custom) | SQLite |
| Git Format | N/A | JSONL |
| Auto Git Sync | No | Yes (daemon) |
| File Locking | Yes (fs2) | Yes (daemon RPC) |
| Multi-Machine | Manual | Automatic via git |
| Daemon | No | Yes (per-workspace) |

---

### 3. CLI Commands

#### SCUD Commands
**Entry**: `/Users/reuben/bmad-tm/bin/scud.js`

**Core Commands**:
- `scud init` - Initialize .scud directory
- `scud tags [<tag>]` - List/switch epic tags
- `scud list [-s status] [-t tag]` - List tasks
- `scud show <task_id>` - Show task details
- `scud set-status <task_id> <status>` - Update status
- `scud next [--claim --name <agent>]` - Find next available task
- `scud stats` - Task statistics
- `scud waves [-n max_parallel]` - Compute parallel execution waves
- `scud mermaid` - Generate dependency diagram

**Assignment Commands**:
- `scud assign <task_id> <assignee>` - Soft assignment
- `scud claim <task_id> --name <agent>` - Hard lock
- `scud release <task_id>` - Release lock
- `scud whois` - Show assignments

**AI Commands** (require API key):
- `scud parse-prd <file> --tag <tag>` - Parse PRD into tasks
- `scud analyze-complexity` - AI complexity analysis
- `scud expand <task_id>` - Expand into subtasks

#### Beads Commands
**From CLI Reference in repomix**

**Core Commands**:
- `bd init [prefix]` - Initialize beads
- `bd list [filters]` - List issues with filters
- `bd show <issue_id>` - Show issue details
- `bd create "title" -t type -p priority` - Create issue
- `bd update <id> --status in_progress` - Update fields
- `bd close <id> --reason "Done"` - Close issue
- `bd ready` - Find unblocked work

**Dependency Commands**:
- `bd dep add <from> <to> --type blocks|related|parent-child|discovered-from`
- `bd dep tree <id>` - Show dependency tree
- `bd dep cycles` - Detect circular dependencies
- `bd blocked` - Show blocked issues

**Git/Sync Commands**:
- `bd sync` - Git push/pull sync
- `bd export` - Export to JSONL
- `bd import` - Import from JSONL
- `bd daemon --start|--stop|--status` - Manage daemon

**Advanced Commands**:
- `bd compact` - Compress old closed issues
- `bd merge <ids> --into <target>` - Merge duplicates
- `bd template` - Manage issue templates
- `bd prime` - Load AI context (~1-2k tokens)

#### Comparison: Commands

| Feature | SCUD | Beads |
|---------|------|-------|
| Task Creation | Via `parse-prd` or manual | `bd create` |
| Wave Planning | `scud waves` built-in | Not built-in |
| Dependency Viz | `scud mermaid` | `bd dep tree --format mermaid` |
| Git Sync | Manual | `bd sync`, daemon auto-sync |
| Context Injection | N/A | `bd prime` |
| Issue Templates | N/A | `bd template` |
| Compaction | N/A | `bd compact` |

---

### 4. Workflow Models

#### SCUD: 5-Phase Gated Workflow
**Location**: `/Users/reuben/bmad-tm/scud-cli/src/models/workflow.rs:5-46`

```
Ideation → Planning → Architecture → Implementation → Retrospective
   PM        SM         Architect        Developer      Retrospective
```

**Characteristics**:
- **Phase Gates**: Agents can only operate in their allowed phases
- **Agent Personas**: PM, SM, Architect, Developer, Retrospective
- **Strict Progression**: Must complete phase to advance
- **Epic-Centric**: Tasks grouped by epic tags

**Phase Gate Rules** (from slash commands):
- PM: Only `ideation`, `planning`
- SM: Only `planning`
- Architect: Only `architecture` + requires active epic
- Developer: Only `implementation` + requires active epic
- Retrospective: All tasks in epic must be done

#### Beads: Dependency-Based Workflow
**From documentation**

```
ready → create → update → close → sync
   ↓
 dep add (discovered-from)
   ↓
 ready (newly unblocked)
```

**Characteristics**:
- **No Phase Gates**: Any status transition allowed
- **Dependency-Driven**: `bd ready` shows unblocked work
- **Discovery Flow**: `discovered-from` links for yak shaving
- **Flexible Hierarchy**: Epics via `parent-child` dependencies

#### Comparison: Workflow

| Aspect | SCUD | Beads |
|--------|------|-------|
| Model | Waterfall phases | Dependency graph |
| Phases | 5 explicit phases | None (status only) |
| Agent Roles | Strict personas (PM/SM/Arch/Dev) | None (flexible) |
| Blocking | Dependencies must be `done` | Dependencies must be `closed` |
| Discovery | Creates subtasks | `discovered-from` links |
| Progression | Phase gates enforce order | Dependency resolution |

---

### 5. Parallel Execution Support

#### SCUD: Wave Computation
**Location**: `/Users/reuben/bmad-tm/scud-cli/src/commands/waves.rs`

**Algorithm**: Kahn's algorithm (topological sort with level assignment)

```
Wave 1: Tasks with no dependencies (in-degree = 0)
Wave 2: Tasks depending only on Wave 1
Wave 3: Tasks depending only on Waves 1-2
...
```

**Features**:
- `--max-parallel N` batches waves into rounds
- Speedup calculation: `total_tasks / total_rounds`
- Blocked task reporting (circular dependencies, missing deps)
- Dynamic claiming: `scud next --claim --name <agent>`

**Task Locking** (`/Users/reuben/bmad-tm/scud-cli/src/models/task.rs:334-395`):
- `claim()` - Hard lock with name
- `release()` - Release lock
- `is_stale_lock()` - Check for abandoned locks
- `lock_age_hours()` - Calculate lock duration

#### Beads: Agent Mail (Optional)
**From ADR 002 in repomix**

**MCP Agent Mail Integration** (optional):
- File reservation system prevents collisions
- <100ms latency vs 2-5s git sync
- Graceful degradation to git-only mode

**Without Agent Mail**:
- Last writer wins (git-based)
- No built-in wave computation
- Manual coordination required

#### Comparison: Parallelization

| Aspect | SCUD | Beads |
|--------|------|-------|
| Wave Planning | Built-in Kahn's algorithm | Not built-in |
| Task Locking | Native `locked_by` field | Via Agent Mail (optional) |
| Collision Prevention | Task claims | Hash IDs + Agent Mail |
| Multi-Agent | `scud next --claim` | Agent Mail reservations |
| Latency | Instant (file-based) | <100ms (Agent Mail) or 2-5s (git) |

---

### 6. AI Integration

#### SCUD: Slash Commands + Prompts
**Location**: `/Users/reuben/bmad-tm/.claude/commands/scud/`

**Agent Slash Commands**:
- `/scud:pm` - Product Manager persona
- `/scud:sm` - Scrum Master persona
- `/scud:architect` - Architect persona
- `/scud:dev` - Developer persona
- `/scud:retrospective` - Retrospective persona
- `/scud:status` - Workflow status

**Task Commands**:
- `/scud:task-list`, `/scud:task-show`, `/scud:task-status`
- `/scud:task-claim`, `/scud:task-next`, `/scud:task-waves`

**LLM Integration**:
- `scud parse-prd` - AI parses PRD markdown
- `scud analyze-complexity` - AI estimates task complexity
- `scud expand` - AI generates subtasks

#### Beads: Context Efficiency Strategy
**Three-tier integration**:

1. **CLI + Hooks** (recommended, ~1-2k tokens)
   - `bd prime` injects workflow context
   - SessionStart/PreCompact hooks auto-refresh

2. **Plugin** (optional, enhanced UX)
   - Slash commands: `/bd-ready`, `/bd-create`, etc.
   - Task agent (`@task-agent`)
   - MCP tools

3. **MCP Server** (fallback, ~10-50k tokens)
   - For MCP-only environments (Claude Desktop)
   - 10+ tools: init, create, list, ready, show, update, close, dep, blocked, stats

#### Comparison: AI Integration

| Aspect | SCUD | Beads |
|--------|------|-------|
| Context Size | Varies (full slash command prompts) | ~1-2k tokens (bd prime) |
| Slash Commands | Agent personas + task commands | Task commands only |
| MCP Server | Optional (scud-mcp) | Built-in (beads-mcp) |
| LLM Features | PRD parsing, complexity, expansion | None built-in |
| Hook Integration | Via slash commands | Native hooks system |
| Multi-Editor | Claude Code focused | Universal (any shell) |

---

### 7. Unique Features

#### SCUD-Only Features

1. **Phase-Gated Workflow** - Enforces development process order
2. **Agent Personas** - PM, SM, Architect, Developer roles with specific capabilities
3. **Fibonacci Complexity** - Built-in estimation system
4. **Wave Computation** - Parallel execution planning with `scud waves`
5. **PRD Parsing** - AI extracts tasks from product requirements
6. **Task Expansion** - AI generates subtasks for complex tasks
7. **Complexity Analysis** - AI estimates task complexity
8. **HTML Viewer** - `scud view` generates interactive task browser

#### Beads-Only Features

1. **Git-Native Sync** - Automatic JSONL export and git integration
2. **Hash-Based IDs** - Collision-resistant distributed ID generation
3. **Daemon Architecture** - Per-workspace background process for sync
4. **Issue Templates** - Predefined structures for bugs, features, epics
5. **Compaction** - Semantic compression of old closed issues
6. **Labels** - Cross-cutting metadata for filtering
7. **Multiple Dependency Types** - blocks, related, parent-child, discovered-from
8. **Mermaid Dependency Trees** - Visual dependency graphs
9. **Context Efficiency** - `bd prime` for minimal token usage
10. **Agent Mail** - Optional real-time multi-agent coordination

---

### 8. Architecture Patterns

#### SCUD Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Claude Code                        │
│  ┌─────────────────────────────────────────────┐    │
│  │  Slash Commands (.claude/commands/scud/)     │    │
│  │  /scud:pm, /scud:dev, /scud:task-*           │    │
│  └──────────────────────┬──────────────────────┘    │
└─────────────────────────┼───────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────┐
│              Node.js Wrapper (bin/scud.js)           │
│  Routes to Rust CLI or Node.js handlers              │
└──────────────────────────┬──────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────┐
│              Rust CLI (scud-cli/)                    │
│  Commands, Models, Storage, LLM Client               │
└──────────────────────────┬──────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────┐
│              File Storage (.scud/)                   │
│  tasks.scg + tasks.json + workflow-state.json        │
└─────────────────────────────────────────────────────┘
```

#### Beads Architecture

```
┌─────────────────────────────────────────────────────┐
│           AI Coding Tool (Claude/Cursor/etc)         │
│  ┌──────────────────┐  ┌────────────────────────┐   │
│  │   bd CLI         │  │   MCP Server (optional) │   │
│  │   (bd prime)     │  │   (10+ tools)           │   │
│  └────────┬─────────┘  └───────────┬────────────┘   │
└───────────┼────────────────────────┼────────────────┘
            ↓                        ↓
┌─────────────────────────────────────────────────────┐
│              Per-Workspace Daemon                    │
│  RPC Server, Auto-flush, Git Sync                    │
└──────────────────────────┬──────────────────────────┘
                          ↓
    ┌─────────────────────┴─────────────────────┐
    ↓                                           ↓
┌──────────────────┐                ┌────────────────────┐
│  SQLite (.beads/ │                │  JSONL (.beads/    │
│  beads.db)       │  ←── sync ───→ │  beads.jsonl)      │
│  (fast queries)  │                │  (git-tracked)     │
└──────────────────┘                └──────────┬─────────┘
                                              ↓
                                    ┌────────────────────┐
                                    │  Git Remote        │
                                    │  (shared state)    │
                                    └────────────────────┘
```

---

## Code References

### SCUD
- `/Users/reuben/bmad-tm/scud-cli/src/models/task.rs:69-115` - Task data model
- `/Users/reuben/bmad-tm/scud-cli/src/models/workflow.rs:5-46` - Workflow phases
- `/Users/reuben/bmad-tm/scud-cli/src/storage/mod.rs` - File storage with locking
- `/Users/reuben/bmad-tm/scud-cli/src/commands/waves.rs` - Wave computation algorithm
- `/Users/reuben/bmad-tm/scud-cli/src/main.rs:67-297` - CLI command definitions
- `/Users/reuben/bmad-tm/bin/scud.js` - Node.js CLI wrapper
- `/Users/reuben/bmad-tm/.claude/commands/scud/` - Slash commands

### Beads (from repomix-output.xml)
- `docs/ARCHITECTURE.md:5155-5430` - Three-layer data model
- `docs/DAEMON.md:6863-7162` - Per-workspace daemon design
- `docs/CLI_REFERENCE.md:5713-6112` - Command reference
- `docs/CONFIG.md:6425-6724` - Configuration system
- `docs/CLAUDE_INTEGRATION.md:5501-5576` - AI integration strategy
- `docs/adr/002-agent-mail-integration.md:1784-2017` - Multi-agent coordination

---

## Architectural Patterns Compared

### SCUD's Strengths
1. **Structured Process**: Phase gates enforce development discipline
2. **Parallel Planning**: Built-in wave computation optimizes team coordination
3. **AI-Powered**: PRD parsing and task expansion automate planning
4. **Rich Task Model**: Complexity, subtasks, test strategy built-in

### Beads' Strengths
1. **Git-Native**: Seamless multi-machine sync without manual intervention
2. **Collision-Resistant**: Hash IDs work in distributed workflows
3. **Context Efficient**: `bd prime` uses 10-50x less tokens than MCP
4. **Editor Agnostic**: Works with any shell-enabled editor
5. **Lightweight**: Minimal overhead, graceful degradation

### When to Use Each

**Choose SCUD when**:
- Following a structured development process
- Need phase gates to enforce workflow
- Want AI-powered PRD parsing and task expansion
- Working on a single machine or repo
- Need wave-based parallel execution planning

**Choose Beads when**:
- Working across multiple machines
- Need git-based team collaboration
- Want minimal AI token overhead
- Need flexible dependency-based workflow
- Require collision-resistant distributed IDs
- Working with multiple editors (not just Claude Code)

---

## Open Questions

1. **Could SCUD benefit from git-based sync?** - Currently file-only, manual coordination needed for teams
2. **Could Beads benefit from wave computation?** - Dependency graph exists, algorithm could be added
3. **Interoperability?** - Could tasks be translated between SCUD and Beads formats?
4. **Phase gates in Beads?** - Could labels + workflow rules approximate SCUD's phase model?

---

## Related Research

- This is the first research document in this repository comparing task management approaches.
