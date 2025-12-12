---
description: Update the status of a SCUD task
---

Update a task's status.

Run: `scud set-status $ARGUMENTS`

Arguments: `<task-id> <status> [--tag <tag>]`

Valid statuses: pending, in-progress, done, blocked, review, deferred, cancelled

After updating:
1. Confirm the status change
2. If marked `done`, suggest running `scud next` to find the next task
3. If marked `blocked`, ask what's blocking and whether to add a note
