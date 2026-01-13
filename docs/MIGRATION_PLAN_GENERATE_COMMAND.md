# Plan: SCUD `generate` Command

## Overview

Create a unified `scud generate` command that orchestrates the full PRD-to-tasks pipeline:
1. Parse PRD into tasks (`scud parse`)
2. Expand complex tasks into subtasks (`scud expand`)
3. Validate and fix dependencies against PRD (`scud check-deps --prd --fix`)

This provides a single entry point for initializing a task graph from a PRD document.

## Prerequisites

Read [UNIFIED_CONTEXT.md](/Users/reuben/projects/harnesses/docs/UNIFIED_CONTEXT.md) first for shared context.

## Current State

Today, initializing tasks from a PRD requires three separate commands:

```bash
scud parse <file> --tag <tag> -n 10
scud expand --tag <tag>
scud check-deps --prd <file> --fix --tag <tag>
```

This is error-prone and requires users to remember the correct sequence.

## Desired End State

Single command that does all three steps:

```bash
scud generate <file> --tag <tag>
```

With options to customize each phase and skip phases if needed.

---

## Phase 1: Add `Generate` Command to CLI

**Goal**: Create the command definition in main.rs with appropriate options.

**Changes**:

- [ ] Add `Generate` variant to `Commands` enum (`src/main.rs`, after `Parse`)

```rust
/// Generate tasks from PRD (parse → expand → check-deps pipeline)
Generate {
    /// Path to PRD/spec document
    file: PathBuf,

    /// Tag name for generated tasks
    #[arg(short, long)]
    tag: String,

    /// Number of tasks to generate (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    num_tasks: u32,

    // === Phase Control ===
    /// Skip task expansion phase
    #[arg(long)]
    no_expand: bool,

    /// Skip dependency validation phase
    #[arg(long)]
    no_check_deps: bool,

    // === Parse Options ===
    /// Append tasks to existing tag instead of replacing
    #[arg(long)]
    append: bool,

    /// Skip loading guidance from .scud/guidance/
    #[arg(long)]
    no_guidance: bool,

    /// Task ID format: sequential (default) or uuid
    #[arg(long, default_value = "sequential")]
    id_format: String,

    // === Model Selection ===
    /// Model to use for all AI operations (overrides config)
    #[arg(long)]
    model: Option<String>,

    // === Output Control ===
    /// Show what would be done without making changes
    #[arg(long)]
    dry_run: bool,

    /// Verbose output showing each phase's details
    #[arg(short, long)]
    verbose: bool,
},
```

- [ ] Add match arm in `main()` to handle the command (`src/main.rs`, in command dispatch)

```rust
Commands::Generate {
    file,
    tag,
    num_tasks,
    no_expand,
    no_check_deps,
    append,
    no_guidance,
    id_format,
    model,
    dry_run,
    verbose,
} => {
    commands::generate::run(
        cli.project,
        &file,
        &tag,
        num_tasks,
        no_expand,
        no_check_deps,
        append,
        no_guidance,
        &id_format,
        model.as_deref(),
        dry_run,
        verbose,
    )
    .await
}
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `scud generate --help` shows all options

---

## Phase 2: Implement `generate` Command Module

**Goal**: Create the command implementation that orchestrates the three phases.

**Changes**:

- [ ] Create `src/commands/generate.rs`

```rust
//! Generate tasks from PRD - unified pipeline command
//!
//! Orchestrates:
//! 1. `parse` - Parse PRD into initial tasks
//! 2. `expand` - Break down complex tasks into subtasks
//! 3. `check-deps` - Validate and fix dependencies against PRD
//!
//! # Example
//!
//! ```bash
//! scud generate requirements.md --tag v1 -n 15
//! ```

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::commands::{ai, check_deps};

pub async fn run(
    project_root: Option<PathBuf>,
    file_path: &Path,
    tag: &str,
    num_tasks: u32,
    no_expand: bool,
    no_check_deps: bool,
    append: bool,
    no_guidance: bool,
    id_format: &str,
    model: Option<&str>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    println!("{}", "━".repeat(60).dimmed());
    println!(
        "{}  Generating tasks from: {}",
        "📋".to_string(),
        file_path.display().to_string().cyan()
    );
    println!("{}", "━".repeat(60).dimmed());
    println!();

    // ═══════════════════════════════════════════════════════════════
    // Phase 1: Parse PRD into tasks
    // ═══════════════════════════════════════════════════════════════
    println!(
        "{}  {} Parsing PRD into tasks...",
        "[1/3]".dimmed(),
        "→".blue()
    );

    if dry_run {
        println!(
            "     {} Would run: scud parse {} --tag {} -n {}",
            "dry-run:".yellow(),
            file_path.display(),
            tag,
            num_tasks
        );
    } else {
        ai::parse_prd::run(
            project_root.clone(),
            file_path,
            tag,
            num_tasks,
            append,
            no_guidance,
            id_format,
            model,
        )
        .await?;
    }

    println!();

    // ═══════════════════════════════════════════════════════════════
    // Phase 2: Expand complex tasks
    // ═══════════════════════════════════════════════════════════════
    if no_expand {
        println!(
            "{}  {} Skipping task expansion (--no-expand)",
            "[2/3]".dimmed(),
            "⏭".yellow()
        );
    } else {
        println!(
            "{}  {} Expanding complex tasks...",
            "[2/3]".dimmed(),
            "→".blue()
        );

        if dry_run {
            println!(
                "     {} Would run: scud expand --tag {}",
                "dry-run:".yellow(),
                tag
            );
        } else {
            ai::expand::run(
                project_root.clone(),
                None,       // task_id - expand all
                false,      // all_tags - just this tag
                Some(tag),  // specific tag
                no_guidance,
                model,
            )
            .await?;
        }
    }

    println!();

    // ═══════════════════════════════════════════════════════════════
    // Phase 3: Validate and fix dependencies
    // ═══════════════════════════════════════════════════════════════
    if no_check_deps {
        println!(
            "{}  {} Skipping dependency check (--no-check-deps)",
            "[3/3]".dimmed(),
            "⏭".yellow()
        );
    } else {
        println!(
            "{}  {} Validating dependencies against PRD...",
            "[3/3]".dimmed(),
            "→".blue()
        );

        if dry_run {
            println!(
                "     {} Would run: scud check-deps --tag {} --prd {} --fix",
                "dry-run:".yellow(),
                tag,
                file_path.display()
            );
        } else {
            check_deps::run(
                project_root.clone(),
                Some(tag),       // tag
                false,           // all_tags
                Some(file_path), // prd
                true,            // fix
                model,
            )
            .await?;
        }
    }

    println!();
    println!("{}", "━".repeat(60).dimmed());
    println!(
        "{}  Tasks generated for tag: {}",
        "✓".green().bold(),
        tag.cyan()
    );
    println!("{}", "━".repeat(60).dimmed());

    // Show next steps
    if !dry_run {
        println!();
        println!("{}", "Next steps:".blue().bold());
        println!("  • View tasks:    scud list --tag {}", tag);
        println!("  • View waves:    scud waves --tag {}", tag);
        println!("  • Start work:    scud next --tag {}", tag);
        if verbose {
            println!("  • View graph:    scud mermaid --tag {}", tag);
        }
    }

    Ok(())
}
```

- [ ] Export from commands mod (`src/commands/mod.rs`)

```rust
pub mod generate;
```

**Success Criteria - Automated**:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

**Success Criteria - Manual**:
- [ ] `scud generate --dry-run requirements.md --tag test` shows plan
- [ ] `scud generate requirements.md --tag test` creates tasks

---

## Phase 3: Add Library Export

**Goal**: Expose generate functionality for programmatic use by Descartes.

**Changes**:

- [ ] Add public types and re-exports (`src/commands/generate.rs`)

```rust
/// Options for the generate pipeline
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    /// Number of tasks to generate
    pub num_tasks: u32,
    /// Skip task expansion phase
    pub no_expand: bool,
    /// Skip dependency validation phase
    pub no_check_deps: bool,
    /// Append to existing tag
    pub append: bool,
    /// Skip guidance files
    pub no_guidance: bool,
    /// ID format: "sequential" or "uuid"
    pub id_format: String,
    /// Override model selection
    pub model: Option<String>,
}

impl GenerateOptions {
    pub fn new() -> Self {
        Self {
            num_tasks: 10,
            id_format: "sequential".to_string(),
            ..Default::default()
        }
    }
}

/// Run the generate pipeline programmatically
pub async fn generate(
    project_root: Option<PathBuf>,
    prd_path: &Path,
    tag: &str,
    options: GenerateOptions,
) -> Result<()> {
    run(
        project_root,
        prd_path,
        tag,
        options.num_tasks,
        options.no_expand,
        options.no_check_deps,
        options.append,
        options.no_guidance,
        &options.id_format,
        options.model.as_deref(),
        false, // dry_run
        false, // verbose
    )
    .await
}
```

- [ ] Document library usage in `src/lib.rs`

```rust
//! ## Task Generation Pipeline
//!
//! ```rust,no_run
//! use scud::commands::generate::{generate, GenerateOptions};
//! use std::path::Path;
//!
//! let options = GenerateOptions {
//!     num_tasks: 15,
//!     ..Default::default()
//! };
//!
//! generate(None, Path::new("prd.md"), "my-feature", options).await?;
//! # Ok::<(), anyhow::Error>(())
//! ```
```

**Success Criteria - Automated**:
- [ ] `cargo doc` generates documentation
- [ ] `use scud::commands::generate::GenerateOptions` compiles

---

## Phase 4: Update CHANGELOG and Version

**Goal**: Document the new command.

**Changes**:

- [ ] Add CHANGELOG entry

```markdown
## [1.33.0] - 2026-01-XX

### Added
- `scud generate` command - unified PRD-to-tasks pipeline
  - Orchestrates: parse → expand → check-deps
  - Options: `--no-expand`, `--no-check-deps`, `--dry-run`
  - Library API: `scud::commands::generate::generate()`

### Deprecated
- `scud swarm` command - Use `descartes ralph` instead
```

- [ ] Update README with generate command

```markdown
### Quick Start

```bash
# Initialize and generate tasks from PRD
scud init
scud generate requirements.md --tag v1

# View generated tasks
scud list --tag v1
scud waves --tag v1

# Start working
scud next --tag v1
```
```

**Success Criteria - Automated**:
- [ ] `cargo publish --dry-run` succeeds

---

## Usage Examples

### Basic Usage

```bash
# Generate 10 tasks from PRD
scud generate requirements.md --tag v1

# Generate more tasks for a complex PRD
scud generate large-prd.md --tag v2 -n 25

# Preview what would happen
scud generate requirements.md --tag test --dry-run
```

### Phase Control

```bash
# Just parse, don't expand or check deps
scud generate requirements.md --tag v1 --no-expand --no-check-deps

# Parse and expand, skip dep checking
scud generate requirements.md --tag v1 --no-check-deps
```

### Integration with Descartes

```bash
# Descartes ralph can use scud generate for initialization
descartes ralph --prd requirements.md --tag v1

# Which internally runs:
# scud generate requirements.md --tag v1
# Then proceeds with ralph loop
```

---

## File Reference Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src/main.rs` | Modify | Add `Generate` command variant and handler |
| `src/commands/generate.rs` | Create | New command implementation |
| `src/commands/mod.rs` | Modify | Export generate module |
| `src/lib.rs` | Modify | Document library usage |
| `CHANGELOG.md` | Modify | Document new command |
| `README.md` | Modify | Add quick start with generate |

---

## Relationship to Descartes

The `scud generate` command simplifies Descartes integration:

**Before** (Descartes migration plan had):
```rust
// Shell out to three separate commands
std::process::Command::new("scud").args(["parse", ...]).status()?;
std::process::Command::new("scud").args(["expand", ...]).status()?;
std::process::Command::new("scud").args(["check-deps", ...]).status()?;
```

**After** (single command or library call):
```rust
// Option 1: Shell to single command
std::process::Command::new("scud")
    .args(["generate", &prd_path, "--tag", &tag])
    .status()?;

// Option 2: Library call (preferred)
use scud::commands::generate::{generate, GenerateOptions};
generate(Some(working_dir), &prd_path, &tag, GenerateOptions::default()).await?;
```

The Descartes migration plan should be updated to use `scud generate` instead of three separate commands.
