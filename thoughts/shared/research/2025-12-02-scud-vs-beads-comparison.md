---
date: 2025-12-02T00:00:00-08:00
researcher: pyrex41
git_commit: 28adba9a56134fe4c2be4c78d9b6160cdc473997
branch: master
repository: bmad-tm
topic: "Compare and Contrast SCUD vs Beads Task Management Systems"
tags: [research, codebase, scud, beads, task-management, comparison]
status: complete
last_updated: 2025-12-02
last_updated_by: pyrex41
---

# Research: SCUD vs Beads - Task Management Systems Comparison

**Date**: 2025-12-02
**Researcher**: pyrex41
**Git Commit**: 28adba9a56134fe4c2be4c78d9b6160cdc473997
**Branch**: master
**Repository**: bmad-tm

## Research Question

Compare and contrast the SCUD task management system (this codebase) with the Beads issue tracking system (steveyegge/beads) based on the provided documentation at beads.xml.

## Executive Summary

**SCUD** (Sprint Cycle Unified Development) and **Beads** are both task/issue management systems designed for AI-assisted software development workflows. While they share similar goals of enabling AI agents to manage development work, they take fundamentally different approaches:

| Aspect | SCUD | Beads |
|--------|------|-------|
| **Core Philosophy** | Phase-driven workflow orchestration | Git-native issue tracking |
| **Storage** | Flat files (SCG format) | SQLite database + JSONL export |
| **Sync Model** | File-based (direct read/write) | Daemon-based with auto-sync to git |
| **AI Integration** | Built-in LLM client for task generation | Optional MCP server wrapping CLI |
| **Scope** | Single-project workflow | Multi-repository coordination |
| **Implementation** | Rust CLI + TypeScript MCP | Go CLI + Python MCP |

---

## Detailed Comparison

### 1. Architecture & Philosophy

#### SCUD: Workflow Orchestration

SCUD is designed around a **5-phase structured workflow**:

1. **Ideation** - PRD creation (Product Manager agent)
2. **Planning** - Task breakdown (Scrum Master agent)
3. **Architecture** - Technical design (Architect agent)
4. **Implementation** - Task execution (Developer agent)
5. **Retrospective** - Learning capture

**Key insight**: SCUD's phases are a documented convention, not enforced by the CLI. The system trusts AI agents to follow the workflow rather than programmatically gating operations.

**File**: `/Users/reuben/bmad-tm/docs/guides/COMPLETE_GUIDE.md:27-47`

#### Beads: Git-Native Issue Tracking

Beads treats issues as first-class citizens in your git repository:

- Issues stored in SQLite database (`.beads/beads.db`)
- Automatic export to JSONL for git version control
- Daemon handles auto-sync with 5-second debounce
- Issues travel with the codebase across forks and clones

**Key insight**: Beads prioritizes data portability and git integration over workflow enforcement.

**From beads.xml lines 2612-2637**: The daemon and MCP server are thin layers - all heavy lifting (dependency graphs, collision resolution, merge logic) happens in the core storage layer.

---

### 2. Data Models

#### SCUD Task Model

```rust
// From /Users/reuben/bmad-tm/scud-cli/src/models/task.rs:69-115
pub struct Task {
    pub id: String,                    // Namespaced: "phase:local_id"
    pub title: String,                 // Max 200 chars
    pub description: String,           // Max 5000 chars
    pub status: TaskStatus,            // 8 statuses
    pub complexity: u32,               // Fibonacci: 0,1,2,3,5,8,13,21,34,55,89
    pub priority: Priority,            // High, Medium, Low
    pub dependencies: Vec<String>,     // Task IDs
    pub parent_id: Option<String>,     // For subtasks
    pub subtasks: Vec<String>,         // Child task IDs
    pub details: Option<String>,       // Implementation notes
    pub test_strategy: Option<String>, // Testing approach
    pub assigned_to: Option<String>,   // Developer assignment
    pub locked_by: Option<String>,     // Exclusive lock holder
    pub locked_at: Option<String>,     // Lock timestamp
}
```

**Task Statuses**: Pending, InProgress, Done, Review, Blocked, Deferred, Cancelled, **Expanded**

The **Expanded** status is unique to SCUD - it marks parent tasks that have been broken into subtasks.

#### Beads Issue Model

```json
// From beads.xml commands/create.md
{
  "id": "bd-a3f8e9",           // Hash-based ID (v0.20.1+)
  "title": "Issue title",
  "type": "bug|feature|task|epic|chore",
  "status": "open|in_progress|blocked|closed",
  "priority": 0-4,            // 0=critical, 4=backlog
  "description": "...",
  "design": "...",            // Technical notes
  "acceptance_criteria": "...",
  "labels": ["backend", "urgent"],
  "assignee": "developer"
}
```

**Issue Statuses**: open, in_progress, blocked, closed (4 vs SCUD's 8)

**Key Differences**:

| Feature | SCUD | Beads |
|---------|------|-------|
| ID Format | Sequential (1, 2, 3) or nested (1.1, 1.2) | Hash-based (bd-a3f8e9) |
| Complexity | Fibonacci scale (1-89) | Not tracked |
| Labels | Not supported | First-class feature |
| Templates | Not supported | Built-in and custom templates |
| Design Notes | `details` field | Separate `design` field |
| Test Strategy | Dedicated field | Part of acceptance criteria |

---

### 3. Dependency Systems

#### SCUD Dependencies

Dependencies in SCUD are:
- Stored as `Vec<String>` of task IDs
- Enforced during task selection (`find_next_task()` checks dependencies met)
- Validated for cycles using DFS (`would_create_cycle()`)

**From `/Users/reuben/bmad-tm/scud-cli/src/models/task.rs:396-436`**: Cycle detection prevents circular dependencies with full path reporting.

**Subtask Relationships**:
- `parent_id` field links subtasks to parents
- `subtasks` vector lists child IDs
- Expanded tasks marked with `TaskStatus::Expanded`
- Nested IDs like `TASK-1.1`, `TASK-1.2`

#### Beads Dependencies

Beads has a richer dependency system with **typed relationships**:

```bash
# From beads.xml commands/dep.md
bd dep add bd-10 bd-20 --type blocks    # bd-10 blocks bd-20
bd dep add bd-10 bd-20 --type related   # Soft relationship
bd dep add bd-10 bd-20 --type parent-child
bd dep add bd-10 bd-20 --type discovered-from
```

**Dependency Types**:
- **blocks** - Hard blocker (affects ready queue)
- **related** - Soft relationship (context only)
- **parent-child** - Epic/subtask hierarchy
- **discovered-from** - Work discovered during another task

**Visualization**: Beads can output Mermaid diagrams for dependency trees:
```bash
bd dep tree bd-1 --format mermaid
```

**Key Difference**: Beads distinguishes between blocking and non-blocking relationships; SCUD treats all dependencies as blockers.

---

### 4. Storage Systems

#### SCUD: SCG (SCUD Graph) Format

SCUD uses a custom text format optimized for token efficiency (~75% reduction vs JSON):

```
# SCUD Graph v1
# Phase: epic-1-auth

@meta {
  name epic-1-auth
  updated 2025-12-02T10:00:00Z
}

@nodes
1 | Create User model | P | 3 | H
2 | Build registration endpoint | P | 5 | H
1.1 | Add email validation | P | 2 | M

@edges
2 -> 1

@parents
1: 1.1

@assignments
2 | alice | alice | 2025-12-02T09:00:00Z

@details
1 | description |
  Create the User model with...
```

**File**: `/Users/reuben/bmad-tm/scud-cli/src/formats/scg.rs`

**Storage Location**: `.scud/tasks/tasks.scg`

**Multi-phase**: Phases separated by `---` delimiter in single file.

#### Beads: SQLite + JSONL

**SQLite** (`.beads/beads.db`): Source of truth
- Full relational model
- Supports custom tables and extensions
- File-level locking with fs2

**JSONL** (`.beads/issues.jsonl`): Git-friendly export
- One JSON object per line
- Sorted by ID for consistent diffs
- Auto-exported by daemon (5-second debounce)

**Hash-based IDs** (v0.20.1+):
- Content-based IDs prevent collisions
- Same ID across branches = same issue being updated
- Eliminates merge conflicts from concurrent creation

**From beads.xml lines 2482-2505**: ID collisions are eliminated - different issues get different hash IDs.

---

### 5. AI/LLM Integration

#### SCUD: Built-in AI Commands

SCUD has native LLM integration with three AI commands:

**1. parse-prd** - Parse PRD markdown into tasks
```bash
scud parse-prd docs/prd.md --tag epic-1
```

**2. analyze-complexity** - Score task complexity (Fibonacci)
```bash
scud analyze-complexity              # All tasks
scud analyze-complexity --task 5     # Single task
```

**3. expand** - Break complex tasks into subtasks
```bash
scud expand 5                        # Single task
scud expand --all                    # All high-complexity tasks
```

**Multi-Provider Support** (`/Users/reuben/bmad-tm/scud-cli/src/llm/client.rs`):
- Anthropic (`claude-sonnet-4-5-20250929`)
- xAI (`grok-code-fast-1`)
- OpenAI (`o3-mini`)
- OpenRouter
- Claude CLI (subprocess)

**Concurrent Processing**: Analyze and expand run with 5 concurrent LLM requests using `buffer_unordered()`.

#### Beads: Optional MCP Server

Beads separates AI integration into an optional MCP server:

**beads-mcp** (`integrations/beads-mcp/`):
- Python-based MCP server
- Wraps bd CLI commands
- No built-in LLM calls
- AI features depend on client (Claude Desktop)

**Agent Mail** (experimental):
- Real-time multi-agent coordination
- Eliminates git sync latency for status updates
- Prevents collision when agents claim issues simultaneously

**From beads.xml ADR 002**: Agent Mail reduces coordination latency from 2000-5000ms to ~50ms.

**Key Difference**: SCUD's AI is core to the system; Beads' AI integration is an optional layer.

---

### 6. Team Coordination

#### SCUD: Lock-Based Claiming

SCUD uses explicit locking for multi-developer coordination:

```bash
scud claim 5 --name alice        # Lock task for alice
scud release 5                   # Release lock (owner)
scud release 5 --force           # Admin override
scud whois                       # Show all assignments
scud sessions                    # Show active locks
```

**Lock Fields**:
- `assigned_to` - Soft assignment (informational)
- `locked_by` - Hard lock (enforced)
- `locked_at` - Lock timestamp for stale detection

**Auto-release**: Tasks auto-unlock when marked done.

**Stale Detection**: Locks >24 hours flagged as stale in `whois` output.

**File Locking**: Exclusive file locks with exponential backoff retry prevent race conditions.

#### Beads: Git-Based Sync

Beads relies on git for coordination with optional daemon:

**Daemon Mode** (`bd daemon`):
- Auto-sync to JSONL (5-second debounce)
- Optional `--auto-commit` and `--auto-push`
- Per-project daemon at `.beads/bd.sock`

**Without Daemon**:
- Direct CLI operations
- Manual `bd sync` for git operations

**Collision Prevention** (v0.20.1+):
- Hash-based IDs eliminate ID collisions
- Same issue = same ID = update not conflict

**Agent Mail** (experimental):
- Real-time lock announcements
- ~50ms latency vs 2-5 second git sync

---

### 7. Workflow & Phase Management

#### SCUD: 5-Phase Workflow (Convention)

**Workflow State** (`.scud/workflow-state.json`):
```json
{
  "current_phase": "implementation",
  "phases": {
    "ideation": { "status": "completed" },
    "planning": { "status": "completed" },
    "architecture": { "status": "completed" },
    "implementation": { "status": "active" },
    "retrospective": { "status": "pending" }
  }
}
```

**Important Finding**: The workflow state file is NOT read by the CLI. Phase enforcement is convention-based through agent documentation.

**Agent System**:
- `/tm-pm` - Product Manager (Ideation, Planning)
- `/tm-sm` - Scrum Master (Task breakdown)
- `/tm-architect` - Architect (Technical design)
- `/tm-dev` - Developer (Implementation)
- `/tm-retrospective` - Facilitator (Learning capture)

#### Beads: No Workflow Enforcement

Beads has no concept of workflow phases:
- Issues move through statuses (open → in_progress → closed)
- No prescribed order of operations
- Agents determine their own workflow
- `bd ready` shows unblocked issues regardless of order

---

### 8. MCP Server Comparison

#### SCUD MCP (`scud-mcp/`)

**Implementation**: TypeScript wrapping Rust CLI
**Tools**: 20 MCP tools covering:
- Core: init, list, next, stats, show, set-status
- Epic: tags, use-tag
- AI: parse-prd, analyze-complexity, expand, research
- Parallel: create-group, list-groups, group-status, assign, claim, release, whois

**Resources**:
- `scud://workflow/state`
- `scud://tasks/list`
- `scud://stats/epic`

#### Beads MCP (`integrations/beads-mcp/`)

**Implementation**: Python wrapping Go CLI
**Tools**: Core issue operations + mail tools
- Issue CRUD: create, list, show, update, close
- Dependencies: dep add/remove/tree
- Ready queue: ready, blocked
- Mail: send, receive (Agent Mail)

**Daemon Integration**: MCP server can route to per-project daemons.

---

### 9. Unique Features

#### SCUD-Only Features

1. **AI-Powered Task Generation**
   - Parse PRDs into structured tasks
   - Automatic complexity scoring
   - Intelligent task expansion

2. **SCG Format**
   - ~75% token reduction
   - Graph-native structure
   - Human-readable

3. **Complexity Scale**
   - Fibonacci scoring (1-89)
   - Expansion recommendations
   - Subtask count guidance

4. **Hook Integration**
   - Claude Code stop hooks
   - Auto-prompts for task completion

5. **Warmup Command**
   - Session orientation
   - Stale lock detection
   - Next task preview

#### Beads-Only Features

1. **Hash-Based IDs**
   - Content-addressable
   - No collision resolution needed
   - Stable across branches

2. **Typed Dependencies**
   - blocks, related, parent-child, discovered-from
   - Soft vs hard relationships
   - Discovery tracking

3. **Issue Templates**
   - Built-in: epic, bug, feature
   - Custom templates in `.beads/templates/`
   - YAML-based

4. **Labels**
   - Cross-cutting metadata
   - Filtering and organization
   - Team conventions

5. **Compaction**
   - Semantic summarization of old issues
   - Tiered compression (30-day, 90-day)
   - Database size management

6. **Agent Mail**
   - Real-time multi-agent coordination
   - Sub-50ms latency
   - Channel-based communication

7. **Mermaid Diagrams**
   - Dependency tree visualization
   - GitHub/GitLab rendering
   - Status indicators in nodes

8. **Merge Tools**
   - Intelligent JSONL conflict resolution
   - beads-merge external tool

---

### 10. Use Case Recommendations

#### Choose SCUD When:

- You want **structured workflow guidance**
- You need **AI-powered task generation** from PRDs
- Your team uses **complexity estimation** (Fibonacci)
- You work on **single projects** with clear phases
- You want **agent-specific documentation** per phase
- **Token efficiency** matters (SCG format)

#### Choose Beads When:

- You need **git-native issue tracking**
- You work across **multiple repositories**
- You want **typed dependency relationships**
- You need **issue templates** and **labels**
- **Multi-agent coordination** is critical (Agent Mail)
- **Long-term projects** need issue compaction
- You want **database extensibility** (SQLite)

---

## Code References

### SCUD Core Files
- `/Users/reuben/bmad-tm/scud-cli/src/models/task.rs` - Task data model
- `/Users/reuben/bmad-tm/scud-cli/src/models/phase.rs` - Phase/Epic container
- `/Users/reuben/bmad-tm/scud-cli/src/storage/mod.rs` - Storage layer
- `/Users/reuben/bmad-tm/scud-cli/src/formats/scg.rs` - SCG format parser
- `/Users/reuben/bmad-tm/scud-cli/src/llm/client.rs` - LLM client
- `/Users/reuben/bmad-tm/scud-cli/src/commands/ai/` - AI commands
- `/Users/reuben/bmad-tm/scud-mcp/src/index.ts` - MCP server

### Beads (from beads.xml)
- `commands/*.md` - CLI command documentation
- `docs/ARCHITECTURE.md` - System architecture
- `docs/ADVANCED.md` - Advanced features
- `integrations/beads-mcp/` - MCP server (Python)
- `lib/beads_mail_adapter.py` - Agent Mail integration

## Summary Comparison Table

| Feature | SCUD | Beads |
|---------|------|-------|
| Language | Rust CLI + TypeScript MCP | Go CLI + Python MCP |
| Storage | SCG flat files | SQLite + JSONL |
| ID System | Sequential/nested | Hash-based |
| Workflow | 5-phase (convention) | Status-based (open/closed) |
| Dependencies | Blocking only | Typed (blocks/related/parent-child) |
| AI Integration | Built-in (parse, analyze, expand) | Optional MCP layer |
| Complexity | Fibonacci scale | Not tracked |
| Templates | Not supported | Built-in + custom |
| Labels | Not supported | First-class |
| Multi-repo | Single project | Multi-repository |
| Sync Model | File-based | Daemon + git sync |
| Agent Coordination | Lock-based claiming | Git sync + Agent Mail |
| Compaction | Not supported | Tiered summarization |

## Open Questions

1. Could SCUD benefit from hash-based IDs for multi-user scenarios?
2. Would typed dependencies (blocking vs related) improve SCUD's task modeling?
3. Should SCUD add label/template support?
4. Could Beads adopt workflow phases as optional structure?
5. Would SCUD benefit from a daemon model for background sync?
