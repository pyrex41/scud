---
name: scud-tasks
description: SCUD task management - view, update, claim, and track tasks in the SCUD graph system
---

# SCUD Task Management

This skill provides tools and knowledge for managing tasks in the SCUD (Structured Collaboration Using Dependencies) system. SCUD is a phase-gated task management system designed for AI-driven development workflows.

## When to Use This Skill

Use this skill when:
- Viewing or listing tasks (`scud list`, `scud show`)
- Updating task status (`scud set-status`)
- Claiming or releasing tasks for parallel work
- Finding the next available task
- Checking progress and statistics
- Understanding wave-based parallelism
- Working with task dependencies

## Quick Reference

### View Tasks
```bash
scud list                           # List all tasks in active tag
scud list --tag auth                # List tasks in specific tag
scud list --status pending          # Filter by status
scud show <task-id>                 # Show task details
scud show <task-id> --tag auth      # Show task in specific tag
```

### Update Status
```bash
scud set-status <task-id> pending
scud set-status <task-id> in-progress
scud set-status <task-id> done
scud set-status <task-id> blocked
scud set-status <task-id> review
```

### Task Assignment & Locking
```bash
scud claim <task-id> --name <name>  # Lock task for yourself
scud release <task-id>              # Release lock
scud whois                          # Show who's working on what
```

### Find Work
```bash
scud next                           # Get next available task
scud next --claim --name <name>     # Find and claim in one step
scud waves                          # See parallel execution waves
```

### Progress
```bash
scud stats                          # Show completion statistics
scud tags                           # List all tags
scud tags <tag>                     # Set active tag
```

## Task Statuses

| Code | Name | Meaning |
|------|------|---------|
| P | Pending | Not started |
| I | InProgress | Currently being worked on |
| D | Done | Completed |
| R | Review | Awaiting review |
| B | Blocked | Cannot proceed |
| F | Deferred | Postponed |
| C | Cancelled | Aborted |
| X | Expanded | Decomposed into subtasks |

## Key Concepts

### Tags (Task Groups)
Tasks are organized into tags (like `auth`, `api`, `ui`). Always specify `--tag <tag>` or set the active tag with `scud tags <tag>`.

### Waves
Tasks are organized into waves based on dependencies:
- **Wave 1**: Tasks with no dependencies (can all run in parallel)
- **Wave 2+**: Tasks that depend on earlier waves

Use `scud waves` to see the wave breakdown.

### Claiming Tasks
When multiple agents or developers work in parallel, use `scud claim` to prevent conflicts:
```bash
scud claim <task-id> --name alice   # Lock task
# ... work on task ...
scud release <task-id>              # Unlock when done
```

## Additional Documentation

For more details, see:
- [Task Commands Reference](task-commands.md) - Complete CLI command reference
- [Graph Concepts](graph-concepts.md) - Understanding SCG format and waves
