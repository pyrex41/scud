# SCUD Graph Concepts

SCUD manages tasks as a **directed acyclic graph (DAG)**:

- **Nodes**: Tasks (status, complexity, priority)
- **Edges**: Dependencies (A must complete before B)
- **Parent-Child**: Subtask relationships

```
     ┌─────┐      ┌─────┐
     │  1  │      │  2  │     Wave 1 (no deps)
     └──┬──┘      └──┬──┘
        │            │
        ▼            ▼
     ┌─────┐      ┌─────┐
     │  3  │◄─────│  4  │     Wave 2
     └──┬──┘      └─────┘
        │
        ▼
     ┌─────┐
     │  5  │                  Wave 3
     └─────┘
```

## SCG Format

Tasks stored in `.scud/tasks/tasks.scg`:

```
@meta {
  name auth
}

@nodes
id | title | status | complexity | priority
auth:1 | Create user model | P | 3 | H
auth:2 | Auth middleware | I | 5 | H

@edges
auth:2 -> auth:1

@details
auth:1 | description |
  Create User model with email, password_hash
```

### Status Codes

| Code | Status |
|------|--------|
| P | Pending |
| I | InProgress |
| D | Done |
| B | Blocked |
| X | Expanded |

### Priority Codes

| Code | Priority |
|------|----------|
| H | High |
| M | Medium |
| L | Low |

## Waves

Computed via topological sort:

1. Tasks with no deps → **Wave 1**
2. Remove Wave 1, find next batch → **Wave 2**
3. Repeat until all assigned

```bash
scud waves

Wave 1: [1, 2]      # Run in parallel
Wave 2: [3, 4]      # After Wave 1
Wave 3: [5]         # After Wave 2
```

## Complexity (Fibonacci)

| Points | Size | Action |
|--------|------|--------|
| 1-3 | Small | Just do it |
| 5-8 | Medium | Plan first |
| 13+ | Large | **Must expand** |

Tasks ≥13 points should be broken down with `scud expand <id>`.

## Subtasks

When expanded, parent becomes status `X` (Expanded):

```
@nodes
auth:4 | Login flow | X | 13 | H
auth:4.1 | Login form | P | 3 | H
auth:4.2 | Login API | P | 5 | H

@parents
auth:4: auth:4.1, auth:4.2
```

Parent is done when all subtasks are done.
