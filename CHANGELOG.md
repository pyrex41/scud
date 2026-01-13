# Changelog

All notable changes to SCUD will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.33.0] - 2026-01-13

### Deprecated

- **`scud swarm` command** - The swarm command is now deprecated in favor of `descartes ralph`. The command displays a deprecation warning at runtime and includes compiler-level `#[deprecated]` attributes. Users should migrate to [Descartes](https://github.com/pyrex41/descartes) for wave-based parallel execution with AI agents.

### Changed

- **`scud clean` now archives by default** - The `clean` command now archives tasks to `.scud/archive/` instead of permanently deleting them. This provides a safety net for accidental cleanups. Use `--delete` to permanently remove tasks (previous behavior). New options:
  - `--list` - List all archived phases
  - `--restore <name>` - Restore a previously archived phase
  - `--keep <tags>` - Specify tags to keep when cleaning multiple
  - `--delete` - Permanently delete instead of archiving (use with caution)

### Added

- **`scud generate` command** - New unified pipeline command that combines `parse`, `expand`, and `check-deps` into a single workflow. This simplifies the task generation process from PRD documents by running all three phases sequentially with configurable options:
  - `--no-expand` - Skip the task expansion phase
  - `--no-check-deps` - Skip the dependency validation phase
  - `--dry-run` - Preview changes without writing
  - `--verbose` - Show detailed output from each phase

- **Public backpressure module** (`scud::backpressure`) - The backpressure validation system is now exported as a public API. This allows external tools like Descartes to use SCUD's validation logic for build/test/lint checks after task completion.
  - `BackpressureConfig` - Configuration for validation commands
  - `run_validation()` - Main entry point for running validation
  - `ValidationResult` and `CommandResult` - Structured results
  - Auto-detection for Rust, Node.js, Python, and Go projects

- **Library documentation** - Comprehensive module-level documentation added to `lib.rs` with usage examples for all public modules: `storage`, `models`, `formats`, `config`, `llm`, `commands`, and `backpressure`.

- **Library usage example** - New example at `examples/library_usage.rs` demonstrating:
  - Task creation and configuration
  - Phase operations and statistics
  - Dependency resolution
  - Storage operations
  - Backpressure validation

### Changed

- **Backpressure module location** - The backpressure module has been moved from `commands::swarm::backpressure` to the top-level `scud::backpressure`. A re-export in `commands::swarm` maintains backward compatibility for existing code.

## [1.32.4] - 2026-01-13

### Added

- Timeout handling for swarm sessions
- Session locking to prevent concurrent swarm runs on the same tag
- Commit tracking for wave summaries

## [1.32.3] - 2026-01-XX

### Added

- Embedded "scud" skill with CLI guide for Claude Code integration

## [1.32.2] - 2026-01-XX

### Fixed

- Test configuration for API key environment variables
- CI/CD publishing steps

## [1.32.1] - 2026-01-XX

### Fixed

- Embedded agent files in crate for proper publishing

## [1.32.0] - 2026-01-XX

### Added

- GitHub Actions workflow for automatic crates.io publishing
- Pure Rust CLI (eliminated npm dependencies)

## [1.31.0] - 2026-01-XX

### Fixed

- Cross-tag dependency resolution
