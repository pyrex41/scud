---
date: 2025-12-13T00:55:51Z
researcher: Claude
git_commit: 1fc3d77c9e73e79984f6467109b9cd133adf6b91
branch: master
repository: scud
topic: "SCUD README Legacy Instructions Audit"
tags: [research, documentation, readme, cli-commands]
status: complete
last_updated: 2025-12-12
last_updated_by: Claude
---

# Research: SCUD README Legacy Instructions Audit

**Date**: 2025-12-13T00:55:51Z
**Researcher**: Claude
**Git Commit**: 1fc3d77c9e73e79984f6467109b9cd133adf6b91
**Branch**: master
**Repository**: scud

## Research Question
Identify legacy/outdated instructions in the SCUD README and related documentation, specifically "serve instead of view" and incorrect default model.

## Summary

The README.md and QUICK_REFERENCE.md contain **multiple outdated references** from previous iterations of the CLI:

1. **`scud serve` should be `scud view`** - The web dashboard command was renamed
2. **Default model is wrong** - Documentation says `grok-3-mini`, actual default is `grok-code-fast-1`
3. **`claim` and `release` commands don't exist** - Task locking was removed
4. **`whois` should be `who-is`** - CLI uses hyphenated form
5. **`--port` option doesn't exist** - View command doesn't support custom ports
6. **`--count` should be `--limit`** - next-batch uses different flag name

## Detailed Findings

### Issue 1: `serve` vs `view` (HIGH PRIORITY)

**Actual command**: `scud view`

**README.md locations with outdated `serve`:**
- Line 48: `scud serve` in Basic Usage
- Line 104: `scud serve # Start web dashboard (port 3000)` in Web Dashboard section
- Line 152: `scud serve # Start web dashboard (port 3000)` in Commands > Visualization
- Line 210: `scud serve # Opens web dashboard with task graph` in Example Workflow

**QUICK_REFERENCE.md locations:**
- Line 25: `scud serve # Start web dashboard (port 3000)`
- Line 26: `scud serve --port 8080 # Custom port`
- Line 118: `scud serve # Web dashboard`

**Evidence**: CLI help shows `view` command, not `serve`:
```
Commands:
  ...
  view                Open interactive task viewer in browser
  ...
```

**Git history context**: Commit `314f48e` added "scud serve", later `9d5251d` renamed it to "scud view".

---

### Issue 2: Incorrect Default Model (HIGH PRIORITY)

**Documented**: `grok-3-mini` (README.md:163, QUICK_REFERENCE.md:41, 169)

**Actual**: `grok-code-fast-1`

**Evidence** from `scud-cli/src/config.rs:24`:
```rust
model: "grok-code-fast-1".to_string(),
```

And `scud-cli/src/config.rs:78-85`:
```rust
pub fn default_model_for_provider(provider: &str) -> &str {
    match provider {
        "xai" => "grok-code-fast-1",
        ...
        _ => "grok-code-fast-1",
    }
}
```

---

### Issue 3: Non-Existent `claim` and `release` Commands (HIGH PRIORITY)

**Documented in README.md:167-168**:
```bash
scud claim <id> --name <name>      # Claim task (lock)
scud release <id>                  # Release task lock
```

**Documented in QUICK_REFERENCE.md:45-46**:
```bash
scud claim <id> --name <name>      # Claim task (lock)
scud release <id>                  # Release task lock
```

**Reality**: These commands do NOT exist in the CLI.

**Evidence**:
```
$ scud claim --help
error: unrecognized subcommand 'claim'
  tip: a similar subcommand exists: 'clean'
```

**Git history context**: Commit `b44abf2` explicitly states "feat: remove task locking system for simplification"

---

### Issue 4: `whois` vs `who-is` (MEDIUM PRIORITY)

**Documented**: `scud whois [--tag <tag>]` (README.md:169, QUICK_REFERENCE.md:47)

**Actual CLI command**: `scud who-is [--tag <tag>]`

The CLI uses hyphenated form `who-is`, not `whois`.

---

### Issue 5: View Command Doesn't Support Port Option (MEDIUM PRIORITY)

**Documented in QUICK_REFERENCE.md:26**:
```bash
scud serve --port 8080             # Custom port
```

**Actual `view` command options**:
```
Open interactive task viewer in browser

Usage: scud view [OPTIONS]

Options:
  -p, --project <PROJECT>  Project root directory
  -h, --help               Print help
```

No `--port` option exists. The view command opens a temp HTML file in browser.

---

### Issue 6: `next-batch` Uses `--limit`, Not `--count` (LOW PRIORITY)

**Documented in QUICK_REFERENCE.md:50**:
```bash
scud next-batch [--count 5]        # Get multiple ready tasks
```

**Actual CLI**:
```
Options:
  -l, --limit <LIMIT>      Maximum number of tasks to return [default: 5]
```

---

## Code References

- `README.md:48,104,152,210` - `serve` references
- `README.md:163` - Wrong default model
- `README.md:167-169` - Non-existent claim/release/whois
- `docs/reference/QUICK_REFERENCE.md:25-26,41,45-50,118,169` - Various legacy references
- `scud-cli/src/main.rs:102-103` - Actual `View` command definition
- `scud-cli/src/config.rs:24,78-85` - Actual default model (`grok-code-fast-1`)

## Architecture Documentation

The CLI commands are defined in `scud-cli/src/main.rs` using clap's derive macros. The `Commands` enum at line 68 defines all available subcommands. Each command maps to a handler in `scud-cli/src/commands/`.

The task locking system (`claim`/`release`) was intentionally removed in commit `b44abf2` to simplify the codebase. Assignment is now handled by the `assign` command without locks.

## Summary of Required Fixes

| File | Line(s) | Issue | Fix |
|------|---------|-------|-----|
| README.md | 48, 104, 152, 210 | `serve` | Change to `view` |
| README.md | 104, 152 | `(port 3000)` | Remove port reference |
| README.md | 163 | `grok-3-mini` | Change to `grok-code-fast-1` |
| README.md | 167-168 | `claim`/`release` commands | Remove or note as deprecated |
| README.md | 169 | `whois` | Change to `who-is` |
| QUICK_REFERENCE.md | 25 | `serve` | Change to `view` |
| QUICK_REFERENCE.md | 26 | `serve --port` | Remove (no port option) |
| QUICK_REFERENCE.md | 41, 169 | `grok-3-mini` | Change to `grok-code-fast-1` |
| QUICK_REFERENCE.md | 45-46 | `claim`/`release` | Remove |
| QUICK_REFERENCE.md | 47 | `whois` | Change to `who-is` |
| QUICK_REFERENCE.md | 50 | `--count` | Change to `--limit` |
| QUICK_REFERENCE.md | 118 | `serve` | Change to `view` |

## Open Questions

1. Should the documentation mention the removed `claim`/`release` system, or simply omit it?
2. Is `assign` the intended replacement for task claiming/locking?
