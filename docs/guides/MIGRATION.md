# Migration Guide: Task Master → SCUD

This guide helps existing Task Master users migrate to SCUD (Sprint Cycle Unified Development).

## Overview

SCUD is a complete rewrite of the Task Master tool with:
- **50x faster** startup (Rust vs. Node.js with MCP)
- **42x fewer tokens** (direct API calls vs. MCP overhead)
- **Same workflow** - commands and task format are backward compatible
- **New features** - parallel development with tag groups and task assignment

## Breaking Changes

### ⚠️ Storage Format Changed!

SCUD now uses **SCG (SCUD Graph)** format instead of JSON:
- Tasks stored in `.scud/tasks/tasks.scg` (was `.taskmaster/tasks/tasks.json`)
- Config in `.scud/config.toml` (was `.taskmaster/workflow-state.json`)

However, the CLI will automatically migrate your existing JSON tasks if found.

## What Changed

### 1. Binary Name

**Old:**
```bash
task-master list
task-master parse-prd docs/prd/my-feature.md --tag my-feature
```

**New:**
```bash
scud list
scud parse-prd docs/prd/my-feature.md --tag my-feature
```

### 2. Installation

**Old:**
```bash
npm install -g @eyaltoledano/claude-task-master
```

**New:**
```bash
pnpm add -g scud-task   # recommended
# Or:
npm install -g scud-task
```

### 3. Directory Structure

**Old:**
```
.taskmaster/
├── tasks/tasks.json
└── workflow-state.json
```

**New:**
```
.scud/
├── tasks/tasks.scg      # SCG format (75% fewer tokens)
└── config.toml          # Active tag and settings
```

### 4. Performance

**Old behavior:**
- Startup time: ~2-3 seconds
- Parse PRD: ~30-45 seconds
- MCP overhead on every call

**New behavior:**
- Startup time: ~50ms (50x faster)
- Parse PRD: ~8-12 seconds (3-4x faster)
- Direct API calls, no MCP

### 5. New Features (Optional)

SCUD adds **experimental parallel development features** that are completely optional:

#### Tag Groups
Coordinate related tags (e.g., backend + frontend):
```bash
scud create-group fullstack --tags backend,frontend --description "Backend + Frontend"
scud group-status fullstack
```

#### Task Assignment & Locking
Multiple developers can work on the same tag:
```bash
scud assign 123 alice
scud claim 123 --name alice    # Locks the task
scud whois                      # See all assignments
scud release 123                # Unlock when done
```

## Migration Steps

### Step 1: Install SCUD

```bash
pnpm add -g scud-task   # recommended
# Or:
npm install -g scud-task
```

### Step 2: Initialize New Project

```bash
cd your-project
scud init
scud hooks install   # Enable automatic task completion
```

### Step 3: Migrate Existing Tasks (If Any)

If you have an existing `.taskmaster/` directory:

```bash
# SCUD will auto-detect and offer to migrate
scud tags

# Or manually re-parse your PRDs:
scud parse-prd docs/features/my-feature.md --tag my-feature
```

### Step 4: Update Slash Commands (Optional)

If you use Claude Code or OpenCode, the slash commands are already updated:
- `/tm-pm` - Product Manager agent
- `/tm-sm` - Scrum Master agent
- `/tm-architect` - Architect agent
- `/tm-dev` - Developer agent
- `/tm-retrospective` - Retrospective agent

These work identically to before.

### Step 5: Update Any Scripts

If you have scripts calling `task-master`, update them to use `scud`:

```bash
# Old
task-master parse-prd docs/prd/feature.md --tag feature

# New
scud parse-prd docs/features/feature.md --tag feature
```

Or create an alias:
```bash
alias task-master='scud'
```

## Rollback Plan

If you encounter issues, you can rollback:

```bash
# Uninstall SCUD
npm uninstall -g scud-task

# Reinstall old task-master
npm install -g @eyaltoledano/claude-task-master
```

Note: You may need to re-create tasks from your PRDs since the storage format changed.

## FAQ

### Q: Will my existing tasks be affected?

**A:** The storage format changed from JSON to SCG. SCUD can migrate existing JSON tasks, or you can re-parse your PRDs.

### Q: Do I need to update my PRDs or feature documents?

**A:** No. SCUD parses markdown the same way as Task Master.

### Q: What if I don't want the new parallel features?

**A:** Simply don't use them! The tag groups and task assignment features are completely optional. SCUD works exactly like Task Master if you ignore the new commands.

### Q: Can I use both tools?

**A:** Not recommended - they use different storage formats (`.taskmaster/` vs `.scud/`).

### Q: What about my API key?

**A:** Default provider changed to xAI:
```bash
export XAI_API_KEY="xai-..."
```

Alternative providers: Anthropic (`ANTHROPIC_API_KEY`), OpenAI (`OPENAI_API_KEY`), OpenRouter (`OPENROUTER_API_KEY`).

Configure with `scud config --provider <provider> --model <model>`.

### Q: Where did DETAILED_WALKTHROUGH.md go?

**A:** It was merged into the more comprehensive `COMPLETE_GUIDE.md` which covers everything in one place. Old implementation details were moved to `log_docs/` for reference.

## Performance Benchmarks

Measured on MacBook Pro M1, 16GB RAM:

| Operation | Task Master | SCUD | Improvement |
|-----------|-------------|------|-------------|
| Startup (`--help`) | 2,100ms | 42ms | **50x faster** |
| List tasks | 2,200ms | 45ms | **49x faster** |
| Parse PRD (10 tasks) | 32,000ms | 8,500ms | **3.8x faster** |
| Analyze complexity | 28,000ms | 7,200ms | **3.9x faster** |
| Show task details | 2,150ms | 38ms | **57x faster** |

Token usage (MCP vs direct API):

| Operation | Task Master | SCUD | Savings |
|-----------|-------------|------|---------|
| Parse PRD | ~85,000 tokens | ~2,000 tokens | **42x fewer** |
| Analyze complexity | ~62,000 tokens | ~1,500 tokens | **41x fewer** |

## Getting Help

- **Quick Start**: See `QUICKSTART.md`
- **Complete Reference**: See `COMPLETE_GUIDE.md`
- **Command Cheat Sheet**: See `QUICK_REFERENCE.md`
- **Parallel Features**: See `PARALLEL_FEATURES.md`
- **Issues**: https://github.com/pyrex41/scud/issues

## Summary

✅ **Same commands** - muscle memory preserved
✅ **Same workflow** - no retraining needed
✅ **50x faster** - dramatic performance improvement
✅ **New features** - optional parallel development tools
✅ **New storage format** - SCG for 75% token reduction

SCUD is a drop-in replacement that makes your existing workflow faster and more powerful.
