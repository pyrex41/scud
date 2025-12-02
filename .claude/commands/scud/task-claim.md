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
2. Remind about automatic release: if hooks are installed, the lock will be auto-released when the task is marked complete
3. Manual release command: `scud release <id>`

After releasing:
1. Confirm the lock is cleared
2. Show how long the task was locked

**Hook Integration:**
- When hooks are installed (`scud hooks install`), task locks are automatically released when the task is marked complete
- This happens when the Claude session ends with `SCUD_TASK_ID` set
- Manual release is only needed if a session crashes or is interrupted
