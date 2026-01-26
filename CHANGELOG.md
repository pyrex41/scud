# Changelog

All notable changes to SCUD will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.47.0] - 2026-01-26

### Added

- **SQLite database** - All event logging, transcript storage, and session history now stored in a queryable SQLite database (`.scud/scud.db`). Replaces JSONL event files with indexed, queryable storage using WAL mode for concurrent access during swarm execution. Schema includes 9 tables: sessions, agent_runs, events, transcript_messages, tool_calls, tool_results, validation_runs, validation_commands, and salvo_worktrees.

- **Real-time transcript capture** - Claude Code conversation transcripts are automatically imported from `~/.claude/projects/` into SQLite during swarm execution. A background file watcher (using FSEvents on macOS) monitors for new transcript files and imports them in real-time. New CLI commands:
  - `scud transcript search <query>` - Full-text search across transcript content
  - `scud transcript stats` - Show aggregate statistics (sessions, messages, tool calls)
  - `scud transcript list` - List recent transcript sessions with message counts
  - `scud transcript view [--session <id>]` - View transcript summaries
  - `scud transcript import` - Bulk import all project transcripts

- **Salvo worktrees** - Automatic git worktree provisioning per-tag for parallel swarm execution. When `scud swarm --tag <tag>` is invoked, SCUD automatically creates an isolated worktree at `../<project>.salvo.<tag>/` with a filtered task file. New CLI commands:
  - `scud salvo list` - Show all salvo worktrees with paths, branches, and sync status
  - `scud salvo sync <tag>` - Manually sync worktree task status back to main
  - `scud salvo remove <tag>` - Clean up worktree and associated git branch

- **Swarm live progress monitoring** - Wave mode now displays live progress with heartbeat monitoring, orphan detection, and configurable stale timeout (`--stale-timeout` flag). Detects agents that stop responding and reports orphaned tasks.

- **6 new event kinds** - WaveStarted, WaveCompleted, ValidationPassed, ValidationFailed, RepairStarted, RepairCompleted. All event kinds have full round-trip SQLite serialization.

### Changed

- **Event storage migrated to SQLite** - EventWriter and EventReader now use SQLite instead of JSONL files. Events are queryable with SQL and indexed by session, task, timestamp, and kind. Old `.scud/swarm/events/*.jsonl` files are no longer written.

- **Swarm command gains new flags**:
  - `--no-worktree` - Skip automatic worktree creation (run in-place, previous behavior)
  - `--salvo-dir <path>` - Custom directory for salvo worktree (overrides convention path)
  - `--stale-timeout <secs>` - Configure stale agent detection timeout

- **Session locks are worktree-aware** - Lock files are now scoped to worktree context, allowing the same tag to run in different worktrees without lock conflicts.

## [1.46.0] - 2026-01-24

### Added

- **Beads execution mode** (`scud swarm --swarm-mode beads`) - New continuous polling execution strategy inspired by the [Beads project](https://github.com/steveyegge/beads). Unlike wave-based execution which batches tasks and waits for completion, beads mode continuously polls for ready tasks and spawns agents immediately when dependencies are met. This enables more fluid execution where downstream tasks start as soon as their dependencies complete.

- **Event logging system** - Comprehensive structured event logging for retrospective analysis. Events are written to JSONL files in `.scud/swarm/events/` and can be aggregated into timelines:
  - `scud swarm retro [session-id]` - View retrospective timeline for a swarm session
  - Tracks spawns, completions, tool calls, file operations, and dependency unblocking
  - Per-task and per-session event files for detailed analysis

- **Claude transcript parsing** - New `scud swarm transcript` command to view and analyze Claude Code transcripts from swarm sessions. Extracts tool calls, file operations, and conversation flow from agent output.

- **Extension system** (experimental) - New extension loader and runner infrastructure for custom agent types:
  - Extension manifests in `.scud/agents/*.toml`
  - Automatic discovery and validation
  - `scud doctor --ext` to scan and validate extensions

- **OpenCode integration** (experimental) - Server-sent events integration with OpenCode for remote agent orchestration:
  - Event streaming from OpenCode sessions
  - Session management and orchestration
  - Alternative to local tmux-based execution

- **Enhanced TUI components** - New modular TUI components for spawn monitor:
  - Agent selector with status indicators
  - Model selector dropdown
  - Streaming view for live output
  - Improved wave visualization

- **Swarm session persistence** - Extended session state tracking with wave and round history, commit tracking, and event correlation.

### Changed

- **Swarm command undeprecated** - The `scud swarm` command has been restored with two execution modes:
  - `--swarm-mode wave` (default) - Traditional wave-based execution with backpressure validation
  - `--swarm-mode beads` - New continuous polling execution for fluid task flow

- **Doctor command enhanced** - Added `--ext` flag for extension scanning and validation.

### Fixed

- Event deduplication now compares full event content instead of just event type discriminant
- Spawned task tracking properly cleans up completed tasks to prevent memory growth

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
