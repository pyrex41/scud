---
description: Find and optionally claim the next available SCUD task
allowed-tools: Bash(scud:*)
argument-hint: [--tag <tag>] [--spawn]
---

Find the next available task based on dependencies and status.

```bash
scud next $ARGUMENTS
```

After finding the next task:
1. Show the task ID, title, and complexity
2. List its dependencies and their status
3. Suggest starting with: `scud set-status <id> in-progress`

Use `--spawn` for machine-readable JSON output (for orchestrators).
