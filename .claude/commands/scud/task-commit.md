---
description: Create git commit with SCUD task context
allowed-tools: Bash(scud:*)
argument-hint: [-m <message>] [-a]
---

Create a git commit with automatic task context.

```bash
scud commit $ARGUMENTS
```

Options:
- `-m "message"` - Commit message (uses current task title if not provided)
- `-a` - Stage all changes before committing

This automatically prefixes commits with the current task ID from `.scud/current-task`.
