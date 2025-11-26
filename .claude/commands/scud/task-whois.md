---
description: Show who is working on SCUD tasks
allowed-tools: Bash(scud:*)
argument-hint: [--tag <tag>]
---

Show task assignments and locks.

```bash
scud whois $ARGUMENTS
```

Display:
- Which tasks are assigned to whom
- Which tasks are currently locked
- How long each lock has been held
- Flag any stale locks (>24 hours)
