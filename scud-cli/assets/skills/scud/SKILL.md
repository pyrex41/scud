---
name: scud-guide
description: SCUD CLI reference - all commands, workflow, best practices (list, waves, tags, next, log, etc)
---

# SCUD Command Guide

SCUD: Fast DAG task manager for AI dev. Commands reference + workflow.

## Core Workflow
```
scud tags          # Switch phases
scud list          # View tasks
scud next          # Get ready task
scud waves         # Plan parallel waves
scud set-status ID STATUS
scud stats         # Progress
scud log ID        # Log entry
scud commit        # Task-aware git commit
```

## Full Commands
| Category | Command | Description |
|----------|---------|-------------|
| View | `scud list [--status pending]` | List tasks |
| | `scud show ID` | Task details |
| | `scud tags` | List/set active tag |
| Work | `scud next` | Next task |
| | `scud waves` | Dependency waves |
| | `scud set-status ID [pending\|in-progress\|done\|blocked]` | Update status |
| | `scud assign ID @dev` | Assign |
| Progress | `scud stats` | Stats |
| | `scud whois` | Who's working |
| Logs | `scud log ID` | Add log |
| | `scud log-show ID` | View logs |
| Git | `scud commit -m "msg"` | [TASK-ID] prefixed |
| AI | `scud parse PRD.md` | Parse to tasks |
| | `scud expand ID` | Subtasks |
| Utils | `scud view` | Browser UI |
| | `scud mermaid` | Graph |
| | `scud doctor` | Diagnose |

## Slash Commands
`/scud:next`, `/scud:task-list`, etc (auto-installed).

## Best Practices
- Find next: `scud next`
- Waves for parallel: `scud waves`
- Log everything: `scud log ID "Progress: X done"`

See `scud help` for details.