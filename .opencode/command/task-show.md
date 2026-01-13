---
description: Show detailed information about a SCUD task
allowed-tools: Bash(scud:*)
argument-hint: <task-id> [--tag <tag>]
---

Show detailed information about a specific task.

```bash
scud show $ARGUMENTS
```

Present the task details including:
- Title, status, complexity, priority
- Full description
- Test strategy (if defined)
- Dependencies and their current status
- Assignment and lock information
