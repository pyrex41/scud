---
description: Claim or release a SCUD task lock
allowed-tools: Bash(scud:*)
argument-hint: <task-id> --name <name> [--tag <tag>] | release <task-id> [--force]
---

Claim a task to prevent conflicts during parallel work, or release a claimed task.

To claim:
```bash
scud claim $ARGUMENTS
```

To release (if first argument is "release"):
```bash
scud release $ARGUMENTS
```

After claiming:
1. Confirm the lock is set
2. Remind to release when done: `scud release <id>`

After releasing:
1. Confirm the lock is cleared
2. Show how long the task was locked
