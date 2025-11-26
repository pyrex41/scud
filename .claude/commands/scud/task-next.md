---
description: Find and optionally claim the next available SCUD task
allowed-tools: Bash(scud:*)
argument-hint: [--claim --name <name>] [--tag <tag>]
---

Find the next available task based on dependencies and status.

```bash
scud next $ARGUMENTS
```

After finding the next task:
1. Show the task ID, title, and complexity
2. List its dependencies and their status
3. If `--claim` was used, confirm the task is now locked
4. Suggest the command to start working: `scud set-status <id> in-progress`
