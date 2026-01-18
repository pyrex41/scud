# SCUD CLI (Rust)

Fast, simple task master for AI-driven development - Rust implementation.

## Overview

This is a high-performance Rust rewrite of the SCUD task management system. It replaces the external `task-master` CLI with a fast, single-binary solution that:

- ⚡ **50x faster** startup time (~10ms vs ~500ms)
- 🎯 **42x token reduction** (~500 tokens vs ~21k tokens per operation)
- 📦 **Simple distribution** - single binary, no dependencies
- 🔧 **Direct LLM integration** - no MCP overhead

## Architecture

```
scud (Rust Binary)
├── Core Commands (No AI - Instant)
│   ├── init               # Initialize .scud/ and install agents
│   ├── tags               # List tags or set active tag
│   ├── list               # List tasks with filters
│   ├── view               # Open interactive HTML viewer in browser
│   ├── show               # Show task details
│   ├── set-status         # Update task status
│   ├── next               # Find next available task
│   ├── stats              # Show statistics
│   └── doctor             # [EXPERIMENTAL] Diagnose stuck states
│
├── AI Commands (Direct LLM API)
│   ├── parse-prd          # Parse PRD markdown into tasks
│   ├── analyze-complexity # Analyze task complexity
│   ├── expand             # Break down complex tasks
│   └── reanalyze-deps     # Analyze cross-tag dependencies
│
└── Storage (SCG)
    └── .scud/tasks/tasks.scg
```

## Installation

### Option 1: npm (Recommended)

Install globally via npm - this will build the Rust binary automatically:

```bash
npm install -g scud-task
```

**Requirements:** Rust toolchain must be installed ([rustup.rs](https://rustup.rs/))

### Option 2: Cargo

Install directly via Cargo:

```bash
cargo install scud-cli
```

Or build from source:

```bash
git clone https://github.com/pyrex41/scud.git
cd scud/scud-cli
cargo install --path .
```

### Verify Installation

```bash
scud --version
scud --help
```

## Building (Development)

### Debug Build
```bash
cargo build
```

### Release Build
```bash
cargo build --release
```

## Usage

### Core Commands

```bash
# Initialize SCUD
scud init

# List tags
scud tags

# Switch to a tag
scud tags auth

# List tasks
scud list
scud list --status pending

# Show task details
scud show 3

# Update task status
scud set-status 3 in-progress

# Find next available task
scud next

# Show statistics
scud stats

# Open interactive task viewer in browser
scud view

# Clean up tasks (archives by default)
scud clean                    # Archive all tags
scud clean --tag auth         # Archive specific tag
scud clean --list             # List archived phases
scud clean --restore <name>   # Restore archived phase
scud clean --delete           # Permanently delete (use with caution)
```

> **Note:** `scud clean` archives tasks by default instead of deleting them. This provides a safety net for accidental cleanups. Use `--delete` to permanently remove tasks. Archives are stored in `.scud/archive/`.

### [EXPERIMENTAL] Doctor Command

Diagnose stuck workflow states:

```bash
# Check for issues in all tags
scud doctor

# Check specific tag with custom stale threshold
scud doctor --tag auth --stale-hours 12

# Auto-fix recoverable issues (stale locks, orphan tasks)
scud doctor --fix
```

The doctor command detects:
- Stale locks (tasks locked >24h by default)
- Tasks blocked by cancelled/missing dependencies
- Orphan in-progress tasks (not locked, stale)
- Missing active tag
- Corrupt storage files

### AI Commands

**Requires:** API key environment variable (see [Provider Configuration](#provider-configuration))

```bash
# Parse PRD into tasks
scud parse-prd docs/features/auth.md --tag auth

# Analyze complexity
scud analyze-complexity                # All tasks
scud analyze-complexity --task 5       # Specific task

# Expand complex tasks
scud expand 7                          # Specific task
scud expand --all                      # All tasks >13 complexity

# Reanalyze cross-tag dependencies
scud reanalyze-deps --all-tags
```

### UUID Task IDs

For integration with external tools that expect UUID task IDs (like Descartes):

```bash
# Generate tasks with UUID IDs instead of sequential numbers
scud parse requirements.md --tag myproject --id-format uuid
```

This generates tasks with 32-character UUID identifiers (e.g., `a1b2c3d4e5f6789012345678901234ab`) instead of sequential numbers (`1`, `2`, `3`).

**Key behaviors:**
- Sequential IDs remain the default for backwards compatibility
- The ID format is stored in the phase metadata and inherited during expansion
- Subtasks also get UUID IDs when the parent phase uses UUID format
- Long UUIDs are truncated in CLI output (`a1b2c3d4...`) for readability
- The `scud show` command displays the full UUID

## Performance Comparison

| Operation | Old (task-master) | New (Rust) | Improvement |
|-----------|------------------|------------|-------------|
| Startup | ~500ms | ~10ms | **50x faster** |
| List tasks | ~100ms | ~5ms | **20x faster** |
| Parse PRD | ~3-5s | ~2-3s | ~40% faster |
| Token overhead | ~21k | ~500 | **42x reduction** |

## Provider Configuration

SCUD supports multiple LLM providers: **xAI (Grok)**, **Anthropic (Claude)**, **OpenAI (GPT)**, and **OpenRouter**.

### Quick Start

```bash
# Initialize with xAI (Grok) - recommended for fast code generation
scud init --provider xai
export XAI_API_KEY=your-key

# Or initialize with Anthropic (Claude)
scud init --provider anthropic
export ANTHROPIC_API_KEY=your-key

# Interactive mode - prompt for provider
scud init
```

### Configuration File

The configuration is stored in `.scud/config.toml`:

```toml
[llm]
provider = "xai"
model = "xai/grok-code-fast-1"
max_tokens = 4096
```

For complete provider documentation, see [PROVIDERS.md](./PROVIDERS.md).

### Supported Providers

| Provider | Environment Variable | Default Model |
|----------|---------------------|---------------|
| xAI | `XAI_API_KEY` | `xai/grok-code-fast-1` |
| Anthropic | `ANTHROPIC_API_KEY` | `claude-sonnet-4-20250514` |
| OpenAI | `OPENAI_API_KEY` | `gpt-4-turbo` |
| OpenRouter | `OPENROUTER_API_KEY` | `anthropic/claude-sonnet-4` |

## Data Models

### Task
```rust
struct Task {
    id: String,
    title: String,
    description: String,
    status: TaskStatus,         // pending, in-progress, done, etc.
    complexity: u32,            // Fibonacci scale: 1,2,3,5,8,13,21
    priority: Priority,         // high, medium, low
    dependencies: Vec<String>,  // Task IDs this depends on
    details: Option<String>,    // Technical details
    test_strategy: Option<String>,
    complexity_analysis: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}
```

### Phase
```rust
struct Phase {
    name: String,
    tasks: Vec<Task>,
    id_format: IdFormat,  // sequential (default) or uuid
}
```

### Config
```toml
[llm]
provider = "xai"
model = "xai/grok-code-fast-1"
max_tokens = 4096
```

## LLM Integration

### Direct Anthropic API
- No MCP server overhead
- Simple HTTP requests
- Minimal token usage
- Fast response times

### Prompt Templates

Located in `src/llm/prompts.rs`:
- `parse_prd()` - Converts markdown to structured tasks
- `analyze_complexity()` - Scores task difficulty
- `expand_task()` - Breaks down complex tasks
- `reanalyze_dependencies()` - Cross-tag dependency analysis

## Integration with SCUD

The Rust CLI integrates seamlessly with the existing SCUD system:

1. `bin/scud.js` detects and delegates to Rust binary
2. Falls back to debug build if release not available
3. Auto-builds if binary not found
4. All agents and slash commands work unchanged

## Development

### Project Structure

```
scud-cli/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── init.rs          # Core commands
│   │   ├── tags.rs
│   │   ├── ...
│   │   └── ai/              # AI commands
│   │       ├── parse_prd.rs
│   │       ├── analyze_complexity.rs
│   │       ├── expand.rs
│   │       └── reanalyze_deps.rs
│   ├── models/
│   │   ├── task.rs
│   │   └── phase.rs
│   ├── storage/
│   │   └── mod.rs           # JSON I/O
│   └── llm/
│       ├── client.rs        # Anthropic API
│       └── prompts.rs       # Prompt templates
```

### Adding New Commands

1. Add command to `Commands` enum in `main.rs`
2. Create handler in `src/commands/`
3. Add module to `src/commands/mod.rs`
4. Update help text

### Adding New LLM Prompts

1. Add prompt function to `src/llm/prompts.rs`
2. Create command handler in `src/commands/ai/`
3. Use `LLMClient::complete()` or `complete_json()`

## Testing

```bash
# Build and test
cargo build
cargo test

# Test specific command
cargo run -- init
cargo run -- tags
cargo run -- --help

# Test AI commands (requires API key)
export ANTHROPIC_API_KEY=sk-...
cargo run -- parse-prd test.md --tag test
```

## Distribution

### As Standalone Binary

```bash
cargo build --release
# Binary: target/release/scud
# Copy to /usr/local/bin or similar
```

### Via npm Package

The npm package builds the Rust binary during installation:
- Runs `cargo build --release` during `npm install`
- Binary is placed in `bin/` directory
- `bin/scud.js` is a thin wrapper that executes the binary
- Requires Rust toolchain to be installed

## Future Enhancements

- [ ] Cross-compilation for multiple platforms
- [ ] Pre-built binaries in npm package (eliminate Rust requirement)
- [ ] Task export/import
- [ ] Custom prompt templates
- [ ] Integration tests with real API calls

## License

MIT

## Contributing

See main SCUD repository for contribution guidelines.
