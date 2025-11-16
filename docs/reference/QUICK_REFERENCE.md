# SCUD Quick Reference

## 5-Phase Workflow

```
Ideation → Planning → Architecture → Implementation → Retrospective
```

| Phase | Agent | Command | Output |
|-------|-------|---------|--------|
| **Ideation** | Product Manager | `/tm-pm` | PRD document |
| **Planning** | PM + Scrum Master | `/tm-pm` → `/tm-sm` | Epic files + Tasks |
| **Architecture** | Architect | `/tm-architect` | Architecture doc + Task details |
| **Implementation** | Developer | `/tm-dev` | Working code + Tests |
| **Retrospective** | Facilitator | `/tm-retrospective` | Learnings doc |

---

## Common Commands

### Setup
```bash
scud init                # Initialize SCUD
scud status              # Check current state
```

### Task Management
```bash
scud tags                # List all epics
scud use-tag <tag>       # Switch epic
scud list                # List tasks
scud list --status done  # Filter by status
scud show <id>           # Task details
scud set-status <id> <status>  # Update status
scud next                # Find next task
scud stats               # Show statistics
```

### AI Commands (require ANTHROPIC_API_KEY)
```bash
scud parse-prd <file> --tag <tag>  # Parse epic into tasks
scud analyze-complexity             # Score all tasks
scud analyze-complexity --task <id> # Score specific task
scud expand <id>                    # Split complex task
scud expand --all                   # Split all tasks >13
scud research "<query>"             # AI research
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

| Score | Time | Description | Action |
|-------|------|-------------|--------|
| **1** | ~30 min | Trivial (config change) | ✅ Good to go |
| **2** | 30m-1h | Simple (add validation) | ✅ Good to go |
| **3** | 1-2h | Moderate (new endpoint) | ✅ Good to go |
| **5** | 2-4h | Complex (integration) | ✅ Good to go |
| **8** | 4-8h | Very complex | ✅ Good to go |
| **13** | 1 day | Extremely complex | ⚠️ Should split |
| **21** | 2+ days | Too large | ❌ Must split |

**Rule:** All tasks should be ≤8 complexity for implementation.

---

## Typical Session Flow

### Starting a New Feature

```bash
# 1. Initialize (once per project)
scud init

# 2. Create PRD
/tm-pm
# Answer questions, PRD created in docs/prd/

# 3. Create epics
/tm-pm  # (in planning mode)
# Creates docs/epics/epic-*.md

# 4. Parse first epic
scud parse-prd docs/epics/epic-1-auth.md --tag epic-1-auth

# 5. Review and refine tasks
scud list
scud analyze-complexity
scud expand --all  # if needed

# 6. Architecture
/tm-architect
# Creates architecture doc, adds task details

# 7. Implementation loop
/tm-dev
# Agent uses: scud next → implement → scud set-status X done → repeat

# 8. Retrospective
/tm-retrospective
# All tasks done, creates learnings doc
```

### Continuing Existing Epic

```bash
# Check status
scud status

# Switch to epic if needed
scud use-tag epic-1-auth

# Find next task
scud next

# Or continue with agent
/tm-dev
```

---

## Agent Cheat Sheet

### `/tm-pm` (Product Manager)
- **When:** Start of project, defining features
- **Creates:** PRD, epic files
- **Phase:** Ideation → Planning

### `/tm-sm` (Scrum Master)
- **When:** After epics are created
- **Does:** Parse epics, refine tasks, break down complex tasks
- **Phase:** Planning
- **Key:** Ensures all tasks are ≤13 complexity

### `/tm-architect` (Architect)
- **When:** After all tasks are created and refined
- **Does:** Design solution, add technical details to ALL tasks
- **Phase:** Architecture
- **Key:** Must complete before implementation

### `/tm-dev` (Developer)
- **When:** Architecture is complete
- **Does:** Execute tasks, write tests, track progress
- **Phase:** Implementation
- **Rules:**
  - Uses `scud next` to find tasks
  - Validates dependencies
  - MUST write tests
  - MUST pass tests before marking done

### `/tm-retrospective` (Facilitator)
- **When:** All tasks are done
- **Does:** Gather metrics, reflection, capture learnings
- **Phase:** Retrospective
- **Triggers:** Requires ALL tasks = `done`

### `/status` (Status Reporter)
- **When:** Anytime
- **Does:** Show current phase, active epic, next steps
- **Phase:** Any

---

## File Structure

```
project/
├── .taskmaster/
│   ├── tasks/
│   │   └── tasks.json              # All tasks, organized by epic
│   └── workflow-state.json         # Current phase, active epic
│
├── docs/
│   ├── prd/
│   │   └── product-name-prd.md     # Product requirements
│   ├── epics/
│   │   ├── epic-1-feature.md       # Epic descriptions
│   │   └── epic-2-feature.md
│   ├── architecture/
│   │   └── epic-1-architecture.md  # Technical designs
│   └── retrospectives/
│       └── epic-1-retrospective.md # Learnings
│
└── .claude/commands/               # Slash commands (if using Claude Code)
    ├── tm-pm.md
    ├── tm-sm.md
    ├── tm-architect.md
    ├── tm-dev.md
    ├── tm-retrospective.md
    └── status.md
```

---

## Task JSON Structure

```json
{
  "id": "1",
  "title": "Create User model",
  "description": "Implement User model with validation",
  "status": "pending",
  "complexity": 3,
  "priority": "high",
  "dependencies": [],
  "details": "Technical implementation details (added by Architect)",
  "test_strategy": "How to test this (added by Architect)",
  "complexity_analysis": "Why this complexity score (from AI)",
  "created_at": "2025-01-15T10:00:00Z",
  "updated_at": "2025-01-15T10:00:00Z"
}
```

---

## Environment Variables

```bash
# Required for AI commands
export ANTHROPIC_API_KEY=sk-ant-...

# Optional: Change model (defaults to claude-sonnet-4-20250514)
export SCUD_MODEL=claude-sonnet-4-20250514
```

---

## Troubleshooting Quick Fixes

| Problem | Solution |
|---------|----------|
| No tasks file | `scud init` |
| No active epic | `scud use-tag <tag>` |
| Wrong phase | Check `/status`, use correct agent |
| Dependencies not met | `scud next` to find available tasks |
| Task too complex | `scud expand <id>` or `scud expand --all` |
| No API key | `export ANTHROPIC_API_KEY=sk-ant-...` |
| Rust binary missing | `cd scud-cli && cargo build --release` |

---

## Best Practices Quick List

✅ **Do:**
- Follow the 5 phases in order
- Keep tasks ≤8 complexity
- Write clear, specific PRDs
- Add technical details in architecture phase
- Write tests for every task
- Run retrospectives

❌ **Don't:**
- Skip phases
- Create tasks >13 complexity
- Ignore dependencies
- Mark tasks done without tests
- Skip retrospectives
- Work on multiple tasks simultaneously

---

## Key Metrics

Track these for each epic:

- **Total tasks:** Number of tasks in epic
- **Total complexity:** Sum of all complexity points
- **Completion %:** (Done tasks / Total tasks) × 100
- **Tasks split:** How many needed expansion
- **Duration:** Time from start to finish

View with: `scud stats`

---

## Common Workflows

### Quick Feature (Small)
```bash
scud init
/tm-pm              # Create mini-PRD
/tm-pm              # Create single epic
scud parse-prd docs/epics/feature.md --tag feature
/tm-architect       # Add details
/tm-dev             # Implement
/tm-retrospective   # Learn
```

### Full Project (Large)
```bash
scud init
/tm-pm              # Comprehensive PRD
/tm-pm              # Break into 5-7 epics

# For each epic:
scud parse-prd docs/epics/epic-1.md --tag epic-1
scud analyze-complexity
scud expand --all
/tm-architect
/tm-dev
/tm-retrospective

# Repeat for each epic
```

### Bug Fix Batch
```bash
# Create "bug-fixes" epic
scud parse-prd docs/epics/bugs.md --tag bugs
# Or manually add tasks to tasks.json
/tm-architect  # Add fix details
/tm-dev        # Fix bugs
```

---

## Performance Tips

### Rust CLI (Fast)
- Core commands (tags, list, show): ~5ms
- AI commands (parse-prd, expand): ~2-3s (API call)

### Speed Up AI Commands
- Use specific task IDs: `scud analyze-complexity --task 5`
- Batch operations: `scud expand --all`
- Keep prompts focused

### Optimize Workflow
- Do all complexity analysis at once
- Expand all tasks before architecture
- Use `scud next` to avoid dependency issues

---

## Integration Points

### With Claude Code
- Slash commands in `.claude/commands/`
- Agents automatically use SCUD CLI
- Seamless workflow

### With Git
- Commit after each phase
- Tag releases by epic
- Track in commit messages

### With Other Tools
- Export tasks to GitHub Issues / Jira
- Use SCUD for planning, external tools for tracking
- Keep `.taskmaster/` in sync

---

## Resources

- **Full Guide:** `COMPLETE_GUIDE.md` - Comprehensive documentation
- **Walkthrough:** `DETAILED_WALKTHROUGH.md` - Step-by-step tutorial
- **Rust CLI:** `RUST_CLI_IMPLEMENTATION.md` - Technical details
- **README:** `README.md` - Project overview
- **Quick Start:** `QUICKSTART.md` - Get started fast

---

**Remember:** SCUD is a guide, not a prison. Follow the workflow, but adapt as needed. The goal is better software, not perfect adherence.

**Happy building! 🚀**
