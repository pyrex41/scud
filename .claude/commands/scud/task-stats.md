---
description: Show SCUD task completion statistics
allowed-tools: Bash(scud:*)
argument-hint: [--tag <tag>]
---

Show completion statistics for tasks.

```bash
scud stats $ARGUMENTS
```

Summarize:
- Overall progress percentage
- Tasks by status (pending, in-progress, done, expanded, blocked)
- Per-tag breakdown if multiple tags exist
