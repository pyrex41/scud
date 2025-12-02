# SCUD Quick Reference

## 5-Phase Workflow

```
Ideation → Planning → Architecture → Implementation → Retrospective
```

| Phase | Agent | Command | Output |
|-------|-------|---------|--------|
| **Ideation** | Product Manager | `/tm-pm` | PRD document |
| **Planning** | PM + Scrum Master | `/tm-pm` → `/tm-sm` | Feature files + Tasks |
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
scud tags                # List all tags
scud use-tag <tag>       # Switch active tag
scud list                # List tasks
scud list --status done  # Filter by status
scud show <id>           # Task details
scud set-status <id> <status>  # Update status
scud next                # Find next task
scud stats               # Show statistics
scud doctor              # [EXPERIMENTAL] Diagnose stuck states
```

### [EXPERIMENTAL] Dynamic-Wave Mode
```bash
# Auto-claim next available task (sets in-progress + locks)
scud next --claim --name <agent>

# Release tasks claimed by agent
scud next --release --name <agent>

# Check workflow health
scud doctor

# Auto-fix stale locks and orphan tasks
scud doctor --fix
```

**Agent Obligation:** After `--claim`, MUST run `scud set-status <id> done` when complete!

### AI Commands (require XAI_API_KEY)
```bash
scud parse-prd <file> --tag <tag>  # Parse PRD into tasks
scud analyze-complexity             # Score all tasks
scud analyze-complexity --task <id> # Score specific task
scud expand <id>                    # Split complex task
scud expand --all                   # Split all tasks >13
scud research "<query>"             # AI research
```

Default model: `grok-code-fast-1`. Configure with `scud config`.

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

# 3. Create feature specs
/tm-pm  # (in planning mode)
# Creates docs/features/*.md

# 4. Parse first feature
scud parse-prd docs/features/auth.md --tag auth

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

### Continuing Existing Feature

```bash
# Check status
scud status

# Switch tag if needed
scud use-tag auth

# Find next task
scud next

# Or continue with agent
/tm-dev
```

---

## Agent Cheat Sheet

### `/tm-pm` (Product Manager)
- **When:** Start of project, defining features
- **Creates:** PRD, feature files
- **Phase:** Ideation → Planning

### `/tm-sm` (Scrum Master)
- **When:** After features are defined
- **Does:** Parse PRDs, refine tasks, break down complex tasks
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
- **Does:** Show current phase, active tag, next steps
- **Phase:** Any

---

## File Structure

```
project/
├── .scud/
│   ├── tasks/
│   │   └── tasks.scg               # All tasks in SCG format
│   └── config.toml                 # Active tag and settings
│
├── docs/
│   ├── prd/
│   │   └── product-name-prd.md     # Product requirements
│   ├── features/
│   │   ├── auth.md                 # Feature specifications
│   │   └── payments.md
│   ├── architecture/
│   │   └── auth-architecture.md    # Technical designs
│   └── retrospectives/
│       └── auth-retrospective.md   # Learnings
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

## Task SCG Structure

Tasks are stored in SCG (SCUD Graph) format. See [SCG_FORMAT_SPEC.md](SCG_FORMAT_SPEC.md) for full details.

```
@nodes
1 | Create User model | P | 3 | H

@edges
2 -> 1

@details
1 | description |
  Implement User model with validation
1 | test_strategy |
  Unit tests for model validation
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

# Configure provider/model with:
# scud config --provider xai --model grok-code-fast-1
```

---

## Troubleshooting Quick Fixes

| Problem | Solution |
|---------|----------|
| No tasks file | `scud init` |
| No active tag | `scud use-tag <tag>` |
| Wrong phase | Check `/status`, use correct agent |
| Dependencies not met | `scud next` to find available tasks |
| Task too complex | `scud expand <id>` or `scud expand --all` |
| No API key | `export ANTHROPIC_API_KEY=sk-ant-...` |
| Rust binary missing | `cd scud-cli && cargo build --release` |
| Stale locks | `scud doctor --fix` or `scud release <id> --force` |
| Stuck workflow | `scud doctor` to diagnose issues |
| Tasks blocked | `scud doctor` to find cancelled/missing deps |

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

Track these for each tag:

- **Total tasks:** Number of tasks in tag
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
/tm-pm              # Create feature spec
scud parse-prd docs/features/feature.md --tag feature
/tm-architect       # Add details
/tm-dev             # Implement
/tm-retrospective   # Learn
```

### Full Project (Large)
```bash
scud init
/tm-pm              # Comprehensive PRD
/tm-pm              # Break into 5-7 features

# For each feature:
scud parse-prd docs/features/auth.md --tag auth
scud analyze-complexity
scud expand --all
/tm-architect
/tm-dev
/tm-retrospective

# Repeat for each feature
```

### Bug Fix Batch
```bash
# Create "bug-fixes" tag
scud parse-prd docs/features/bugs.md --tag bugs
# Or manually add tasks to tasks.scg
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
- Tag releases by feature
- Track in commit messages

### With Other Tools
- Export tasks to GitHub Issues / Jira
- Use SCUD for planning, external tools for tracking
- Keep `.scud/` in sync

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
