---
description: List or set the active SCUD task tag
allowed-tools: Bash(scud:*)
argument-hint: [<tag>]
---

List all tags or set the active tag.

```bash
scud tags $ARGUMENTS
```

If listing tags:
- Show all available tags
- Indicate which is currently active
- Show task count per tag if available

If setting a tag:
- Confirm the active tag changed
- Show quick stats for the new active tag
