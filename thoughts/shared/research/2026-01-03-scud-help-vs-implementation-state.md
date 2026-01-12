---
date: 2026-01-03T23:59:59Z
researcher: Claude
git_commit: 8bd6909
branch: master
repository: pyrex41/scud
topic: "SCUD Help vs Implementation State Analysis"
tags: [research, codebase, cli, commands, documentation]
status: complete
last_updated: 2026-01-03
last_updated_by: Claude
---

# Research: SCUD Help vs Implementation State Analysis

**Date**: 2026-01-03T23:59:59Z
**Researcher**: Claude
**Git Commit**: 8bd6909
**Branch**: master
**Repository**: pyrex41/scud

## Research Question

The user noted that `scud help` instructions don't match the current state of the code. This research documents the actual state of implemented commands vs documented commands.

## Summary

The SCUD CLI has **27 fully implemented commands** with no placeholder or stub implementations. However, the documentation contains references to **removed/deprecated commands** and **renamed commands** that create confusion:

1. **Removed Commands**: `claim`, `release`, `research` - documented but never existed or were removed
2. **Renamed Commands**: `serve` → `view`, `whois` → `who-is`, `use-tag` → `tags <tag>`
3. **Flag Mismatches**: `--count` → `--limit`, removed `--claim`/`--release` flags from `next`
4. **Underdocumented**: `config`, `migrate`, `convert`, `reanalyze-deps` exist but lack documentation

## Detailed Findings

### Actual Commands (from `scud help`)

The CLI exposes exactly these 27 commands:

| Command | Status | Description |
|---------|--------|-------------|
| `init` | Implemented | Initialize SCUD in current directory |
| `tags` | Implemented | List phase tags or set active tag |
| `list` | Implemented | List tasks in active phase |
| `view` | Implemented | Open interactive task viewer in browser |
| `show` | Implemented | Show detailed task information |
| `set-status` | Implemented | Update task status |
| `next` | Implemented | Find next available task |
| `stats` | Implemented | Show phase statistics |
| `migrate` | Implemented | Migrate task data to new format |
| `waves` | Implemented | Plan parallel execution waves |
| `config` | Implemented | Configuration management (with subcommands) |
| `parse` | Implemented | Parse PRD/phase markdown into tasks (AI) |
| `clean` | Implemented | Clear all tasks (with confirmation) |
| `analyze-complexity` | Implemented | Analyze task complexity (AI) |
| `expand` | Implemented | Expand complex task into subtasks (AI) |
| `reanalyze-deps` | Implemented | Re-analyze cross-tag dependencies (AI) |
| `assign` | Implemented | Assign task to a developer |
| `who-is` | Implemented | Show who is working on what |
| `next-batch` | Implemented | Get multiple ready tasks at once |
| `convert` | Implemented | Convert between JSON and SCG formats |
| `doctor` | Implemented | [EXPERIMENTAL] Diagnose stuck workflows |
| `mermaid` | Implemented | Generate Mermaid diagram |
| `log` | Implemented | Write a summary log entry for a task |
| `log-show` | Implemented | Show log entries for a task |
| `warmup` | Implemented | Quick orientation for new session |
| `commit` | Implemented | Create git commit with task context |
| `help` | Built-in | Print help message |

### Config Subcommands

```
scud config show              # Display current configuration
scud config set-provider      # Set LLM provider (xai, anthropic, openai, openrouter)
scud config agents list       # List installed SCUD agents
scud config agents add        # Add agents (pm, sm, architect, dev, retrospective, status)
scud config agents remove     # Remove agents
```

### Commands Documented But NOT Implemented

| Documented Command | Status | Notes |
|--------------------|--------|-------|
| `scud claim <id> --name <name>` | Never existed | Documented in orchestrator.md |
| `scud release <id>` | Never existed | Documented in orchestrator.md |
| `scud serve` | Renamed | Now `scud view` |
| `scud research "topic"` | Never existed | Mentioned in scud-cli/README.md |
| `scud use-tag <tag>` | Renamed | Now `scud tags <tag>` |
| `scud next --claim --name` | Removed | Dynamic-wave mode flags removed |
| `scud next --release --name` | Removed | Dynamic-wave mode flags removed |
| `scud whois` | Renamed | Now `scud who-is` (hyphenated) |
| `scud next-batch --count` | Renamed | Flag is now `--limit` |

### Flag Discrepancies

**`scud next` actual flags:**
```
-t, --tag <TAG>     Tag to search in
-s, --spawn         Output JSON for orchestrator spawning
```

**`scud next` documented but NOT implemented:**
```
--claim --name <name>   # Removed
--release --name <name> # Removed
```

**`scud next-batch` actual flags:**
```
-t, --tag <TAG>      Tag to search in
-l, --limit <LIMIT>  Maximum tasks to return [default: 5]
```

**`scud next-batch` documented but NOT implemented:**
```
--count <n>  # Should be --limit
```

### Implementation Verification

All 27 commands have complete implementations in `/scud-cli/src/commands/`:

- **No placeholder implementations** - searched for `todo!()`, `unimplemented!()`, "not implemented"
- **No deprecated markers** - no commands marked deprecated in code
- **One experimental command** - `doctor` is marked `[EXPERIMENTAL]` but fully functional
- **One blocked conversion** - `convert` blocks SCG→JSON (intentionally, CLI requires SCG format)

### Documentation Files Needing Updates

| File | Issues |
|------|--------|
| `/README.md` | References `claim`, `release`, `serve` |
| `/scud-cli/README.md` | References `use-tag`, `research`, `next --claim/--release` |
| `/docs/reference/QUICK_REFERENCE.md` | References `claim`, `release`, `serve`, `whois` |
| `/docs/orchestrator.md` | Extensively documents removed `claim`/`release` system |
| `/scud-cli/PROVIDERS.md` | References `.taskmaster/config.toml` (should be `.scud/config.toml`) |

### Commands Implemented But Underdocumented

These commands exist in CLI but lack comprehensive documentation:

1. **`scud config`** - Full subcommand tree not documented in user guides
2. **`scud migrate`** - Not in QUICK_REFERENCE.md
3. **`scud convert`** - Not in README commands section
4. **`scud reanalyze-deps`** - Not in QUICK_REFERENCE.md
5. **`scud log` / `scud log-show`** - Minimal documentation

## Code References

- Main command definitions: [`scud-cli/src/main.rs:67-360`](https://github.com/pyrex41/scud/blob/8bd6909/scud-cli/src/main.rs#L67-L360)
- Command dispatch: [`scud-cli/src/main.rs:363-468`](https://github.com/pyrex41/scud/blob/8bd6909/scud-cli/src/main.rs#L363-L468)
- Config subcommands: [`scud-cli/src/main.rs:6-52`](https://github.com/pyrex41/scud/blob/8bd6909/scud-cli/src/main.rs#L6-L52)
- Next command (actual flags): [`scud-cli/src/main.rs:126-135`](https://github.com/pyrex41/scud/blob/8bd6909/scud-cli/src/main.rs#L126-L135)
- Previous research on discrepancies: [`thoughts/shared/research/2025-12-12-scud-readme-legacy-instructions-audit.md`](https://github.com/pyrex41/scud/blob/8bd6909/thoughts/shared/research/2025-12-12-scud-readme-legacy-instructions-audit.md)

## Architecture Documentation

### Command Registration Pattern
- Uses `clap` derive macros with `#[derive(Parser)]` and `#[derive(Subcommand)]`
- Commands defined as enum variants in `Commands` enum
- Dispatch via single `match` expression in `main()`
- Async runtime via `tokio` for AI commands

### Storage Pattern
- All commands use `Storage::new(project_root)` for `.scud/` directory access
- Tag resolution via `helpers::resolve_group_tag()` with fallback chain
- Cross-tag dependencies via `helpers::flatten_all_tasks()`

### AI Command Pattern
- Located in `src/commands/ai/` module
- Use `LLMClient::complete_json()` with prompts from `llm/prompts.rs`
- Parallel execution via `futures::stream::iter().buffer_unordered()`
- Retry logic with 3 attempts

## Related Research

- [2025-12-12-scud-readme-legacy-instructions-audit.md](./2025-12-12-scud-readme-legacy-instructions-audit.md) - Previous audit identifying similar issues

## Open Questions

1. Should the task locking system (`claim`/`release`) be re-implemented, or should all documentation be updated to remove references?
2. Is the `research` command planned for future implementation?
3. Should `who-is` be aliased to `whois` for backwards compatibility?
