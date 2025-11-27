# SCUD

**Sprint Cycle Unified Development** - Fast, AI-powered workflow for building software 🚀

> **Status: Beta (1.0.0-beta.1)** - Core functionality is stable and 50x faster than previous version. Tests and CI/CD are coming in follow-up releases. See [log_docs/FOLLOWUP_PLAN.md](log_docs/FOLLOWUP_PLAN.md) for roadmap.

A lightweight task management system that guides you through structured development phases with AI assistance.

---

## Quick Start

### Install

**Using npm (recommended):**
```bash
npm install -g scud-task
cd your-project
scud init
```

**Using Bun:**
```bash
# Bun blocks postinstall scripts by default
npm install -g scud-task

# Or if you prefer Bun, you'll need to manually run the postinstall:
bun install -g scud-task
cd ~/.bun/install/global/node_modules/scud-task
node bin/postinstall.js
```

### Use with Claude Code
```bash
/status                    # Check workflow state
/scud-pm                     # Create PRD
/scud-sm                     # Plan tasks
/scud-architect              # Design solution
/scud-dev                    # Implement
/scud-retrospective          # Learn & improve
```

**Full guide:** [docs/guides/COMPLETE_GUIDE.md](docs/guides/COMPLETE_GUIDE.md)

---

## Usage Modes

SCUD offers two ways to work with AI assistants, each with different trade-offs:

### Mode 1: Direct CLI + Skills/Commands (Recommended)

**How it works:**
- AI assistant uses SCUD CLI directly via bash
- Slash commands (Claude Code) or skills (OpenCode) provide structured prompts
- Assistant reads/writes files and executes `scud` commands

**Setup:**
```bash
npm install -g scud-task
cd your-project
scud init

# In Claude Code, use:
/scud-pm        # Product manager agent
/scud-sm        # Scrum master agent
/scud-dev       # Developer agent
```

**Pros:**
- ✅ Full file system access (can edit tasks JSON directly if needed)
- ✅ Can see all project files for context
- ✅ More flexible - can use any tool/command
- ✅ Better error messages (sees full CLI output)
- ✅ Can combine SCUD with other tools seamlessly

**Cons:**
- ❌ Requires bash/shell access
- ❌ AI must learn CLI commands
- ❌ More verbose (multi-step operations)

**Best for:** Power users, complex workflows, file-heavy operations

---

### Mode 2: MCP Server (Universal Protocol)

**How it works:**
- Lightweight TypeScript server wraps SCUD CLI
- Exposes 20 MCP tools + 3 resources via standardized protocol
- Works with any MCP-compatible client (Claude Desktop, Cursor, Claude Code, etc.)

**Setup:**
```bash
npm install -g scud scud-mcp

# Configure your MCP client (example for Claude Desktop):
# ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "scud": {
      "command": "scud-mcp",
      "env": {
        "ANTHROPIC_API_KEY": "sk-ant-..."
      }
    }
  }
}

# For Cursor/other clients, see scud-mcp/README.md for config
# Then use naturally: "Initialize SCUD and parse my PRD"
```

**Pros:**
- ✅ Structured protocol (well-defined tool schemas)
- ✅ Works across multiple AI clients (Claude Desktop, Cursor, etc.)
- ✅ Cleaner interface (named tools vs bash commands)
- ✅ Type-safe tool calls with validation
- ✅ Can add file access via MCP resources (extensible)

**Cons:**
- ❌ Requires MCP server installation (extra dependency)
- ❌ Less ad-hoc than direct bash (predefined tools only)
- ❌ Client must support MCP protocol

**Best for:** Multi-client usage, structured workflows, type-safe operations

---

### Which Mode Should You Use?

**Use Direct CLI + Skills if:**
- You want maximum flexibility (any bash command)
- You prefer ad-hoc operations over predefined tools
- You're already comfortable with CLI workflows
- You want zero external dependencies (just the CLI)

**Use MCP Server if:**
- You use multiple AI clients (Claude Desktop, Cursor, etc.)
- You want structured, type-safe tool calls
- You prefer protocol-based integration
- You want extensible architecture (can add custom tools/resources)

**Can you use both?** Yes! The MCP server wraps the CLI, so they're fully compatible. Use MCP for structured operations and CLI for ad-hoc tasks. Both modes work in Claude Code, Cursor, and Claude Desktop.

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
- ⚡ **50x faster** than JavaScript alternatives
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

**Guides:**
- [Complete Guide](docs/guides/COMPLETE_GUIDE.md) - Comprehensive reference (25,000 words)
- [Migration Guide](docs/guides/MIGRATION.md) - Upgrading from BMAD-TM Lite
- [MCP Server Guide](scud-mcp/README.md) - Model Context Protocol integration

**Reference:**
- [Quick Reference](docs/reference/QUICK_REFERENCE.md) - Command cheat sheet

**Features:**
- [Parallel Features](docs/features/PARALLEL_FEATURES.md) - Epic groups & task assignment

**Development:**
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
/scud-pm
# Creates: docs/prd/my-app-prd.md

# 3. Create epics
/scud-pm
# Creates: docs/epics/epic-1-auth.md

# 4. Parse into tasks
scud parse-prd docs/epics/epic-1-auth.md --tag epic-1-auth
# Creates tasks in .scud/tasks/tasks.json

# 5. Analyze & refine
scud analyze-complexity
scud expand --all  # Split complex tasks

# 6. Design solution
/scud-architect
# Adds technical details to all tasks

# 7. Implement
/scud-dev
# Agent uses: scud next → implement → scud set-status X done

# 8. Retrospective
/scud-retrospective
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
.scud/
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

- **Complete Guide:** [docs/guides/COMPLETE_GUIDE.md](docs/guides/COMPLETE_GUIDE.md)
- **Quick Reference:** [docs/reference/QUICK_REFERENCE.md](docs/reference/QUICK_REFERENCE.md)
- **Parallel Features:** [docs/features/PARALLEL_FEATURES.md](docs/features/PARALLEL_FEATURES.md)
- **Implementation Logs:** [log_docs/](log_docs/)

**Happy building! 🚀**
