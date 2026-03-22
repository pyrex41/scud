# SCUD Task Manager Skill

Use when working with SCUD tasks, planning work, or tracking progress.

## Essential Commands

```bash
scud warmup                      # Session orientation
scud next                        # Find next available task
scud show <id>                   # Task details
scud list [--status pending]     # List tasks
scud set-status <id> <status>    # Update status
scud stats                       # Progress statistics
scud waves                       # Parallel execution plan
scud commit -m "msg"             # Task-aware git commit
scud heavy "query" -v            # Multi-agent reasoning
scud mcp-server                  # Start MCP server
```

## Workflow

1. `scud warmup` - orient
2. `scud next` - find available task
3. `scud set-status <id> in-progress` - claim it
4. Implement the task
5. `scud commit -m "message"` - commit with task prefix
6. `scud set-status <id> done` - complete

## Status Values

`pending`, `in-progress`, `done`, `blocked`, `failed`, `review`, `expanded`, `deferred`, `cancelled`
