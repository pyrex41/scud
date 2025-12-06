---
date: 2025-12-06
author: Claude
status: in_progress
ticket: N/A
title: SCUD Locking System Removal
---

# Plan: SCUD Locking System Removal

## Overview

Remove the task claiming/locking system from SCUD to simplify the codebase. Tasks become read-only reference context rather than a tracking system.

## Goals

- Remove `scud claim` and `scud release` commands
- Remove `locked_by`, `locked_at` fields from Task model
- Remove `@assignments` section from SCG format (or simplify to only `assigned_to` if kept)
- Simplify `scud next` to remove `--claim`/`--release` modes
- Remove `scud sessions` command
- Update `scud doctor` to remove stale lock detection
- Update `scud set-status` to remove auto-release logic
- Keep `assigned_to` field for informational purposes (optional soft assignment)

## Non-Goals

- Remove task dependencies (keep DAG structure)
- Remove wave computation
- Remove complexity estimation
- Change SCG format fundamentals

---

## Phase 1: Remove Lock Fields from Task Model

### Files to Modify
- `scud-cli/src/models/task.rs`

### Changes

- [ ] Remove `locked_by: Option<String>` field (line 112)
- [ ] Remove `locked_at: Option<String>` field (line 115)
- [ ] Remove `claim()` method (lines 353-365)
- [ ] Remove `release()` method (lines 367-371)
- [ ] Remove `is_locked()` method (lines 373-375)
- [ ] Remove `is_locked_by()` method (lines 377-382)
- [ ] Remove `lock_age_hours()` method (lines 391-401)
- [ ] Remove `is_stale_lock()` method (lines 403-407)
- [ ] Remove related tests

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes (with updated/removed tests)

---

## Phase 2: Remove claim/release Commands

### Files to Modify
- `scud-cli/src/commands/claim.rs` - DELETE
- `scud-cli/src/commands/release.rs` - DELETE
- `scud-cli/src/commands/mod.rs` - Remove exports
- `scud-cli/src/main.rs` - Remove command definitions and routing

### Changes

- [ ] Delete `commands/claim.rs`
- [ ] Delete `commands/release.rs`
- [ ] Remove `pub mod claim;` from `commands/mod.rs`
- [ ] Remove `pub mod release;` from `commands/mod.rs`
- [ ] Remove `Commands::Claim` variant from main.rs
- [ ] Remove `Commands::Release` variant from main.rs
- [ ] Remove claim/release match arms in main()

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes

---

## Phase 3: Update SCG Format

### Files to Modify
- `scud-cli/src/formats/scg.rs`

### Changes

- [ ] Remove `locked_by` and `locked_at` from @assignments parsing (lines 248-270)
- [ ] Simplify to just `id | assigned_to` format
- [ ] Update serialization (lines 455-474) to only write assigned_to
- [ ] Update or remove assignment-related tests

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] Existing SCG files with @assignments still parse (backward compat)

---

## Phase 4: Simplify next Command

### Files to Modify
- `scud-cli/src/commands/next.rs`
- `scud-cli/src/main.rs`

### Changes

- [ ] Remove `--claim` flag from CLI definition
- [ ] Remove `--release` flag from CLI definition
- [ ] Remove `--name` flag from CLI definition (no longer needed)
- [ ] Remove `handle_claim()` function (lines 136-296)
- [ ] Remove `handle_release()` function (lines 298-346)
- [ ] Simplify `find_next_available()` - remove `exclude_locked` parameter
- [ ] Remove `NextTaskResult::AllLocked` variant
- [ ] Update command routing in main.rs

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `scud next` still finds next available task

---

## Phase 5: Update set-status and doctor

### Files to Modify
- `scud-cli/src/commands/set_status.rs`
- `scud-cli/src/commands/doctor.rs`

### Changes in set_status.rs
- [ ] Remove auto-release logic (lines 32-38)
- [ ] Remove "(lock released)" messaging

### Changes in doctor.rs
- [ ] Remove stale lock detection (lines 156-166)
- [ ] Remove `stale_locks` field from DiagnosticResults
- [ ] Remove stale lock auto-fix logic (lines 248-263)
- [ ] Remove `--stale-hours` CLI flag
- [ ] Keep orphan in-progress detection (useful without locks)

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes

---

## Phase 6: Remove sessions and Update whois

### Files to Modify
- `scud-cli/src/commands/sessions.rs` - DELETE
- `scud-cli/src/commands/whois.rs` - Simplify
- `scud-cli/src/commands/mod.rs`
- `scud-cli/src/main.rs`

### Changes

- [ ] Delete `commands/sessions.rs`
- [ ] Remove `pub mod sessions;` from mod.rs
- [ ] Remove `Commands::Sessions` from main.rs
- [ ] Update `whois.rs` to only show `assigned_to` (remove stale lock warnings)

### Success Criteria
- [ ] `cargo check` passes
- [ ] `cargo test` passes

---

## Phase 7: Update Help and Documentation

### Files to Modify
- `bin/scud.js`
- `.claude/commands/scud/task-claim.md` - DELETE
- Any other command docs referencing claim/release

### Changes

- [ ] Remove claim/release from scud.js help
- [ ] Delete task-claim.md if exists
- [ ] Update any references to locking in docs

### Success Criteria
- [ ] `node bin/scud.js help` shows updated commands

---

## Final Verification

- [ ] `cargo check` passes
- [ ] `cargo test` passes (all 125+ tests)
- [ ] `scud next` works
- [ ] `scud set-status <id> done` works without lock references
- [ ] `scud doctor` works without stale lock detection
- [ ] `scud whois` shows assignments only
- [ ] Old SCG files with @assignments section still parse

## Rollback Plan

If issues arise:
1. Git revert the commits
2. The lock fields are optional (serde skip_serializing_if), so old data still works
