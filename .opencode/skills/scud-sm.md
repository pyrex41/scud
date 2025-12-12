# SCUD Task Manager Skill

Use when working with SCUD tasks:
- Finding next task to work on
- Viewing task lists and status
- Updating task progress
- Planning parallel execution

## Essential Commands

```bash
scud list                    # List tasks
scud next                    # Find next available task
scud show <id>               # Task details
scud set-status <id> <status>  # Update status
scud stats                   # Progress statistics
scud waves                   # Parallel execution plan
scud tags                    # List/set active tag
```

## Workflow

1. `scud next` - find available task
2. `scud set-status <id> in-progress` - start
3. Implement the task
4. `scud set-status <id> done` - complete

## Status Values

`pending`, `in-progress`, `done`, `blocked`, `expanded`
