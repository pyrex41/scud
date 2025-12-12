---
description: Create git commit with SCUD task context
---

Create a git commit with automatic task context.

Run: `scud commit $ARGUMENTS`

Arguments: `[-m <message>] [-a]`

Options:
- `-m "message"` - Commit message (uses current task title if not provided)
- `-a` - Stage all changes before committing

This automatically prefixes commits with the current task ID.
