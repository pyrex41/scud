---
description: Claim a SCUD task lock for parallel work
---

Claim a task to prevent conflicts during parallel work.

Run: `scud claim $ARGUMENTS`

Arguments: `<task-id> --name <name> [--tag <tag>]`

After claiming:
1. Confirm the lock is set
2. Remind to release when done: `scud release <id>`
