# SCUD Graph Concepts

This document explains how SCUD manages tasks as a graph structure, including the SCG (SCUD Graph) format, dependencies, waves, and parallel execution.

## The Task Graph

SCUD represents tasks as a **directed acyclic graph (DAG)**:

- **Nodes**: Tasks (with status, complexity, priority)
- **Edges**: Dependencies (task A must complete before task B)
- **Parent-Child**: Subtask relationships (task expansion)

```
     ┌─────┐      ┌─────┐
     │  1  │      │  2  │     Wave 1 (no deps)
     └──┬──┘      └──┬──┘
        │            │
        ▼            ▼
     ┌─────┐      ┌─────┐
     │  3  │◄─────│  4  │     Wave 2 (depends on 1, 2)
     └──┬──┘      └─────┘
        │
        ▼
     ┌─────┐
     │  5  │                  Wave 3 (depends on 3)
     └─────┘
```

## SCG Format (SCUD Graph)

SCUD stores tasks in the **SCG format**, a token-efficient, human-readable format optimized for:
- Minimal file size (~60% smaller than JSON)
- Git-friendly diffs
- Human readability and manual editing
- Fast parsing

### File Structure

Tasks are stored in `.scud/tasks/tasks.scg`:

```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
}

@nodes
id | title | status | complexity | priority
auth:1 | Create user model | P | 3 | H
auth:2 | Auth middleware | I | 5 | H
auth:3 | Login endpoint | P | 5 | M
auth:4 | Registration | D | 5 | M

@edges
auth:3 -> auth:1
auth:3 -> auth:2
auth:4 -> auth:1

@parents
auth:3: auth:3.1, auth:3.2

@assignments
auth:2 | alice | alice | 2025-11-26T10:00:00Z

@details
auth:1 | description |
  Create User model with email, password_hash, created_at

auth:2 | test_strategy |
  Unit tests for middleware functions
  Integration tests for auth flow
```

### Section Reference

| Section | Purpose | Format |
|---------|---------|--------|
| `@meta` | Phase metadata | `name <tag>` |
| `@nodes` | Task definitions | `id \| title \| status \| complexity \| priority` |
| `@edges` | Dependencies | `child -> parent` |
| `@parents` | Subtask relationships | `parent: child1, child2` |
| `@assignments` | Who's working | `id \| assignee \| locker \| lock_time` |
| `@details` | Extended text | `id \| field_name \| <multiline>` |

### Status Codes

Single-letter codes for compact representation:

| Code | Status | Meaning |
|------|--------|---------|
| P | Pending | Not started |
| I | InProgress | Being worked on |
| D | Done | Completed |
| R | Review | Awaiting review |
| B | Blocked | Cannot proceed |
| F | Deferred | Postponed |
| C | Cancelled | Aborted |
| X | Expanded | Has subtasks |

### Priority Codes

| Code | Priority |
|------|----------|
| H | High |
| M | Medium |
| L | Low |

## Namespaced Task IDs

Task IDs are namespaced by tag to prevent collisions:

```
auth:1      # Task 1 in 'auth' tag
auth:1.1    # Subtask 1.1 of task 1 in 'auth'
api:1       # Task 1 in 'api' tag (different from auth:1)
```

This enables:
- Multiple tags in a single file
- Clear task ownership
- Cross-tag references (rare but possible)

## Dependencies

Dependencies define execution order:

```
@edges
auth:3 -> auth:1    # Task 3 depends on task 1
auth:3 -> auth:2    # Task 3 also depends on task 2
auth:4 -> auth:1    # Task 4 depends on task 1
```

### Dependency Rules

1. **No cycles**: A -> B -> A is invalid
2. **Within actionable tasks**: Only non-Done, non-Expanded tasks count
3. **Cross-tag possible**: `api:1 -> auth:2` works but is discouraged

### Checking Dependencies

```bash
scud show 3 --tag auth
# Shows: Dependencies: 1 (done), 2 (in-progress)

scud next --tag auth
# Only returns tasks whose dependencies are all Done
```

## Waves and Parallel Execution

**Waves** are computed using **Kahn's algorithm** (topological sort):

### Wave Computation

1. Find all tasks with no dependencies → **Wave 1**
2. Remove Wave 1 tasks from graph
3. Find all tasks with no remaining dependencies → **Wave 2**
4. Repeat until all tasks assigned

### Example

```
Tasks:
  1: No deps
  2: No deps
  3: Depends on 1
  4: Depends on 1, 2
  5: Depends on 3

Waves:
  Wave 1: [1, 2]      # Can run in parallel
  Wave 2: [3, 4]      # Can run after Wave 1
  Wave 3: [5]         # Can run after Wave 2
```

### Viewing Waves

```bash
scud waves --tag auth

Wave 1: (2 tasks)
  P 1 | Create user model [3]
  P 2 | Auth config [2]

Wave 2: (2 tasks)
  P 3 | Auth middleware <- 1 [5]
  P 4 | Registration <- 1,2 [5]

Wave 3: (1 task)
  P 5 | Login flow <- 3,4 [5]

Summary:
  Tasks: 5
  Waves: 3
  Speedup: 1.7x
```

### Max Parallel Batching

Use `--max-parallel N` to batch large waves:

```bash
scud waves --max-parallel 3

Wave 1, Round 1: [1, 2, 3]    # First 3 tasks
Wave 1, Round 2: [4, 5]       # Remaining 2 tasks
Wave 2, Round 1: [6, 7, 8]    # Next wave
```

### Speedup Calculation

```
Speedup = Total Tasks / Total Rounds

Example:
  10 tasks, 3 waves, max-parallel 5
  Wave 1: 6 tasks → 2 rounds
  Wave 2: 3 tasks → 1 round
  Wave 3: 1 task  → 1 round
  Total rounds: 4
  Speedup: 10/4 = 2.5x
```

## Subtasks (Task Expansion)

Complex tasks (≥13 points) should be expanded:

```
Before:
  Task 4: Implement login/logout [13 points] - status: Pending

After expansion:
  Task 4: Implement login/logout [13 points] - status: Expanded
    4.1: Login form component [3]
    4.2: Login API endpoint [3]
    4.3: Token generation [2]
    4.4: Logout endpoint [2]
    4.5: Auth state management [3]
```

### SCG Representation

```
@nodes
auth:4 | Implement login/logout | X | 13 | H
auth:4.1 | Login form component | P | 3 | H
auth:4.2 | Login API endpoint | P | 3 | H
auth:4.3 | Token generation | P | 2 | M
auth:4.4 | Logout endpoint | P | 2 | M
auth:4.5 | Auth state management | P | 3 | H

@parents
auth:4: auth:4.1, auth:4.2, auth:4.3, auth:4.4, auth:4.5
```

### Subtask Rules

1. Parent status becomes **Expanded** (X)
2. Parent excluded from wave computation
3. Subtasks become actionable items
4. Stats count subtasks, not parent
5. Parent is "done" when all subtasks are done

## Complexity (Fibonacci Points)

SCUD uses Fibonacci numbers for estimation:

| Points | Size | Action |
|--------|------|--------|
| 1 | Trivial | Config change |
| 2 | Tiny | One-liner fix |
| 3 | Small | Few functions |
| 5 | Medium | Multiple files |
| 8 | Large | Significant feature |
| 13 | Too big | **Must expand** |
| 21 | Way too big | **Must expand** |
| 34+ | Epic | **Must expand** |

### Why Fibonacci?

- Forces relative sizing (is this twice as hard or three times?)
- Gaps prevent false precision
- 8 vs 13 is a clear decision point
- Higher numbers signal "needs breakdown"

## File Locking

SCUD uses file-level locking for concurrent access:

### Lock Strategy

1. **Exclusive lock** for writes (one writer at a time)
2. **Shared lock** for reads (multiple readers OK)
3. **Exponential backoff** retry (up to 10 attempts)
4. **Atomic writes** prevent corruption

### Task-Level Locking

Separate from file locks, task claims prevent work conflicts:

```bash
scud claim 3 --name alice    # Sets locked_by, locked_at
scud release 3               # Clears lock
```

```
@assignments
auth:3 | alice | alice | 2025-11-26T10:30:00Z
#        ↑        ↑        ↑
#        │        │        └── Lock timestamp
#        │        └── Who locked (can be released)
#        └── Who's assigned (informational)
```

## Multiple Tags in One File

A single `tasks.scg` can contain multiple tags, separated by `---`:

```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
}

@nodes
auth:1 | Create user model | P | 3 | H

---

# Phase: api

@meta {
  name api
}

@nodes
api:1 | REST endpoints | P | 5 | H
api:2 | GraphQL schema | P | 8 | M
```

## Practical Examples

### Reading the Graph

```bash
# See all tasks
scud list --tag auth

# See dependency structure
scud waves --tag auth

# See specific task with dependencies
scud show 3 --tag auth
```

### Modifying the Graph

```bash
# Status changes update @nodes section
scud set-status 1 done --tag auth

# Claims update @assignments section
scud claim 2 --name alice --tag auth

# Expansion adds subtasks to @nodes and @parents
scud expand 4 --tag auth
```

### Diagnosing Issues

```bash
# Find problems
scud doctor --tag auth

# Common issues:
# - Circular dependencies (A -> B -> A)
# - Orphaned subtasks (parent deleted)
# - Stale locks (>24 hours old)
# - Missing dependency targets
```
