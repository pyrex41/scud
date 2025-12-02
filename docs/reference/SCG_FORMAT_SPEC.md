# SCG (SCUD Graph) Format Specification v1

A token-efficient, human-readable format for representing task dependency graphs.

## Overview

SCG is a text-based format that represents tasks as a Directed Acyclic Graph (DAG). It achieves ~75% token reduction compared to JSON while remaining human-editable and git-friendly.

**Storage**: `.scud/tasks/tasks.scg`

## File Structure

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

## Status Codes

| Code | Status | Description |
|------|--------|-------------|
| `P` | Pending | Not started |
| `I` | InProgress | Being worked on |
| `D` | Done | Completed |
| `R` | Review | Awaiting review |
| `B` | Blocked | Cannot proceed |
| `F` | Deferred | Postponed |
| `C` | Cancelled | Aborted |
| `X` | Expanded | Has subtasks (excluded from waves) |

## Priority Codes

| Code | Priority |
|------|----------|
| `H` | High |
| `M` | Medium |
| `L` | Low |

## Complexity

Fibonacci numbers only: `0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89`

Tasks with complexity 13+ should typically be expanded into subtasks.

## ID Format

```
<phase>:<local_id>[.<subtask>...]
```

Examples: `auth:1`, `auth:1.1`, `api:2.3.1`

## Example

```
# SCUD Graph v1
# Phase: auth

@meta {
  name auth
  updated 2025-01-15T10:30:00Z
}

@nodes
auth:1 | Design auth system | X | 13 | H
auth:1.1 | Implement JWT tokens | D | 5 | H
auth:1.2 | Add refresh token flow | I | 3 | M
auth:2 | Rate limiting | P | 5 | M

@edges
auth:1.2 -> auth:1.1
auth:2 -> auth:1

@parents
auth:1: auth:1.1, auth:1.2

@assignments
auth:1.2 | alice | alice | 2025-01-15T09:00:00Z

@details
auth:1 | description |
  Design the authentication system architecture.
  Must support OAuth2 and API keys.
auth:1.1 | test_strategy |
  Unit tests for token generation and validation.
```

## Escaping

| Character | Escaped |
|-----------|---------|
| `\|` | `\\|` |
| `\` | `\\` |
| newline (in single-line) | `\n` |

Multiline content in `@details` uses 2-space indentation instead of escaping.

## Validation Rules

- **IDs**: Max 100 chars, alphanumeric + `-_:.`
- **Titles**: Non-empty, max 200 chars
- **Descriptions**: Max 5000 chars
- **Dependencies**: Must form a DAG (no cycles)
- **Subtasks**: Parent must have status `X`

## Multiple Phases

Phases are separated by `---`:

```
# SCUD Graph v1
# Phase: auth
...

---

# SCUD Graph v1
# Phase: api
...
```

## Graph Concepts

- **Waves**: Groups of tasks executable in parallel (same depth in DAG)
- **Blocked**: Task whose dependencies aren't all `Done`
- **Ready**: `Pending` task with all dependencies `Done`
