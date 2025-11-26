---
description: Diagnose and fix SCUD task issues
---

Diagnose issues with tasks like stale locks, orphaned subtasks, or circular dependencies.

Run: `scud doctor $ARGUMENTS`

Arguments: `[--tag <tag>] [--stale-hours <n>] [--fix]`

Report findings:
- Stale locks (tasks locked for too long)
- Orphaned subtasks
- Circular dependencies
- Missing dependency targets

If `--fix` was used, confirm what was repaired.
