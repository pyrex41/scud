---
event: session:start
---

# SCUD Skills Bootstrap

You have access to the **SCUD task management system** with skills that provide specialized knowledge and workflows.

## Available Skills

Use the `find_skills` tool to discover available skills, then `use_skill` to activate them.

### SCUD Task Management Skill

The `scud-tasks` skill provides comprehensive task management capabilities:

- **Task viewing**: `scud list`, `scud show`
- **Status updates**: `scud set-status`
- **Parallel work**: `scud claim`, `scud release`, `scud whois`
- **Finding work**: `scud next`, `scud waves`
- **Progress tracking**: `scud stats`, `scud tags`

### Tool Translation

When skills reference Claude Code tools, use OpenCode equivalents:
- `TodoWrite` → `update_plan`
- `Task` tool with subagents → Use OpenCode's subagent system (@mention)
- `Skill` tool → `use_skill` custom tool
- `Read`, `Write`, `Edit`, `Bash` → Your native tools

## Task Commands

These slash commands are available for task management:
- `/task-list` - List tasks with optional status filter
- `/task-next` - Find next available task
- `/task-show` - Show task details
- `/task-status` - Update task status
- `/task-claim` - Claim a task lock
- `/task-release` - Release a task lock
- `/task-waves` - Show parallel execution waves
- `/task-stats` - Show completion statistics
- `/task-whois` - Show who is working on tasks
- `/task-tags` - List or set active tag
- `/task-doctor` - Diagnose task issues

## SCUD Workflow Agents

For full workflow management, SCUD provides phase-based agents:
- `/scud:pm` - Product Manager (PRD creation)
- `/scud:sm` - Scrum Master (task breakdown)
- `/scud:architect` - Architect (technical design)
- `/scud:dev` - Developer (implementation)
- `/scud:retrospective` - Retrospective (post-phase analysis)
- `/scud:status` - Workflow status
