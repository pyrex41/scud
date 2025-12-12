# SCUD Quick Reference

## Commands Overview

### Setup
```bash
scud init                          # Initialize SCUD in current directory
scud warmup                        # Quick session orientation
```

### Task Management
```bash
scud tags                          # List all tags
scud tags <tag>                    # Set active tag
scud list [--tag <tag>]            # List tasks
scud list --status pending         # Filter by status
scud show <id>                     # Task details
scud set-status <id> <status>      # Update status
scud next [--tag <tag>]            # Find next ready task
scud stats [--tag <tag>]           # Show statistics
```

### Visualization
```bash
scud serve                         # Start web dashboard (port 3000)
scud serve --port 8080             # Custom port
scud mermaid                       # Generate Mermaid diagram
scud mermaid --all-tags            # All tags in one diagram
scud waves [--tag <tag>]           # Show parallel execution waves
```

### AI Commands (require XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD into tasks
scud analyze-complexity            # Score all tasks
scud analyze-complexity --task <id> # Score specific task
scud expand <id>                   # Split complex task
scud expand --all                  # Split all tasks >13
```

Default model: `grok-3-mini`. Configure with `scud config`.

### Orchestrator Commands
```bash
scud claim <id> --name <name>      # Claim task (lock)
scud release <id>                  # Release task lock
scud whois [--tag <tag>]           # See who's working on what
scud doctor [--tag <tag>]          # Diagnose stuck states
scud doctor --fix                  # Auto-fix stale locks
scud next-batch [--count 5]        # Get multiple ready tasks
```

### Utilities
```bash
scud log <id> "message"            # Add log entry to task
scud log-show <id>                 # Show task log entries
scud commit [-m "msg"]             # Git commit with task context
scud commit --all                  # Stage all, then commit
scud clean [--tag <tag>]           # Clear tasks (with confirmation)
scud clean --force                 # Skip confirmation
```

---

## Task Statuses

```
pending → in-progress → done
              ↓
      review, blocked, deferred, cancelled
```

**Valid statuses:** `pending`, `in-progress`, `done`, `review`, `blocked`, `deferred`, `cancelled`

---

## Complexity Scale (Fibonacci)

| Score | Effort | Action |
|-------|--------|--------|
| **1** | Trivial | Good to go |
| **2** | Simple | Good to go |
| **3** | Moderate | Good to go |
| **5** | Medium | Good to go |
| **8** | Complex | Good to go |
| **13** | Large | Should split |
| **21** | Too large | Must split |

**Rule:** All tasks should be ≤8 complexity for implementation.

---

## Typical Workflow

```bash
# 1. Initialize (once per project)
scud init

# 2. Parse feature doc into tasks
scud parse docs/feature.md --tag my-feature

# 3. Review tasks
scud list
scud analyze-complexity
scud expand --all  # if needed

# 4. View execution plan
scud waves

# 5. Work on tasks
scud next                          # Find ready task
scud set-status <id> in-progress   # Start work
# ... do the work ...
scud set-status <id> done          # Complete

# 6. Track progress
scud stats
scud serve  # Web dashboard
```

---

## File Structure

```
.scud/
├── tasks/tasks.scg     # All tasks in SCG format
├── config.toml         # Provider/model settings
├── active-tag          # Currently active tag
├── current-task        # Active task ID
└── logs/               # Task log entries
```

---

## SCG Format

Tasks are stored in SCG (SCUD Graph) format. See [SCG_FORMAT_SPEC.md](SCG_FORMAT_SPEC.md).

```
@nodes
1 | Create User model | P | 3 | H

@edges
2 -> 1

@details
1 | description |
  Implement User model with validation
```

**Status codes:** P=Pending, I=InProgress, D=Done, R=Review, B=Blocked, F=Deferred, C=Cancelled, X=Expanded
**Priority codes:** H=High, M=Medium, L=Low

---

## Environment Variables

```bash
# Required for AI commands (default provider: xAI)
export XAI_API_KEY=xai-...

# Alternative providers:
# export ANTHROPIC_API_KEY=sk-ant-...
# export OPENAI_API_KEY=sk-...
# export OPENROUTER_API_KEY=sk-or-...

# Configure provider/model:
scud config --provider xai --model grok-3-mini
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| No tasks file | `scud init` |
| No active tag | `scud tags <tag>` |
| Dependencies not met | `scud next` finds available tasks |
| Task too complex | `scud expand <id>` |
| No API key | `export XAI_API_KEY=xai-...` |
| Stale locks | `scud doctor --fix` |
| Stuck workflow | `scud doctor` to diagnose |

---

## Best Practices

**Do:**
- Keep tasks ≤8 complexity
- Use `scud next` to respect dependencies
- Use `scud waves` to plan parallel work
- Track progress with `scud stats`

**Don't:**
- Create tasks >13 complexity without splitting
- Ignore task dependencies
- Work on multiple tasks without claiming

---

## Resources

- **SCG Format:** [SCG_FORMAT_SPEC.md](SCG_FORMAT_SPEC.md)
- **Orchestrator Pattern:** [../orchestrator.md](../orchestrator.md)
- **Parallel Features:** [../features/PARALLEL_FEATURES.md](../features/PARALLEL_FEATURES.md)

**Happy building!**
