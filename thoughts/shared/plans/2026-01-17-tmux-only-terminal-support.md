# Tmux-Only Terminal Support Implementation Plan

## Overview

Remove all terminal multiplexer support except tmux from SCUD's spawn system. This simplifies the codebase, reduces maintenance burden, and provides a more reliable agent spawning experience. Tmux is the only terminal that supports true detached operation, making it the ideal choice for background agent work.

## Current State Analysis

SCUD currently supports 5 terminal environments:
- **Tmux** - Full support with detached sessions, control window, attach functionality
- **Zellij** - Pane-based spawning, partial detached support
- **WezTerm** - Window spawning, no detached mode
- **Kitty** - Window spawning, no detached mode
- **iTerm2** - AppleScript-based window spawning (macOS only), no detached mode

### Key Discoveries:
- Terminal enum with 5 variants: `scud-cli/src/commands/spawn/terminal.rs:196-216`
- Auto-detection logic: `scud-cli/src/commands/spawn/terminal.rs:218-242`
- ~400 lines of spawn functions for non-tmux terminals
- ~50 lines of Zellij-specific attach functions
- CLI `--terminal` flag in spawn and swarm commands: `main.rs:555-557`, `main.rs:616-618`
- Test mock infrastructure with duplicate terminal types: `tests/e2e/mock_terminal.rs`

## Desired End State

- Only tmux is supported for spawning agents
- The `--terminal` flag is removed from CLI (tmux is always used)
- Codebase is ~500 lines lighter
- Simpler, more predictable agent spawning behavior
- All tests pass with simplified mock infrastructure

## What We're NOT Doing

- Adding new tmux features (that's a separate effort)
- Changing the prompt generation or agent system
- Modifying session metadata storage
- Changing the swarm orchestration logic

## Implementation Approach

Remove code in a single phase since all changes are tightly coupled. The Terminal enum, spawn functions, CLI args, and tests all need to be updated together to maintain consistency.

## Phase 1: Remove Non-Tmux Terminal Support

### Overview
Remove all terminal variants except Tmux, simplify the spawn logic, update CLI, and fix tests.

### Changes Required:

#### 1.1 Simplify Terminal Enum and Remove Detection

**File**: `scud-cli/src/commands/spawn/terminal.rs`

**Changes**:
1. Remove `Terminal` enum entirely (or keep as single-variant for future extensibility)
2. Remove `detect_terminal()` function
3. Remove `parse_terminal()` function
4. Remove `spawn_kitty()` function (~35 lines)
5. Remove `spawn_wezterm()` function (~35 lines)
6. Remove `spawn_iterm2()` function (~60 lines)
7. Remove `spawn_zellij()` function (~105 lines)
8. Remove `focus_zellij_pane()` function (~35 lines)
9. Remove `zellij_session_exists()` function (~10 lines)
10. Update `spawn_terminal()` to call `spawn_tmux()` directly
11. Update `spawn_terminal_with_harness()` to call `spawn_tmux()` directly
12. Update `spawn_terminal_with_harness_and_model()` to call `spawn_tmux()` directly
13. Update `spawn_terminal_ralph()` and `spawn_terminal_ralph_with_harness()` similarly
14. Remove `check_terminal_available()` function (or simplify to only check tmux)

#### 1.2 Update Spawn Command

**File**: `scud-cli/src/commands/spawn/mod.rs`

**Changes**:
1. Remove `terminal_arg` parameter from `run()` function
2. Remove `parse_terminal()` call
3. Remove `check_terminal_available()` call
4. Remove terminal-specific conditional logic (e.g., `if terminal == Terminal::Tmux`)
5. Always use tmux for spawning
6. Update `SpawnSession::new()` to hardcode "tmux" for terminal field

#### 1.3 Update Swarm Command

**File**: `scud-cli/src/commands/swarm/mod.rs`

**Changes**:
1. Remove `--terminal` flag handling
2. Remove terminal parsing logic
3. Always use tmux for spawning

#### 1.4 Update CLI Arguments

**File**: `scud-cli/src/main.rs`

**Changes**:
1. Remove `--terminal` / `-T` argument from `Spawn` command
2. Remove `--terminal` / `-T` argument from `Swarm` command
3. Update help text to reflect tmux-only support

#### 1.5 Simplify Test Mocks

**File**: `scud-cli/tests/e2e/mock_terminal.rs`

**Changes**:
1. Simplify `TerminalType` enum to just `Tmux` and `Mock`
2. Remove `Kitty`, `ITerm2`, `VSCode`, `WezTerm` variants
3. Update `MockTerminalDetector::detect_available()` to only return `Tmux`, `Mock`
4. Update tests that use other terminal types

#### 1.6 Update Multi-Agent Tests

**File**: `scud-cli/tests/user_stories/multi_agent.rs`

**Changes**:
1. Remove any references to non-tmux terminals
2. Update test expectations if needed

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud-cli`
- [x] All tests pass: `cargo test -p scud-cli`
- [x] Clippy passes: `cargo clippy -p scud-cli -- -D warnings`
- [x] Format check passes: `cargo fmt -p scud-cli --check`

#### Manual Verification:
- [x] `scud spawn --help` no longer shows `--terminal` option
- [x] `scud swarm --help` no longer shows `--terminal` option
- [x] `scud spawn --limit 1 --dry-run` shows tmux as the terminal
- [ ] Spawning agents works with tmux: `scud spawn --limit 1` (requires tmux installed, user verification)

---

## Testing Strategy

### Unit Tests:
- Existing spawn tests should continue to pass
- Mock terminal tests should work with simplified enum

### Integration Tests:
- E2E tests using mock terminal should pass
- Multi-agent user story tests should pass

### Manual Testing Steps:
1. Run `scud spawn --help` and verify no `--terminal` flag
2. Run `scud spawn --limit 1 --dry-run` and verify output shows tmux
3. If tmux is installed, run `scud spawn --limit 1` and verify agent spawns in tmux session

## Performance Considerations

This is a simplification that removes code. No performance impact expected.

## Migration Notes

Users who relied on `--terminal kitty` or similar flags will need to switch to tmux. Since tmux works universally (doesn't require being "inside" a specific terminal), this should be straightforward.

## References

- Research: `thoughts/shared/research/2026-01-17-terminal-multiplexer-detached-sessions.md`
- Terminal implementation: `scud-cli/src/commands/spawn/terminal.rs`
