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
4. Remind about hooks: if hooks are installed, set `SCUD_TASK_ID=<id>` when starting work
5. Suggest the command to start working: `scud set-status <id> in-progress`

**Note:** The `--claim` flag is experimental. It locks the task to prevent conflicts in parallel workflows.

**Hook Integration:**
- When hooks are installed (`scud hooks install`), tasks are automatically marked complete when Claude sessions end
- Set the `SCUD_TASK_ID` environment variable to enable automatic completion
- Example: `SCUD_TASK_ID=5 claude "Implement task 5"`
