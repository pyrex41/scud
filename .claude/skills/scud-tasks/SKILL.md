---
name: scud-tasks
description: SCUD task management - view, update, and track tasks in the SCUD DAG system. Use when the user asks about tasks, wants to see progress, needs the next task, or wants to update task status.
---

# SCUD Task Management

SCUD organizes work as a DAG (directed acyclic graph) of tasks with dependencies, priorities, and complexity scores.

## Quick Reference

```bash
scud warmup                        # Session start: status + next task
scud next                          # Next available task (all deps done)
scud show <id>                     # Full task details
scud list                          # All tasks in active tag
scud list --status pending         # Filter by status
scud set-status <id> in-progress   # Start working
scud set-status <id> done          # Mark complete
scud stats                         # Completion statistics
scud waves                         # View parallel execution plan
scud create --title "..."          # Create a new task
```

## Workflow

1. **Orient**: `scud warmup` - see project state and what's next
2. **Claim**: `scud set-status <id> in-progress` - mark you're working on it
3. **Implement**: do the work
4. **Commit**: `scud commit -m "message"` - auto-prefixes `[TASK-ID]`
5. **Complete**: `scud set-status <id> done` - unblocks dependent tasks
6. **Repeat**: `scud next` - find next available task

## Task Statuses

| Status | Meaning |
|--------|---------|
| `pending` | Ready to start (or waiting on deps) |
| `in-progress` | Currently being worked on |
| `done` | Completed and verified |
| `blocked` | Cannot proceed (external blocker) |
| `failed` | Attempted but failed |
| `review` | Ready for review |
| `expanded` | Decomposed into subtasks |
| `deferred` | Postponed |
| `cancelled` | No longer needed |

## Dependencies & Waves

Tasks can depend on other tasks. A task is "ready" when:
- Status is `pending`
- All dependencies have status `done`

`scud waves` groups ready tasks into parallel waves using topological sort (Kahn's algorithm). Wave 1 has no deps, Wave 2 depends on Wave 1, etc.

`scud next` returns the highest-priority ready task.

## Tags (Phases)

Tasks are grouped by tag (phase/feature). Switch context:

```bash
scud tags              # List all tags
scud tags <name>       # Set active tag
scud list              # Shows tasks in active tag only
```

## Task IDs

Hierarchical: `1`, `1.1`, `1.1.1`. Subtasks inherit parent dependencies.

## Heavy Ensemble for Research

Use `scud heavy` when you need deep analysis or multi-perspective reasoning:

```bash
scud heavy "What are the security implications of this auth change?" -v
scud heavy "query" --mode hybrid   # local files + web research
```
