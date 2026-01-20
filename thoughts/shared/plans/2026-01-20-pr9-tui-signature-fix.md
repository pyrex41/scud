# PR #9 Fix: Update tui::run Call Signature

## Overview

Fix PR #9 (`claude/test-monitor-feature-OJEwO`) by updating the `tui::run` call in the swarm module to match the updated function signature that includes the optional feed endpoint parameter.

## Current State Analysis

PR #9 adds a `--monitor` flag to the swarm command and creates a `MonitorableSession` trait. However, there's an inconsistency:

- `tui::run` in `mod.rs:28` has signature: `pub fn run(project_root: Option<PathBuf>, session_name: &str) -> Result<()>`
- But PR #10 (which adds socket feed) changes this to: `pub fn run(project_root: Option<PathBuf>, session_name: &str, feed_endpoint: Option<String>) -> Result<()>`
- The swarm module call at line 210 uses the old 2-argument signature

### Key Discoveries:
- `scud-cli/src/commands/swarm/mod.rs:210` calls `tui::run(project_root_clone, &session_name_clone)`
- This needs to be `tui::run(project_root_clone, &session_name_clone, None)` to match PR #10's signature
- PR #9 and PR #10 have interdependent changes

## Desired End State

The swarm module correctly calls `tui::run` with all three arguments, passing `None` for the feed endpoint (swarm doesn't expose the `--feed` flag yet).

### Verification:
- Code compiles when both PR #9 and PR #10 are merged
- `scud swarm --monitor` works correctly

## What We're NOT Doing

- Not adding `--feed` support to swarm command (that can be a future enhancement)
- Not modifying PR #10's changes
- Not changing any other aspects of PR #9

## Implementation Approach

Update the single call site in swarm/mod.rs to include the third argument.

## Phase 1: Fix tui::run Call

### Overview
Update the tui::run call to include the feed_endpoint parameter as None.

### Changes Required:

#### 1.1 Update swarm/mod.rs

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add third argument to tui::run call

**Current code (line ~210):**
```rust
std::thread::spawn(move || {
    let _ = tui::run(project_root_clone, &session_name_clone);
});
```

**Updated code:**
```rust
std::thread::spawn(move || {
    let _ = tui::run(project_root_clone, &session_name_clone, None);
});
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds with both PR #9 and PR #10 changes
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no new warnings

#### Manual Verification:
- [ ] `scud swarm --tag test --monitor --dry-run` shows TUI launching message
- [ ] Monitor TUI appears when running swarm with real tasks

---

## Implementation Note

This fix should be applied to the PR #9 branch after PR #10 is merged, OR both PRs need to be coordinated so they're compatible. The simplest approach:

1. Merge PR #10 first (socket feed)
2. Rebase PR #9 on updated master
3. Apply this fix during rebase conflict resolution

Alternatively, apply this fix to PR #9 now and coordinate the merge order.

## Testing Strategy

### Automated:
- Build succeeds
- All tests pass

### Manual:
- Test swarm with monitor flag

## References

- PR #9: https://github.com/pyrex41/scud/pull/9
- PR #10: https://github.com/pyrex41/scud/pull/10
- Branch: `claude/test-monitor-feature-OJEwO`
