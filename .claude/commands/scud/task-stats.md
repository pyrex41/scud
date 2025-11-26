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
- Tasks by status (pending, in-progress, done, blocked)
- Total complexity points completed vs remaining
- Highlight any blocked tasks that need attention
