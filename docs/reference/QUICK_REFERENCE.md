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
scud view                          # Open task viewer in browser
scud mermaid                       # Generate Mermaid diagram
scud mermaid --all-tags            # All tags in one diagram
scud waves [--tag <tag>]           # Show parallel execution waves
```

### AI Commands (require XAI_API_KEY)
```bash
scud parse <file> --tag <tag>      # Parse PRD into tasks
scud parse <file> --tag <tag> --no-guidance  # Skip guidance
scud analyze-complexity            # Score all tasks
scud analyze-complexity --task <id> # Score specific task
scud expand <id>                   # Split complex task
scud expand --all                  # Split all tasks >13
scud expand --all --no-guidance    # Skip guidance
```

Default model: `grok-code-fast-1`. Configure with `scud config set-provider <provider> --model <model>`.

Project guidance in `.scud/guidance/*.md` is automatically included in prompts.

### Orchestrator Commands
```bash
scud assign <id> <name>            # Assign task to a developer
scud who-is [--tag <tag>]          # See who's working on what
scud next-batch [--limit 5]        # Get multiple ready tasks
scud doctor [--tag <tag>]          # Diagnose stuck states
scud doctor --fix                  # Auto-fix stale locks
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
scud view  # Task viewer
```

---

## File Structure

```
.scud/
├── tasks/tasks.scg     # All tasks in SCG format
├── config.toml         # Provider/model settings
├── active-tag          # Currently active tag
├── current-task        # Active task ID
├── guidance/           # Project guidance for AI
│   └── *.md            # Auto-loaded markdown files
└── logs/               # Task log entries
```

---

## Project Guidance

Provide project context for AI commands by adding `.md` files to `.scud/guidance/`:

```bash
.scud/guidance/
├── coding-standards.md    # Your coding conventions
├── architecture.md        # System architecture notes
└── tech-stack.md          # Technology decisions
```

Files are automatically loaded for `parse` and `expand`. Use `--no-guidance` to skip.

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
scud config set-provider xai --model grok-code-fast-1
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
| Stuck tasks | `scud doctor` to diagnose |

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
