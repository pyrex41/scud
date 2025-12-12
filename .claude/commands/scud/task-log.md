---
description: Write a log entry for a SCUD task
allowed-tools: Bash(scud:*)
argument-hint: <task-id> <message>
---

Write a summary log entry for a task.

```bash
scud log $ARGUMENTS
```

Use this to document progress, blockers, or decisions for a task.

To view logs:
```bash
scud log-show <task-id>
```
