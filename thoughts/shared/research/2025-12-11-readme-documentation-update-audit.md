---
date: 2025-12-11T10:00:00-05:00
researcher: Claude (Opus 4.5)
git_commit: 314f48e76ef06c5524848accf35565e174158df1
branch: master
repository: scud
topic: "README and Documentation Update Audit - Current State vs Documentation"
tags: [research, documentation, readme, audit, simplification]
status: complete
last_updated: 2025-12-11
last_updated_by: Claude
---

# Research: README and Documentation Update Audit

**Date**: 2025-12-11
**Researcher**: Claude (Opus 4.5)
**Git Commit**: 314f48e76ef06c5524848accf35565e174158df1
**Branch**: master
**Repository**: scud

## Research Question

Audit the SCUD codebase to identify discrepancies between documentation and actual implementation. User wants to:
1. Add SCUD missile system inspiration note at top of README
2. Update documentation to reflect simplifications made
3. Remove references to features that no longer exist

## Executive Summary

The documentation is significantly out of sync with the actual implementation. Major findings:

1. **Commands that DON'T exist** (but are extensively documented):
   - `scud hooks install/status/uninstall` - Never implemented
   - `scud status` - Only `scud set-status` exists
   - `scud research` - Not a CLI command
   - `scud use-tag` - Use `scud tags <tag>` instead

2. **Outdated concepts** still in documentation:
   - 5-phase workflow (Ideation → Planning → Architecture → Implementation → Retrospective)
   - `/tm-pm`, `/tm-sm`, `/tm-architect`, `/tm-dev` slash commands
   - `workflow-state.json` file
   - "epic" terminology (now "tags" or "phases")

3. **New features** not well documented:
   - `scud serve` - Web dashboard with Mermaid visualization
   - `scud log/log-show` - Task logging
   - `scud warmup` - Session orientation
   - `scud commit` - Git commit with task context
   - `scud mermaid` - Diagram generation
   - `scud clean` - Clear all tasks
   - `scud convert`, `scud migrate`, `scud next-batch`, `scud reanalyze-deps`

---

## Detailed Findings

### Section 1: Commands That DON'T Exist (Remove from Docs)

#### 1.1 `scud hooks install/status/uninstall`

**Status**: PLANNED BUT NEVER IMPLEMENTED

**Documentation references** (to remove):
- `README.md`: Lines 20, 28, 74, 242, 243, 280
- `docs/orchestrator.md`: Lines 15, 41, 142, 188, 235, 303, 359, 366, 369, 505, 517
- `docs/guides/MIGRATION.md`: Line 116

**CLI test result**:
```
error: unrecognized subcommand 'hooks'
tip: a similar subcommand exists: 'who-is'
```

**Recommendation**: Remove all references to `scud hooks install`. The hook integration works through manual `.claude/settings.local.json` configuration, not a CLI command.

---

#### 1.2 `scud status` (standalone)

**Status**: NEVER IMPLEMENTED - Only `scud set-status` exists

**Documentation references** (to correct):
- `docs/guides/COMPLETE_GUIDE.md`: Lines 658-667 (shows `scud status` command)
- `docs/reference/QUICK_REFERENCE.md`: Line 24

**CLI test result**:
```
error: unrecognized subcommand 'status'
tip: some similar subcommands exist: 'tags', 'set-status', 'stats'
```

**Recommendation**: Remove `scud status` references. Users should use `scud stats` for statistics or `scud tags` for active tag info.

---

#### 1.3 `scud research`

**Status**: NOT A CLI COMMAND

**Clarification**: This exists only as a Claude Code slash command (`/research_codebase`), not as a CLI command.

**Documentation references** (to remove):
- `docs/guides/COMPLETE_GUIDE.md`: Lines 924-978 (entire section documenting `scud research "<query>"`)
- `docs/reference/QUICK_REFERENCE.md`: Line 64

**CLI test result**:
```
error: unrecognized subcommand 'research'
```

**Recommendation**: Remove the `scud research` section entirely. It was never implemented in the Rust CLI.

---

#### 1.4 `scud use-tag`

**Status**: DOES NOT EXIST - Use `scud tags <tag>` instead

**Documentation references** (to correct):
- `docs/guides/COMPLETE_GUIDE.md`: Lines 686-697
- `docs/reference/QUICK_REFERENCE.md`: Lines 30-31

**Actual command**: `scud tags <tag-name>` sets the active tag

**Recommendation**: Replace `scud use-tag` with `scud tags <tag>` throughout documentation.

---

### Section 2: Outdated Concepts (Simplify or Remove)

#### 2.1 Five-Phase Workflow

The documentation extensively describes a 5-phase workflow that is no longer enforced:

```
Ideation → Planning → Architecture → Implementation → Retrospective
```

**Current reality**: SCUD is now a simpler DAG-based task manager. The workflow phases are optional conventions, not enforced by the CLI. The `workflow-state.json` file is no longer used.

**Files affected**:
- `docs/guides/COMPLETE_GUIDE.md`: Entire structure based on 5 phases (1,800+ lines)
- `docs/reference/QUICK_REFERENCE.md`: Phase workflow table
- `README.md`: References to phase workflow in places

**Recommendation**:
- Simplify to describe SCUD as a "DAG-driven task manager"
- Remove phase enforcement language
- Keep phases as an optional organizational pattern, not a requirement

---

#### 2.2 Agent Slash Commands (`/tm-pm`, `/tm-sm`, etc.)

**Status**: NEVER CREATED - Agent files don't exist

The code infrastructure exists in `scud-cli/src/commands/config.rs` but the actual agent markdown files were never created:
- `/tm-pm` - Product Manager
- `/tm-sm` - Scrum Master
- `/tm-architect` - Architect
- `/tm-dev` - Developer
- `/tm-retrospective` - Retrospective
- `/status` - Status reporting

**Files affected**:
- `docs/guides/COMPLETE_GUIDE.md`: Extensive agent documentation (lines 143-150, 986-1122)
- `docs/reference/QUICK_REFERENCE.md`: Agent cheat sheet (lines 154-193)
- `docs/guides/MIGRATION.md`: Lines 134-138

**Recommendation**: Remove agent slash command documentation. They were planned but never implemented.

---

#### 2.3 `workflow-state.json`

**Status**: DEPRECATED - Replaced by `.scud/active-tag` plain text file

The Rust CLI never reads `workflow-state.json`. Active tag is stored as plain text in `.scud/active-tag`.

**Files affected**:
- `docs/guides/COMPLETE_GUIDE.md`: References workflow state
- File structure diagrams showing `workflow-state.json`

**Recommendation**: Update file structure diagrams to show actual `.scud/` structure:
```
.scud/
├── tasks/tasks.scg
├── config.toml
├── active-tag
├── current-task
└── logs/
```

---

#### 2.4 "Epic" Terminology

**Status**: Replaced with "tag" or "phase"

The codebase now uses "tag" or "phase" consistently. "Epic" is legacy terminology from the original design.

**Recommendation**: Replace "epic" with "tag" throughout documentation, or explicitly note that they're synonymous.

---

### Section 3: New Features to Document

#### 3.1 `scud serve` - Web Dashboard

**Implementation**: `scud-cli/src/commands/serve.rs` (559 lines)

**Features**:
- Starts local web server (default port 3000)
- HTML dashboard with task table
- Live Mermaid.js dependency graph visualization
- Dark theme, client-side filtering
- Auto-opens browser (disable with `--no-open`)

**Usage**:
```bash
scud serve                    # Start on port 3000, auto-open browser
scud serve --port 8080        # Custom port
scud serve --no-open          # Don't open browser
```

**Current documentation**: None in user-facing docs

---

#### 3.2 `scud log` / `scud log-show` - Task Logging

**Implementation**: `scud-cli/src/commands/log.rs` (76 lines)

**Features**:
- Append timestamped log entries to tasks
- Stores in `.scud/logs/<task-id>.log`
- View log history per task

**Usage**:
```bash
scud log <task-id> "Implemented user validation logic"
scud log-show <task-id>
```

**Current documentation**: None

---

#### 3.3 `scud warmup` - Session Orientation

**Implementation**: `scud-cli/src/commands/warmup.rs` (144 lines)

**Features**:
- Shows working directory
- Recent git commits (last 5)
- Active tag and progress percentage
- Current task assignments
- Next available task

**Usage**:
```bash
scud warmup
```

**Current documentation**: Only in `.opencode/hook/session-start.md`

---

#### 3.4 `scud commit` - Git Commit with Task Context

**Implementation**: `scud-cli/src/commands/commit.rs` (176 lines)

**Features**:
- Creates git commits with task ID prefix
- Uses `SCUD_TASK_ID` env var or `.scud/current-task` file
- Auto-stages files with `--all` flag

**Usage**:
```bash
scud commit                              # Uses task title as message
scud commit -m "Fix validation bug"      # Custom message
scud commit --all                        # Stage all changes first
```

**Current documentation**: None

---

#### 3.5 `scud mermaid` - Diagram Generation

**Implementation**: `scud-cli/src/commands/mermaid.rs` (128 lines)

**Features**:
- Generates Mermaid flowchart syntax
- Status-based node shapes and colors
- Dependency and parent-child edges
- Multi-tag support with subgraphs

**Usage**:
```bash
scud mermaid                 # Active tag only
scud mermaid --tag auth      # Specific tag
scud mermaid --all-tags      # All phases
```

**Current documentation**: Mentioned in planning docs only

---

#### 3.6 `scud clean` - Clear Tasks

**Implementation**: `scud-cli/src/commands/clean.rs` (91 lines)

**Features**:
- Clear all tasks from a tag or all tags
- Interactive confirmation (bypass with `--force`)

**Usage**:
```bash
scud clean                    # Clear all tasks (with confirmation)
scud clean --tag auth         # Clear specific tag
scud clean --force            # Skip confirmation
```

**Current documentation**: None

---

#### 3.7 Other New Commands

| Command | Purpose | Documentation |
|---------|---------|---------------|
| `scud convert` | Convert between JSON and SCG formats | None |
| `scud migrate` | Migrate task data to new format | None |
| `scud next-batch` | Get multiple ready tasks for orchestrators | None |
| `scud reanalyze-deps` | AI-powered cross-tag dependency analysis | None |
| `scud doctor` | Diagnose stuck workflow states | Mentioned in QUICK_REFERENCE |

---

### Section 4: README.md Specific Updates Needed

#### 4.1 Add SCUD Missile Inspiration Header

User requested adding a note that SCUD is inspired by the SCUD short-range ballistic missile system - lightweight, flexible, but powerful, usable in a variety of contexts.

**Suggested addition** (at top of README, after title):

```markdown
# SCUD

> *Inspired by the SCUD short-range ballistic missile system—lightweight, flexible, and powerful. Like its namesake, SCUD can be deployed quickly in a variety of contexts, delivering results with minimal overhead.*

**Sprint Cycle Unified Development** - Fast, AI-powered task management for building software
```

---

#### 4.2 Quick Start Section Updates

**Current** (incorrect):
```bash
scud hooks install  # Enable automatic task completion
```

**Should be**:
```bash
# Hooks are configured manually in .claude/settings.local.json
# See docs/orchestrator.md for hook configuration
```

---

#### 4.3 Basic Usage Updates

**Current** (incorrect):
```bash
scud parse-prd docs/feature.md --tag my-feature
```

**Should be**:
```bash
scud parse docs/feature.md --tag my-feature  # 'parse' is the primary name now
```

---

#### 4.4 Commands Section Updates

Remove from README:
- `scud hooks install`
- `scud hooks status`

Add to README:
- `scud serve` - Start web dashboard
- `scud log` - Write task log entry
- `scud warmup` - Session orientation
- `scud commit` - Git commit with task context
- `scud mermaid` - Generate dependency diagram
- `scud clean` - Clear all tasks

---

#### 4.5 Mode 2: MCP Server Section

Update the MCP server section to note that some documented tools may not be implemented:
- `scud_research` - Not implemented
- `scud_create_group`, `scud_list_groups`, `scud_group_status` - May not be fully implemented

---

### Section 5: Other Documentation Files to Update

| File | Priority | Changes Needed |
|------|----------|----------------|
| `docs/guides/COMPLETE_GUIDE.md` | HIGH | Major rewrite needed - 80% outdated |
| `docs/reference/QUICK_REFERENCE.md` | HIGH | Remove outdated commands, add new ones |
| `docs/orchestrator.md` | MEDIUM | Remove `hooks install` references |
| `docs/guides/MIGRATION.md` | LOW | Update agent references |
| `scud-mcp/README.md` | MEDIUM | Update tool list, remove `scud_research` |
| `scud-cli/README.md` | LOW | Generally accurate |

---

## Recommended Action Plan

### Phase 1: Critical README Updates
1. Add SCUD missile inspiration header
2. Remove all `scud hooks install` references
3. Change `parse-prd` to `parse` in examples
4. Add new commands section (serve, log, warmup, etc.)

### Phase 2: Quick Reference Updates
1. Remove non-existent commands (status, use-tag, research, hooks)
2. Add new commands with usage examples
3. Update file structure diagram

### Phase 3: Complete Guide Overhaul (Optional)
The COMPLETE_GUIDE.md is 1,800+ lines and largely obsolete. Options:
- **Option A**: Delete and replace with simpler documentation
- **Option B**: Add deprecation notice and create new guide
- **Option C**: Major rewrite to reflect current state

Recommendation: Option A - The current guide creates more confusion than value.

---

## Code References

### CLI Command Implementations
- `scud-cli/src/main.rs:68-361` - Commands enum (source of truth)
- `scud-cli/src/commands/mod.rs:1-26` - Module exports

### Storage Layer
- `scud-cli/src/storage/mod.rs:242-287` - Active tag handling
- `.scud/active-tag` - Plain text active tag file (not workflow-state.json)

### Verified Non-Existent Commands
- No `hooks` module in `src/commands/`
- No `status` command (only `set_status`)
- No `research` command
- No `use_tag` command (handled by `tags` command)

---

## Summary

The SCUD documentation needs significant updates to match the actual implementation. The core CLI is simpler and more focused than what the documentation describes. Key changes:

1. **Remove**: hooks commands, standalone status, research, use-tag, 5-phase workflow enforcement, agent slash commands
2. **Add**: serve, log, warmup, commit, mermaid, clean, and other new commands
3. **Update**: terminology (epic → tag), file structure, command names (parse-prd → parse)
4. **Highlight**: The SCUD missile inspiration at the top of the README
