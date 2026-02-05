# AGENTS.md - SCUD / Descartes GUI Operations Guide

## Project Overview

SCUD is a DAG-based task manager with a desktop GUI (Descartes) built on iced 0.14. The workspace contains `scud-core` (library), `scud-cli` (CLI binary), and `descartes-gui` (iced GUI).

## Tech Stack

- Language: Rust (edition 2021)
- GUI Framework: iced 0.14 (with tokio + advanced features)
- Async: tokio (full)
- Serialization: serde + serde_json
- Messaging: ZeroMQ (zeromq 0.4)
- Testing: iced_test 0.14 (for GUI), standard cargo test
- Key crates: anyhow, tracing, async-stream

## Directory Structure

```
scud/
├── scud-core/           # Core library (Storage, Task, DAG)
├── scud-cli/            # CLI binary
├── descartes-gui/       # Iced desktop GUI (primary target)
│   └── src/
│       ├── main.rs      # App struct, Message enum, update/view
│       ├── state.rs     # AppState, data structures
│       ├── scud_bridge.rs  # ScudCommand/ScudEvent, async bridge
│       ├── theme.rs     # Styling
│       ├── zmq_client.rs   # ZeroMQ swarm connection
│       ├── components/  # Reusable UI components
│       └── views/       # Tab views
│           ├── mod.rs
│           ├── agents.rs   # Agent/config panel
│           ├── header.rs   # Header bar
│           ├── monitor.rs  # Headless session monitor
│           ├── output.rs   # Output display
│           └── waves.rs    # Task waves view
├── specs/               # JTBD specifications
└── docs/                # Documentation
```

## Validation Commands

- **Build**: `cargo build -p descartes-gui`
- **Test**: `cargo test -p descartes-gui`
- **Lint**: `cargo clippy -p descartes-gui -- -D warnings`
- **Format check**: `cargo fmt -p descartes-gui --check`
- **Full check**: `cargo fmt -p descartes-gui --check && cargo clippy -p descartes-gui -- -D warnings && cargo test -p descartes-gui`

Run full check before committing. Clippy warnings are errors.

## Conventions

### Code Style
- Follow standard Rust idioms
- Use `anyhow::Result` for error handling in application code
- Use `tracing` for logging (`tracing::debug!`, `tracing::info!`, etc.)
- Imports: group std, external crates, then local modules

### iced Patterns
- `Message` enum in `main.rs` for all UI events
- `update()` returns `Task<Message>` (iced 0.14 uses Task, not Command)
- Views are functions in `views/` that take state references and return `Element<Message>`
- Use `pick_list`, `text_input`, `button` from iced widgets
- State structs live in `state.rs`

### Bridge Pattern
- `ScudCommand` enum for requests to the bridge
- `ScudEvent` enum for responses from the bridge
- Bridge runs async operations via tokio, sends events back through iced subscriptions
- Commands sent via `tokio::sync::mpsc`

### Testing
- GUI tests use `iced_test` framework
- Test helpers (like `test_app()`) in test modules
- Test file locations: inline `#[cfg(test)] mod tests` in each source file

## Subagent Guidelines

- Search/analysis: up to 100 parallel Sonnet subagents
- Implementation: up to 5 parallel Sonnet subagents, partition by file
- Validation: exactly 1 Sonnet subagent, sequential steps
- Architecture/debugging: Opus subagent as needed

Never parallelize test execution.

## Common Operations

### Adding a new Message variant
1. Add variant to `Message` enum in `main.rs`
2. Add handler in `update()` match
3. Wire to view in appropriate `views/*.rs` file

### Adding a new ScudCommand/ScudEvent
1. Add variant to `ScudCommand` in `scud_bridge.rs`
2. Add variant to `ScudEvent` in `scud_bridge.rs`
3. Handle command in bridge's async loop
4. Handle event in `update()` via `Message::ScudEvent`

### Adding state fields
1. Add field to struct in `state.rs`
2. Initialize in `new()` in `main.rs`
3. Update `test_app()` helper if tests exist

## Guardrails

- Never modify `scud-core` or `scud-cli` without explicit instruction
- Never commit Cargo.lock changes unless dependencies actually changed
- Always run `cargo test -p descartes-gui` before committing
- Keep iced version at 0.14 (don't upgrade)
