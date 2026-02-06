---
name: scud-tasks
description: SCUD task management - view, update, and track tasks in the SCUD graph system (project)
---

# SCUD Task Management

SCUD is a DAG-based task management system for AI-driven development. Tasks are organized into tags (phases) with dependencies.

## When to Use

- Viewing tasks: `scud list`, `scud show <id>`
- Finding work: `scud next`, `scud waves`
- Updating status: `scud set-status <id> <status>`
- Checking progress: `scud stats`

## Essential Commands

```bash
# View tasks
scud list                    # List tasks in active tag
scud list --status pending   # Filter by status
scud show 3                  # Show task details

# Find work
scud next                    # Get next available task
scud waves                   # See parallel execution waves

# Update status
scud set-status 3 in-progress
scud set-status 3 done

# Progress
scud stats                   # Completion statistics
scud tags                    # List/set active tag
```

## Task Statuses

| Status | Meaning |
|--------|---------|
| pending | Not started |
| in-progress | Being worked on |
| done | Completed |
| blocked | Cannot proceed |
| expanded | Decomposed into subtasks |

## Workflow

1. `scud next` - find available task
2. `scud set-status <id> in-progress` - start working
3. Implement the task
4. `scud set-status <id> done` - mark complete

## Key Concepts

**Tags**: Tasks grouped by feature/phase. Set active tag with `scud tags <name>`.

**Waves**: Dependency-based parallelism. Wave 1 has no deps, Wave 2+ depends on prior waves. Use `scud waves` to plan parallel work.

**Dependencies**: Tasks can depend on others. `scud next` only returns tasks with all deps satisfied.
