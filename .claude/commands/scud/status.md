---
description: Update the status of a SCUD task
allowed-tools: Bash(scud:*)
argument-hint: <task-id> <status> [--tag <tag>]
---

Update a task's status. Valid statuses: pending, in-progress, done, blocked, review, deferred, cancelled.

```bash
scud set-status $ARGUMENTS
```

After updating:
1. Confirm the status change
2. If marked `done`, suggest running `scud next` to find the next task
3. If marked `blocked`, ask what's blocking and whether to add a note
