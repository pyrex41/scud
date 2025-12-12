# SCUD CLI Commands - Quick Reference

**SCUD Task Manager v1.25.0**

---

## Session Startup

```bash
# Quick orientation at session start
scud warmup

# View task statistics
scud stats
```

---

## Tag Management

```bash
# List all tags (shows active tag with *)
scud tags

# Set active tag
scud tags <tag-name>

# Parse markdown into tasks (creates tag if needed)
scud parse docs/features/auth.md --tag auth
```

**Note:** All task operations apply to the **active tag**. Verify with `scud tags`.

---

## Task Viewing & Navigation

```bash
# List all tasks in active tag
scud list

# Filter by status
scud list --status pending
scud list --status done
scud list --status in-progress

# Show detailed task information
scud show 3

# Find next available task (considers dependencies)
scud next

# View task graph as Mermaid diagram
scud mermaid
```

---

## Task Status Management

```bash
# Update task status
scud set-status 3 in-progress
scud set-status 3 done
scud set-status 3 blocked

# Valid statuses:
# - pending
# - in-progress
# - done
# - blocked
# - review
# - deferred
# - cancelled
```

---

## Parallel Execution Planning

```bash
# Show parallel execution waves
scud waves

# Limit parallel tasks per wave
scud waves --max-parallel 3

# Plan across all tags
scud waves --all-tags

# Show who is working on what
scud who-is
```

---

## AI-Powered Commands

```bash
# Parse PRD/markdown into tasks
scud parse docs/feature.md --tag feature-name

# Analyze task complexity
scud analyze-complexity

# Expand complex task into subtasks
scud expand 5

# Re-analyze cross-tag dependencies
scud reanalyze-deps
```

---

## Git Integration

```bash
# Commit with task context (auto-prefixes with task ID)
scud commit -m "implement feature"

# Stage all and commit
scud commit -a -m "complete implementation"
```

---

## Task Logging

```bash
# Write progress log for a task
scud log 3 "Completed auth module, moving to tests"

# View logs for a task
scud log-show 3
```

---

## Diagnostics

```bash
# Diagnose workflow issues
scud doctor

# Auto-fix recoverable issues
scud doctor --fix

# Check specific tag
scud doctor --tag auth

# Set stale threshold (hours)
scud doctor --stale-hours 12
```

---

## Project Setup

```bash
# Initialize SCUD in project
scud init

# View/edit configuration
scud config

# Start web dashboard
scud serve
```

---

## Common Workflows

### Starting a New Feature
```bash
scud parse docs/features/auth.md --tag auth
scud tags auth                    # Switch to new tag
scud list                         # View generated tasks
scud analyze-complexity           # Check complexity
scud expand 5                     # Break down complex tasks
```

### Working on Tasks
```bash
scud warmup                       # Orient yourself
scud next                         # Find available task
scud set-status 3 in-progress     # Start working
scud show 3                       # View details
scud set-status 3 done            # Complete task
scud commit -m "implement auth"   # Commit with context
```

### Parallel Team Work
```bash
scud waves                        # Plan parallel execution
scud who-is                       # See who's working on what
scud next --spawn                 # Get task as JSON for orchestrators
```

---

## File Locations

```
.scud/
├── tasks/
│   └── tasks.scg           # All tasks in SCG format
├── config.toml             # Configuration (provider, model, active tag)
├── current-task            # Currently active task ID
└── docs/                   # Documentation and PRDs
```

---

## Environment Variables

```bash
# Set your AI provider API key:
export XAI_API_KEY=xai-...           # xAI (Grok) - default
export ANTHROPIC_API_KEY=sk-ant-...  # Anthropic (Claude)
export OPENAI_API_KEY=sk-...         # OpenAI
export OPENROUTER_API_KEY=...        # OpenRouter
```

---

## Tips for Agents

1. **Start every session** with `scud warmup` for context
2. **Use `scud next`** to find tasks with satisfied dependencies
3. **Check task details** with `scud show <id>` before starting
4. **Mark status changes** immediately with `scud set-status`
5. **Use `scud commit`** to auto-prefix commits with task context
6. **Run `scud doctor`** if workflow seems stuck

---

**Last Updated:** 2025-12-12
**Version:** SCUD v1.25.0
