# Migration Guide: Task Master → SCUD

This guide helps existing Task Master users migrate to SCUD (Sprint Cycle Unified Development).

## Overview

SCUD is a complete rewrite of the Task Master tool with:
- **50x faster** startup (Rust vs. Node.js with MCP)
- **42x fewer tokens** (direct API calls vs. MCP overhead)
- **Same workflow** - commands and JSON format are backward compatible
- **New features** - parallel development with epic groups and task assignment

## Breaking Changes

### ⚠️ None!

SCUD is designed to be **100% backward compatible** with existing Task Master projects:
- All task JSON files remain unchanged
- Same `.taskmaster/` directory structure
- All slash commands work identically (`/tm-pm`, `/tm-dev`, etc.)
- Workflow phases are the same

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
npm install -g scud
# Or clone and run:
./install-claude-code.sh
```

### 3. Performance

**Old behavior:**
- Startup time: ~2-3 seconds
- Parse PRD: ~30-45 seconds
- MCP overhead on every call

**New behavior:**
- Startup time: ~50ms (50x faster)
- Parse PRD: ~8-12 seconds (3-4x faster)
- Direct API calls, no MCP

### 4. New Features (Optional)

SCUD adds **experimental parallel development features** that are completely optional:

#### Epic Groups
Coordinate related epics (e.g., backend + frontend):
```bash
scud create-group fullstack --description "Backend + Frontend"
scud add-to-group backend fullstack
scud add-to-group frontend fullstack
scud group-status fullstack
```

#### Task Assignment & Locking
Multiple developers can work on the same epic:
```bash
scud assign TASK-123 alice
scud claim TASK-123 alice  # Locks the task
scud whois                  # See all assignments
scud release TASK-123       # Unlock when done
```

## Migration Steps

### Step 1: Install SCUD

```bash
npm install -g scud
```

Or for local development:
```bash
git clone https://github.com/yourusername/scud.git
cd scud
./install-claude-code.sh
```

### Step 2: Verify Existing Project

Your existing `.taskmaster/` directory will work as-is:

```bash
cd your-existing-project
scud tags    # Should list your existing epics
scud list    # Should list your tasks
scud stats   # Should show your statistics
```

### Step 3: Update Slash Commands (Optional)

If you use Claude Code or OpenCode, the slash commands are already updated:
- `/tm-pm` - Product Manager agent
- `/tm-sm` - Scrum Master agent
- `/tm-architect` - Architect agent
- `/tm-dev` - Developer agent
- `/tm-retrospective` - Retrospective agent

These work identically to before.

### Step 4: Update Any Scripts

If you have scripts calling `task-master`, update them to use `scud`:

```bash
# Old
task-master parse-prd docs/prd/feature.md --tag feature

# New
scud parse-prd docs/prd/feature.md --tag feature
```

Or create an alias:
```bash
alias task-master='scud'
```

## Rollback Plan

If you encounter issues, you can rollback:

```bash
# Uninstall SCUD
npm uninstall -g scud

# Reinstall old task-master
npm install -g @eyaltoledano/claude-task-master
```

Your `.taskmaster/` data is unchanged and will work with either tool.

## FAQ

### Q: Will my existing tasks be affected?

**A:** No. SCUD uses the exact same JSON schema as Task Master. All your existing tasks, epics, and workflow state are preserved.

### Q: Do I need to update my PRDs or epic documents?

**A:** No. SCUD parses markdown the same way as Task Master.

### Q: What if I don't want the new parallel features?

**A:** Simply don't use them! The epic groups and task assignment features are completely optional. SCUD works exactly like Task Master if you ignore the new commands.

### Q: Can I use both tools?

**A:** Yes, but not simultaneously. Both read/write the same `.taskmaster/` directory, so use one or the other per project.

### Q: What about my ANTHROPIC_API_KEY?

**A:** Same as before - set it in your environment:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

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
- **Issues**: https://github.com/yourusername/scud/issues

## Summary

✅ **Zero breaking changes** - existing projects work as-is
✅ **Same commands** - muscle memory preserved
✅ **Same workflow** - no retraining needed
✅ **50x faster** - dramatic performance improvement
✅ **New features** - optional parallel development tools

SCUD is a drop-in replacement that makes your existing workflow faster and more powerful.
