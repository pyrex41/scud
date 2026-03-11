---
description: List SCUD tasks with optional status filter
allowed-tools: Bash(scud:*)
argument-hint: [--status pending|in-progress|done|blocked] [--tag <tag>]
---

List tasks from the SCUD task graph.

```bash
scud list $ARGUMENTS
```

After running, summarize:
- Total tasks shown
- Breakdown by status
- Any tasks that are blocked or stale
