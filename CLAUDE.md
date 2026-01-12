# Project Instructions

## SCUD Task Management

This project uses SCUD Task Manager for task management.

### Session Workflow

1. **Start of session**: Run `scud warmup` to orient yourself
   - Shows current working directory and recent git history
   - Displays active tag, task counts, and any stale locks
   - Identifies the next available task

2. **Claim a task**: Use `/scud:task-next` or `scud next --claim --name "Claude"`
   - Always claim before starting work to prevent conflicts
   - Task context is stored in `.scud/current-task`

3. **Work on the task**: Implement the requirements
   - Reference task details with `/scud:task-show <id>`
   - Dependencies are automatically tracked by the DAG

4. **Commit with context**: Use `scud commit -m "message"` or `scud commit -a -m "message"`
   - Automatically prefixes commits with `[TASK-ID]`
   - Uses task title as default commit message if none provided

5. **Complete the task**: Mark done with `/scud:task-status <id> done`
   - The stop hook will prompt for task completion

### Progress Journaling

Keep a brief progress log during complex tasks:

```
## Progress Log

### Session: 2025-01-15
- Investigated auth module, found issue in token refresh
- Updated refresh logic to handle edge case
- Tests passing, ready for review
```

This helps maintain continuity across sessions and provides context for future work.

### Key Commands

- `scud warmup` - Session orientation
- `scud next` - Find next available task
- `scud show <id>` - View task details
- `scud set-status <id> <status>` - Update task status
- `scud commit` - Task-aware git commit
- `scud stats` - View completion statistics
