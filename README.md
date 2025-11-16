# SCUD

**Sprint Cycle Unified Development** - Fast, AI-powered workflow for building software 🚀

> **Status: Beta (1.0.0-beta.1)** - Core functionality is stable and 50x faster than previous version. Tests and CI/CD are coming in follow-up releases. See [FOLLOWUP_PLAN.md](FOLLOWUP_PLAN.md) for roadmap.

A lightweight task management system that guides you through structured development phases with AI assistance.

---

## Quick Start

### Install
```bash
npm install -g scud
cd your-project
scud init
```

### Use with Claude Code
```bash
/status                    # Check workflow state
/tm-pm                     # Create PRD
/tm-sm                     # Plan tasks
/tm-architect              # Design solution
/tm-dev                    # Implement
/tm-retrospective          # Learn & improve
```

**Full guide:** [QUICKSTART.md](QUICKSTART.md)

---

## The 5-Phase Workflow

```
Ideation → Planning → Architecture → Implementation → Retrospective
  (PRD)     (Tasks)    (Design)       (Code)          (Learn)
```

Each phase has a dedicated AI agent that guides you through best practices:
- ✅ **Product Manager** - Define requirements
- ✅ **Scrum Master** - Break into tasks
- ✅ **Architect** - Design solution
- ✅ **Developer** - Implement with tests
- ✅ **Facilitator** - Capture learnings

---

## Key Features

### Fast Rust CLI
- ⚡ **50x faster** than external task-master
- 🎯 **42x fewer tokens** (500 vs 21k)
- 📦 **Single binary** - no dependencies

### Parallel Development (Experimental)
- 🔀 **Epic Groups** - Coordinate backend/frontend
- 👥 **Task Assignment** - Team collaboration
- 🔒 **Task Locking** - Prevent conflicts

### Smart Task Management
- 📋 Automatic complexity analysis
- 🔗 Dependency tracking
- 🧪 Test requirements
- 📊 Progress metrics

---

## Documentation

**Getting Started:**
- [Quickstart Guide](QUICKSTART.md) - Get up and running in 5 minutes
- [Complete Guide](COMPLETE_GUIDE.md) - Comprehensive reference (25,000 words)
- [Quick Reference](QUICK_REFERENCE.md) - Command cheat sheet

**Features:**
- [Parallel Features](PARALLEL_FEATURES.md) - Epic groups & task assignment

**Implementation:**
- [Development Logs](log_docs/) - Implementation details & history

---

## Commands

### Core Commands (Instant)
```bash
scud init                          # Initialize SCUD
scud tags                          # List all epics
scud list                          # List tasks
scud next                          # Find next task
scud set-status <id> <status>      # Update task
scud stats                         # Show statistics
```

### AI Commands (Requires ANTHROPIC_API_KEY)
```bash
scud parse-prd <file> --tag <tag>  # Parse epic into tasks
scud analyze-complexity             # Analyze all tasks
scud expand --all                   # Break down complex tasks
scud research "<query>"             # AI research
```

### Parallel Development (Experimental)
```bash
# Epic Groups
scud create-group "Name" --epics tag1,tag2
scud group-status <group-id>

# Task Assignment
scud claim <task-id> --name <you>
scud whois                          # See assignments
```

---

## Example Workflow

```bash
# 1. Initialize
scud init

# 2. Define product (with Claude Code)
/tm-pm
# Creates: docs/prd/my-app-prd.md

# 3. Create epics
/tm-pm
# Creates: docs/epics/epic-1-auth.md

# 4. Parse into tasks
scud parse-prd docs/epics/epic-1-auth.md --tag epic-1-auth
# Creates tasks in .taskmaster/tasks/tasks.json

# 5. Analyze & refine
scud analyze-complexity
scud expand --all  # Split complex tasks

# 6. Design solution
/tm-architect
# Adds technical details to all tasks

# 7. Implement
/tm-dev
# Agent uses: scud next → implement → scud set-status X done

# 8. Retrospective
/tm-retrospective
# Captures learnings in docs/retrospectives/
```

---

## Why SCUD?

**Structured but Flexible:**
- Enforces best practices (dependencies, testing, phase gates)
- But adapts to your workflow
- No heavy XML or complex configuration

**AI-Powered:**
- Parse PRDs automatically
- Analyze task complexity
- Break down large tasks
- Research technical topics

**Fast & Simple:**
- Rust CLI is instant
- JSON storage is transparent
- Works offline (core commands)
- No vendor lock-in

**Team-Ready:**
- Epic groups for parallel work
- Task assignment for collaboration
- Git worktree support
- Progress tracking

---

## Requirements

- **Node.js 16+** (for npm package wrapper)
- **Rust & Cargo** (for building CLI, or use pre-built binary)
- **Anthropic API key** (for AI features)

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

---

## File Structure

```
.taskmaster/
├── tasks/tasks.json          # All tasks by epic
├── workflow-state.json       # Current phase & epic
└── epic-groups.json          # Epic groups (parallel features)

docs/
├── prd/                      # Product requirements
├── epics/                    # Epic descriptions
├── architecture/             # Technical designs
└── retrospectives/           # Learnings

.claude/commands/             # AI agents (slash commands)
```

---

## Development

```bash
# Build Rust CLI
cd scud-cli
cargo build --release

# The binary will be at:
# scud-cli/target/release/scud
```

---

## Contributing

Issues and PRs welcome at [github.com/pyrex41/scud](https://github.com/pyrex41/scud)

---

## License

MIT

---

## Learn More

- **Complete Guide:** [COMPLETE_GUIDE.md](COMPLETE_GUIDE.md)
- **Quick Reference:** [QUICK_REFERENCE.md](QUICK_REFERENCE.md)
- **Parallel Features:** [PARALLEL_FEATURES.md](PARALLEL_FEATURES.md)
- **Implementation Logs:** [log_docs/](log_docs/)

**Happy building! 🚀**
