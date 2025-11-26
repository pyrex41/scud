---
description: Diagnose and fix SCUD task issues
allowed-tools: Bash(scud:*)
argument-hint: [--tag <tag>] [--stale-hours <n>] [--fix]
---

Diagnose issues with tasks like stale locks, orphaned subtasks, or circular dependencies.

```bash
scud doctor $ARGUMENTS
```

Report findings:
- Stale locks (tasks locked for too long)
- Orphaned subtasks
- Circular dependencies
- Missing dependency targets

If `--fix` was used, confirm what was repaired.
