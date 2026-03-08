# PR #10 Enhancement: Make ZeroMQ Optional via Feature Flag

## Overview

Enhance PR #10 (`claude/socket-feed-monitor-V2JHm`) by making the ZeroMQ dependency optional through a Cargo feature flag. This keeps the socket feed functionality available but doesn't require all users to compile ZeroMQ (which adds ~15 transitive dependencies).

## Current State Analysis

PR #10 adds:
- `zeromq = "0.4"` as a required dependency in Cargo.toml
- `scud-cli/src/commands/spawn/feed.rs` - socket feed implementation
- Changes to `tui/mod.rs` and `tui/app.rs` for feed integration
- `--feed` CLI flag to the monitor command

### Key Discoveries:
- ZeroMQ adds significant transitive dependencies (async-trait, dashmap, rand, etc.)
- The feed module is self-contained and can be conditionally compiled
- Existing feature flags in codebase use simple pattern: `feature-name = []`

## Desired End State

1. ZeroMQ dependency is optional
2. New `socket-feed` feature flag enables the dependency
3. Feed-related code is conditionally compiled
4. `--feed` CLI flag is only available when feature is enabled
5. Monitor works normally without the feature (just no `--feed` option)

### Verification:
- `cargo build` works without the feature (no zeromq compiled)
- `cargo build --features socket-feed` works with zeromq
- `scud monitor --help` shows `--feed` only when feature is enabled
- All tests pass in both configurations

## What We're NOT Doing

- Not changing the feed implementation itself
- Not modifying message formats or protocols
- Not adding feature flags for other optional functionality

## Implementation Approach

Use Rust's standard conditional compilation with `#[cfg(feature = "...")]` attributes. This is the simplest, most dependable approach with minimal boilerplate.

## Phase 1: Make Dependency Optional

### Overview
Update Cargo.toml to make zeromq an optional dependency gated by a feature flag.

### Changes Required:

#### 1.1 Update Cargo.toml

**File**: `scud-cli/Cargo.toml`
**Changes**: Make zeromq optional and add feature flag

**Current:**
```toml
# Socket feed (ZeroMQ)
zeromq = "0.4"        # Async ZeroMQ bindings for socket feed

[features]
default = []
real-llm = []        # Enable tests with real LLM API calls
real-terminal = []   # Enable tests with real terminal sessions (tmux/kitty)
```

**Updated:**
```toml
# Socket feed (ZeroMQ) - optional
zeromq = { version = "0.4", optional = true }

[features]
default = []
real-llm = []        # Enable tests with real LLM API calls
real-terminal = []   # Enable tests with real terminal sessions (tmux/kitty)
socket-feed = ["zeromq"]  # Enable ZMQ socket feed for remote monitoring
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles without zeromq
- [ ] `cargo build --features socket-feed` compiles with zeromq

---

## Phase 2: Conditionally Compile Feed Module

### Overview
Add `#[cfg(feature = "socket-feed")]` to the feed module and all code that uses it.

### Changes Required:

#### 2.1 Update spawn/mod.rs

**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Conditionally include feed module

**Current:**
```rust
pub mod agent;
pub mod feed;
pub mod hooks;
pub mod monitor;
pub mod terminal;
```

**Updated:**
```rust
pub mod agent;
#[cfg(feature = "socket-feed")]
pub mod feed;
pub mod hooks;
pub mod monitor;
pub mod terminal;
```

#### 2.2 Update tui/mod.rs

**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Conditionally import and use feed

**Add at top:**
```rust
#[cfg(feature = "socket-feed")]
use super::feed::{self, FeedConfig};
```

**Update the run function to conditionally handle feed:**
```rust
pub fn run(
    project_root: Option<PathBuf>,
    session_name: &str,
    #[cfg(feature = "socket-feed")]
    feed_endpoint: Option<String>,
    #[cfg(not(feature = "socket-feed"))]
    _feed_endpoint: Option<String>,  // Accept but ignore when feature disabled
) -> Result<()> {
    // Start socket feed if endpoint provided AND feature enabled
    #[cfg(feature = "socket-feed")]
    let feed_handle = if let Some(endpoint) = feed_endpoint {
        let config = FeedConfig::from_endpoint(&endpoint);
        match feed::start_feed_sync(config) {
            Ok((handle, bound_endpoint)) => {
                eprintln!(
                    "{} Socket feed bound to {}",
                    ColoredColorize::green("✓"),
                    ColoredColorize::cyan(bound_endpoint.as_str())
                );
                Some(handle)
            }
            Err(e) => {
                eprintln!(
                    "{} Failed to start socket feed: {}",
                    ColoredColorize::yellow("!"),
                    ColoredColorize::dimmed(e.to_string().as_str())
                );
                None
            }
        }
    } else {
        None
    };

    #[cfg(not(feature = "socket-feed"))]
    let feed_handle: Option<()> = None;  // Placeholder type when feature disabled

    // ... rest of function
```

#### 2.3 Update tui/app.rs

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Conditionally compile feed-related fields and methods

**Imports:**
```rust
#[cfg(feature = "socket-feed")]
use crate::commands::spawn::feed::{
    create_agent_update, create_output_message, session_to_snapshot, FeedHandleSync,
    StatsSnapshot, TaskSnapshot, WaveSnapshot, WaveUpdate,
};
```

**App struct fields:**
```rust
pub struct App {
    // ... existing fields ...

    // === Socket Feed ===
    #[cfg(feature = "socket-feed")]
    feed_handle: Option<FeedHandleSync>,
    #[cfg(feature = "socket-feed")]
    previous_agent_statuses: HashMap<String, AgentStatus>,
    #[cfg(feature = "socket-feed")]
    last_feed_publish: Instant,
}
```

**Methods - wrap all feed-related methods:**
```rust
#[cfg(feature = "socket-feed")]
impl App {
    pub fn set_feed_handle(&mut self, handle: Option<FeedHandleSync>) {
        self.feed_handle = handle;
    }

    pub fn has_feed(&self) -> bool {
        self.feed_handle.is_some()
    }

    // ... other feed methods ...
}

#[cfg(not(feature = "socket-feed"))]
impl App {
    pub fn set_feed_handle(&mut self, _handle: Option<()>) {}
    pub fn has_feed(&self) -> bool { false }
}
```

#### 2.4 Update main.rs CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Conditionally include --feed flag

**In Monitor command:**
```rust
Monitor {
    #[arg(short, long)]
    session: Option<String>,

    /// Enable socket feed for external consumers (e.g., tcp://*:5555)
    #[cfg(feature = "socket-feed")]
    #[arg(short, long)]
    feed: Option<String>,
},
```

**In match arm:**
```rust
Commands::Monitor { session, #[cfg(feature = "socket-feed")] feed } => {
    #[cfg(feature = "socket-feed")]
    let feed_endpoint = feed;
    #[cfg(not(feature = "socket-feed"))]
    let feed_endpoint = None;

    commands::spawn::run_monitor(cli.project, session, feed_endpoint)
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds (no zeromq)
- [ ] `cargo build --features socket-feed` succeeds (with zeromq)
- [ ] `cargo test` passes in both configurations
- [ ] `cargo clippy` passes in both configurations

#### Manual Verification:
- [ ] `scud monitor --help` without feature shows no --feed option
- [ ] `scud monitor --help` with feature shows --feed option
- [ ] Monitor works normally without the feature
- [ ] Feed works when feature is enabled

---

## Phase 3: Update Documentation

### Overview
Document the feature flag in relevant places.

### Changes Required:

#### 3.1 Add feature documentation to Cargo.toml

Already done in Phase 1 with inline comment.

#### 3.2 Update README if socket feed is documented there

**File**: `scud-cli/README.md` (if applicable)
**Changes**: Note that socket-feed requires feature flag

```markdown
### Socket Feed (Optional)

The monitor can expose a ZeroMQ socket for external consumers:

```bash
# Build with socket feed support
cargo build --features socket-feed

# Use the feed
scud monitor --session my-session --feed tcp://*:5555
```
```

### Success Criteria:

#### Automated Verification:
- [ ] Documentation builds/renders correctly

---

## Alternative: Simpler Approach

If the above is too much boilerplate, a simpler alternative is to just make the dependency optional but keep all the code always compiled (with runtime checks):

```toml
zeromq = { version = "0.4", optional = true }

[features]
socket-feed = ["zeromq"]
```

Then in code, use `cfg!()` macro for runtime checks instead of `#[cfg()]` for compile-time:

```rust
if cfg!(feature = "socket-feed") && feed_endpoint.is_some() {
    // This code is always compiled but the branch is optimized out
    // when feature is disabled
}
```

However, this doesn't save compile time since the code is still compiled. The Phase 1-2 approach is more thorough.

## Testing Strategy

### Automated:
- Build without feature
- Build with feature
- Run tests in both configurations
- Clippy in both configurations

### Manual:
- Test monitor without --feed flag (both configs)
- Test monitor with --feed flag (socket-feed config only)

## References

- PR #10: https://github.com/pyrex41/scud/pull/10
- Branch: `claude/socket-feed-monitor-V2JHm`
- Cargo Features Documentation: https://doc.rust-lang.org/cargo/reference/features.html
