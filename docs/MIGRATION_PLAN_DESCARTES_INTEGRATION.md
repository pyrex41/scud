# Plan: SCUD Migration for Descartes Integration

## Overview

Prepare SCUD for cleaner integration with Descartes by deprecating the `swarm` command, exposing backpressure utilities as a public API, and ensuring the library interface is complete and well-documented.

## Prerequisites

Read [UNIFIED_CONTEXT.md](/Users/reuben/projects/harnesses/docs/UNIFIED_CONTEXT.md) first for shared context.

## Current State Analysis

### What SCUD Has Today

**Task Management (Keep):**
- Task storage in SCG format (`src/storage/mod.rs`)
- Task model with status, dependencies, complexity (`src/models/task.rs`)
- Wave computation (`src/commands/waves.rs`)
- Task status updates (`src/commands/set_status.rs`)

**AI Commands (Keep):**
- PRD parsing (`src/commands/ai/parse_prd.rs`)
- Task expansion (`src/commands/ai/expand.rs`)
- Complexity analysis (`src/commands/ai/analyze_complexity.rs`)

**Agent Spawning (Keep `spawn`, Deprecate `swarm`):**
- `spawn` command: Fire-and-forget agent spawning (`src/commands/spawn/mod.rs`)
- `swarm` command: Wave-based orchestration loop (`src/commands/swarm/mod.rs`) **→ DEPRECATE**

**Utilities (Keep):**
- Git integration (`src/commands/commit.rs`)
- TUI monitor (`src/commands/spawn/tui/`)
- Backpressure validation (`src/commands/swarm/backpressure.rs`) **→ EXPOSE AS PUBLIC**

### Library Exports Today

```rust
// src/lib.rs (current)
pub mod commands;
pub mod config;
pub mod formats;
pub mod llm;
pub mod models;
pub mod storage;
```

The `commands` module is exported, but `swarm::backpressure` is not explicitly documented as a public API.

## Desired End State

1. `scud swarm` shows deprecation warning pointing to `descartes ralph`
2. Backpressure validation is a clean, reusable public API
3. Library exports are documented with examples
4. `scud spawn` remains for simple fire-and-forget use

## Implementation Approach

Small, incremental changes with backward compatibility. No breaking changes to the library API.

---

## Phase 1: Add Deprecation Warning to Swarm Command

**Goal**: Alert users that `scud swarm` is moving to Descartes, without breaking existing usage.

**Changes**:

- [ ] Add deprecation warning at start of `run()` function (`src/commands/swarm/mod.rs:39`)

```rust
// Add at the start of run() function, after parameter validation
eprintln!(
    "{}: The 'scud swarm' command is deprecated and will be removed in v2.0.",
    "Warning".yellow().bold()
);
eprintln!(
    "         Use 'descartes ralph --scud-tag {}' instead for wave-based execution.",
    tag.unwrap_or("<tag>")
);
eprintln!(
    "         See: https://github.com/pyrex41/descartes#ralph-command"
);
eprintln!();
```

- [ ] Add `#[deprecated]` attribute to the command module (`src/commands/swarm/mod.rs:1`)

```rust
//! Swarm mode - Wave-based parallel execution with backpressure
//!
//! **DEPRECATED**: This command is moving to Descartes.
//! Use `descartes ralph --scud-tag <tag>` instead.
//!
//! ...existing docs...

#![deprecated(
    since = "1.33.0",
    note = "Use `descartes ralph` command instead. See https://github.com/pyrex41/descartes"
)]
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes (may need `#[allow(deprecated)]` in main.rs)

**Success Criteria - Manual**:
- [ ] Running `scud swarm --tag test` shows deprecation warning before execution
- [ ] Execution still works (backward compatible)

---

## Phase 2: Extract Backpressure as Public Utility

**Goal**: Make backpressure validation reusable by Descartes without code duplication.

**Changes**:

- [ ] Move backpressure module to top-level (`src/backpressure.rs`)
  - Currently at: `src/commands/swarm/backpressure.rs`
  - Move to: `src/backpressure.rs`

- [ ] Re-export from swarm for backward compatibility (`src/commands/swarm/mod.rs`)

```rust
// Keep the old path working
pub mod backpressure {
    pub use crate::backpressure::*;
}
```

- [ ] Export from lib.rs (`src/lib.rs`)

```rust
pub mod backpressure;  // Add this line
pub mod commands;
pub mod config;
pub mod formats;
pub mod llm;
pub mod models;
pub mod storage;
```

- [ ] Add documentation to backpressure module (`src/backpressure.rs:1-20`)

```rust
//! Backpressure validation utilities for agent orchestration.
//!
//! Backpressure is programmatic validation that prevents bad code from
//! being committed. This includes build/compile checks, linting, type
//! checking, and tests.
//!
//! # Example
//!
//! ```rust,no_run
//! use scud::backpressure::{BackpressureConfig, run_validation};
//! use std::path::Path;
//!
//! let config = BackpressureConfig::load(None)?;
//! let result = run_validation(Path::new("."), &config)?;
//!
//! if !result.all_passed {
//!     for failure in &result.failures {
//!         eprintln!("Failed: {}", failure);
//!     }
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo doc` generates documentation for `scud::backpressure`
- [ ] Existing swarm tests still pass

**Success Criteria - Manual**:
- [ ] `use scud::backpressure::BackpressureConfig` works in external crate
- [ ] Old path `scud::commands::swarm::backpressure` still works

---

## Phase 3: Document Library API

**Goal**: Create clear documentation for Descartes (and other consumers) to use SCUD as a library.

**Changes**:

- [ ] Add module-level documentation to `src/lib.rs`

```rust
//! SCUD - Fast, simple task master for AI-driven development
//!
//! This crate provides both a CLI tool (`scud`) and a library for programmatic
//! task management in AI-driven development workflows.
//!
//! # Library Usage
//!
//! ```rust,no_run
//! use scud::storage::Storage;
//! use scud::models::{Task, TaskStatus};
//!
//! // Load tasks from .scud/tasks/
//! let storage = Storage::new(None);  // Uses current directory
//! let phases = storage.load_tasks()?;
//!
//! // Get tasks for a specific tag
//! if let Some(phase) = phases.get("my-feature") {
//!     for task in &phase.tasks {
//!         println!("{}: {} ({:?})", task.id, task.title, task.status);
//!     }
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Modules
//!
//! - [`storage`] - Task persistence (SCG format)
//! - [`models`] - Task, Phase, TaskStatus data structures
//! - [`formats`] - SCG format parser/serializer
//! - [`backpressure`] - Validation utilities for orchestration
//! - [`config`] - Configuration management
//! - [`llm`] - LLM client for AI commands
//! - [`commands`] - CLI command implementations (advanced use)
```

- [ ] Add example to `examples/library_usage.rs`

```rust
//! Example: Using SCUD as a library for custom orchestration
//!
//! Run with: cargo run --example library_usage

use anyhow::Result;
use scud::backpressure::{BackpressureConfig, run_validation};
use scud::models::TaskStatus;
use scud::storage::Storage;
use std::path::Path;

fn main() -> Result<()> {
    // Initialize storage
    let storage = Storage::new(None);

    if !storage.is_initialized() {
        eprintln!("SCUD not initialized. Run: scud init");
        return Ok(());
    }

    // Load all tasks
    let phases = storage.load_tasks()?;

    // Find ready tasks (pending with dependencies met)
    for (tag, phase) in &phases {
        let ready: Vec<_> = phase.tasks.iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .filter(|t| t.dependencies.iter().all(|dep| {
                phase.get_task(dep)
                    .map(|d| d.status == TaskStatus::Done)
                    .unwrap_or(true)  // External deps assumed done
            }))
            .collect();

        if !ready.is_empty() {
            println!("Tag '{}' has {} ready tasks:", tag, ready.len());
            for task in ready {
                println!("  - {}: {}", task.id, task.title);
            }
        }
    }

    // Example: Run backpressure validation
    let bp_config = BackpressureConfig::load(None)?;
    if !bp_config.commands.is_empty() {
        println!("\nRunning backpressure validation...");
        let result = run_validation(Path::new("."), &bp_config)?;
        println!("All passed: {}", result.all_passed);
    }

    Ok(())
}
```

- [ ] Add `[[example]]` to `Cargo.toml`

```toml
[[example]]
name = "library_usage"
```

**Success Criteria - Automated**:
- [ ] `cargo build --examples` passes
- [ ] `cargo doc --open` shows documentation
- [ ] `cargo run --example library_usage` runs successfully in SCUD repo

**Success Criteria - Manual**:
- [ ] Documentation is clear and complete
- [ ] Example demonstrates key library features

---

## Phase 4: Version Bump and Changelog

**Goal**: Release the changes with proper versioning.

**Changes**:

- [ ] Update version in `Cargo.toml` to `1.33.0`

```toml
[package]
name = "scud-cli"
version = "1.33.0"  # Was 1.32.4
```

- [ ] Add CHANGELOG entry (`CHANGELOG.md` or inline in README)

```markdown
## [1.33.0] - 2026-01-XX

### Deprecated
- `scud swarm` command - Use `descartes ralph` instead for wave-based execution

### Added
- Public `backpressure` module for use by external orchestrators
- Library documentation with examples

### Changed
- Backpressure module moved from `commands::swarm::backpressure` to `backpressure`
  (old path still works for backward compatibility)
```

- [ ] Update README to mention Descartes for orchestration

**Success Criteria - Automated**:
- [ ] `cargo publish --dry-run` succeeds

**Success Criteria - Manual**:
- [ ] CHANGELOG accurately reflects changes
- [ ] README is updated

---

## Risks and Mitigations

### Risk: Breaking existing swarm users
**Mitigation**: Deprecation warning only, command still works. No breaking changes until v2.0.

### Risk: Backpressure module path change breaks imports
**Mitigation**: Re-export from old path for backward compatibility.

### Risk: Library API is incomplete for Descartes needs
**Mitigation**: Descartes already uses the library successfully. Document current usage patterns.

---

## Open Questions

None - all resolved.

---

## File Reference Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src/commands/swarm/mod.rs:1` | Modify | Add `#[deprecated]` attribute |
| `src/commands/swarm/mod.rs:39` | Modify | Add deprecation warning in `run()` |
| `src/commands/swarm/backpressure.rs` | Move | Move to `src/backpressure.rs` |
| `src/backpressure.rs` | Create | New location for backpressure module |
| `src/lib.rs` | Modify | Add documentation, export backpressure |
| `examples/library_usage.rs` | Create | Library usage example |
| `Cargo.toml` | Modify | Version bump, add example |
| `CHANGELOG.md` or README | Modify | Document changes |
