# Descartes GUI Consolidation & Completion Plan

## Overview

Consolidate the workspace by removing the descartes CLI crate (all functionality exists in scud-cli), updating descartes-gui to depend only on scud-core, completing incomplete GUI features, and implementing the missing Claude Direct mode in scud-eval.

## Current State Analysis

### What Exists
- **scud-cli**: Full-featured CLI with swarm, spawn, ralph, waves, etc. Has harnesses (Claude, OpenCode)
- **scud-core**: Shared library with Task, Phase, Storage, compute_waves, SCG format
- **descartes-gui**: Iced 0.14 GUI that depends on descartes crate for Config only
- **descartes**: CLI crate with harnesses, specs, transcripts - redundant with scud-cli
- **scud-eval**: Evaluation framework, missing Claude Direct mode

### Key Discoveries
- `descartes-gui/src/main.rs:14` imports `descartes::{scud, Config}` but only uses:
  - `Config.harness.kind` (string: "claude-code" or "opencode")
  - `Config.swarm.round_size` (usize)
  - `Config.swarm.default_tag` (string)
- `descartes-gui/src/scud_bridge.rs` already uses `scud-core` directly for all operations
- The legacy `load_waves_from_scud()` function (lines 556-584) uses descartes::scud but is not the primary path
- scud-cli already has `Harness` enum supporting Claude and OpenCode
- Iced 0.14 is the latest stable version (released Dec 7, 2025)

## Desired End State

After this plan:
1. descartes crate is removed from the workspace
2. descartes-gui depends only on scud-core (and optionally scud-cli for types)
3. Swarm config is loaded from `.scud/config.toml` with sensible defaults
4. Single task execution (StartAgent) works in the GUI
5. Theme constants are used throughout views
6. scud-eval Claude Direct mode is implemented
7. Interactive REPL added to scud-cli

### Verification
- `cargo build --workspace` succeeds without descartes crate
- `descartes-gui` launches and can load tasks, compute waves, run swarms
- `scud-eval run --mode claude-direct --tasks eval-trivial` works
- `scud repl` starts an interactive session

## What We're NOT Doing

- Changing the descartes-gui name (keeping "Descartes" branding)
- Migrating descartes agents (using scud agents in `.scud/agents/`)
- Adding new GUI features beyond completing existing incomplete ones
- Changing SCG format or task model

---

## Phase 1: Extend scud-cli Config with Swarm Settings

### Overview
Add swarm configuration (harness, round_size, default_tag) to scud-cli's config system so descartes-gui can load from `.scud/config.toml`.

### Changes Required:

#### 1.1 Update scud-cli Config

**File**: `scud-cli/src/config.rs`

Add swarm configuration to the Config struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LLMConfig,
    #[serde(default)]
    pub swarm: SwarmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// Default harness for swarm execution
    #[serde(default = "default_harness")]
    pub harness: String,
    /// Number of agents per round
    #[serde(default = "default_round_size")]
    pub round_size: usize,
    /// Default tag when none specified
    #[serde(default = "default_tag")]
    pub default_tag: String,
}

fn default_harness() -> String {
    std::env::var("SCUD_HARNESS").unwrap_or_else(|_| "claude".to_string())
}

fn default_round_size() -> usize {
    std::env::var("SCUD_ROUND_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
}

fn default_tag() -> String {
    std::env::var("SCUD_DEFAULT_TAG").unwrap_or_else(|_| "default".to_string())
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            harness: default_harness(),
            round_size: default_round_size(),
            default_tag: default_tag(),
        }
    }
}
```

#### 1.2 Expose Config in scud-cli lib.rs

**File**: `scud-cli/src/lib.rs`

Ensure Config is publicly exported:

```rust
pub mod config;
pub use config::Config;
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` succeeds
- [ ] `cargo test -p scud-cli config` passes
- [ ] Existing configs without `[swarm]` section still load (backward compatible)

#### Manual Verification:
- [ ] `scud config show` displays swarm settings
- [ ] `.scud/config.toml` can include `[swarm]` section

---

## Phase 2: Update descartes-gui to Use scud-core Only

### Overview
Remove the descartes dependency from descartes-gui. Load configuration from `.scud/config.toml` via scud-cli's Config type or create minimal local config.

### Changes Required:

#### 2.1 Update Cargo.toml

**File**: `descartes-gui/Cargo.toml`

Replace descartes dependency with scud-cli:

```toml
[dependencies]
# Remove: descartes = { path = "../descartes", package = "descartes-cli" }
# Add:
scud = { path = "../scud-cli" }
scud-core = { path = "../scud-core" }
```

#### 2.2 Update state.rs

**File**: `descartes-gui/src/state.rs`

Change `SwarmDefaults::from_config` to use scud Config:

```rust
impl SwarmDefaults {
    /// Create SwarmDefaults from scud Config
    pub fn from_config(config: &scud::Config) -> Self {
        Self {
            harness: config.swarm.harness.clone(),
            round_size: config.swarm.round_size,
            default_tag: config.swarm.default_tag.clone(),
        }
    }

    /// Load from .scud/config.toml with defaults if not found
    pub fn load() -> Self {
        let config_path = std::env::current_dir()
            .map(|p| p.join(".scud/config.toml"))
            .ok();

        config_path
            .and_then(|p| scud::Config::load(&p).ok())
            .map(|c| Self::from_config(&c))
            .unwrap_or_default()
    }
}
```

#### 2.3 Update main.rs

**File**: `descartes-gui/src/main.rs`

Replace descartes imports with scud-core and scud:

```rust
// Remove: use descartes::{scud, Config};
// The scud_bridge already uses scud-core directly, so no scud module needed

use scud::Config;  // For SwarmDefaults::from_config
```

Update `new()` to load config from scud:

```rust
fn new() -> (Self, Task<Message>) {
    // Load configuration from .scud/config.toml
    let config_path = std::env::current_dir()
        .map(|p| p.join(".scud/config.toml"))
        .ok();

    let swarm_defaults = config_path
        .and_then(|p| scud::Config::load(&p).ok())
        .map(|c| SwarmDefaults::from_config(&c))
        .unwrap_or_else(|| {
            tracing::warn!("Failed to load config, using defaults");
            SwarmDefaults::default()
        });

    // ... rest unchanged
}
```

#### 2.4 Remove Legacy load_waves_from_scud Function

**File**: `descartes-gui/src/main.rs`

Delete lines 556-584 (the `load_waves_from_scud()` function) and remove `Message::LoadWaves` handler since it's superseded by ScudBridge.

Update enum to remove legacy message:
```rust
pub enum Message {
    // Navigation
    SwitchView(ViewMode),

    // Remove: LoadWaves,
    // Remove: WavesLoaded(Result<Vec<Vec<TaskInfo>>, String>),

    // ... rest unchanged
}
```

Remove handler in `update()`:
```rust
// Remove the Message::LoadWaves and Message::WavesLoaded arms
```

#### 2.5 Update tracing filter

**File**: `descartes-gui/src/main.rs:37`

Change filter from `descartes=info` to `scud=info`:

```rust
tracing_subscriber::fmt()
    .with_env_filter("descartes_gui=debug,scud=info")
    .init();
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p descartes-gui` succeeds without descartes dependency
- [ ] `cargo test -p descartes-gui` passes

#### Manual Verification:
- [ ] GUI launches without errors
- [ ] Tasks load correctly from `.scud/` storage
- [ ] Waves compute correctly
- [ ] Swarm starts with correct harness and round_size

---

## Phase 3: Remove descartes Crate from Workspace

### Overview
Delete the descartes crate entirely and update workspace configuration.

### Changes Required:

#### 3.1 Remove descartes from Workspace Members

**File**: `Cargo.toml` (workspace root)

```toml
[workspace]
resolver = "2"
members = ["scud-cli", "scud-core", "descartes-gui", "scud-eval"]
# Removed: "descartes"

[workspace.package]
version = "1.44.0"
edition = "2021"
license = "MIT"
authors = ["SCUD Team"]

[workspace.dependencies]
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
iced = { version = "0.14", features = ["tokio", "advanced"] }  # Updated from 0.12
```

#### 3.2 Delete descartes Directory

```bash
rm -rf descartes/
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes

#### Manual Verification:
- [ ] No orphaned references to descartes crate in any files

---

## Phase 4: Implement Single Task Execution (StartAgent)

### Overview
Complete the TODO at `descartes-gui/src/main.rs:285` to actually spawn an agent for a single task.

### Changes Required:

#### 4.1 Add RunTask Command to ScudBridge

**File**: `descartes-gui/src/scud_bridge.rs`

Add new command for single task execution:

```rust
pub enum ScudCommand {
    // ... existing commands ...

    /// Run a single task via scud ralph-style execution
    RunTask { task_id: String, harness: String },
}
```

Add handler in `run()`:

```rust
ScudCommand::RunTask { task_id, harness } => {
    self.run_single_task(&task_id, &harness).await;
}
```

Implement the method:

```rust
async fn run_single_task(&mut self, task_id: &str, harness: &str) {
    // Use scud spawn to run a single task
    let args = vec![
        "spawn".to_string(),
        "--task".to_string(),
        task_id.to_string(),
        "--harness".to_string(),
        harness.to_string(),
        "--json-events".to_string(),
    ];

    let mut cmd = tokio::process::Command::new("scud");
    cmd.args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    match cmd.spawn() {
        Ok(mut child) => {
            self.swarm_handle = Some(child);

            let _ = self.event_tx.send(ScudEvent::TaskStarted {
                task_id: task_id.to_string(),
            }).await;

            // Stream output (similar to run_swarm)
            if let Some(stdout) = child.stdout.take() {
                let reader = tokio::io::BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    // Parse JSON events or send as output
                    if let Ok(event) = serde_json::from_str::<ScudJsonEvent>(&line) {
                        let _ = self.event_tx.send(event.into()).await;
                    } else {
                        let _ = self.event_tx.send(ScudEvent::TaskOutput {
                            task_id: task_id.to_string(),
                            text: line,
                        }).await;
                    }
                }
            }

            // Wait for completion
            match child.wait().await {
                Ok(status) => {
                    let _ = self.event_tx.send(ScudEvent::TaskCompleted {
                        task_id: task_id.to_string(),
                        success: status.success(),
                    }).await;
                }
                Err(e) => {
                    let _ = self.event_tx.send(ScudEvent::Error(e.to_string())).await;
                }
            }

            self.swarm_handle = None;
        }
        Err(e) => {
            let _ = self.event_tx.send(ScudEvent::Error(
                format!("Failed to start task: {}", e)
            )).await;
        }
    }
}
```

#### 4.2 Update StartAgent Handler

**File**: `descartes-gui/src/main.rs`

Replace the TODO with actual implementation:

```rust
Message::StartAgent(task_id) => {
    self.state.agent_status = AgentStatus::Running;
    self.state.current_task = Some(task_id.clone());
    self.state.output_buffer.clear();

    if let Some(ref tx) = self.scud_command_tx {
        let tx = tx.clone();
        let harness = self.state.swarm_defaults.harness.clone();
        return Task::perform(
            async move {
                let _ = tx.send(ScudCommand::RunTask { task_id, harness }).await;
            },
            |_| Message::Tick,
        );
    }

    Task::none()
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p descartes-gui` succeeds
- [ ] Existing tests pass

#### Manual Verification:
- [ ] Click "Start" on a task in Waves view
- [ ] Agent output streams to Output view
- [ ] Task status updates when complete
- [ ] Pause/Resume/Cancel work during execution

---

## Phase 5: Apply Theme Constants

### Overview
Use the theme constants defined in `theme.rs` throughout the view modules.

### Changes Required:

#### 5.1 Update theme.rs to Export Constants

**File**: `descartes-gui/src/theme.rs`

Remove `#[allow(dead_code)]` and ensure all constants are pub:

```rust
//! Theme constants for consistent styling

use iced::Color;

pub const ACCENT: Color = Color::from_rgb(0.3, 0.5, 0.9);
pub const SUCCESS: Color = Color::from_rgb(0.3, 0.8, 0.4);
pub const WARNING: Color = Color::from_rgb(0.9, 0.7, 0.2);
pub const ERROR: Color = Color::from_rgb(0.9, 0.3, 0.3);

pub mod background {
    use iced::Color;
    pub const PRIMARY: Color = Color::from_rgb(0.1, 0.1, 0.12);
    pub const SECONDARY: Color = Color::from_rgb(0.15, 0.15, 0.18);
    pub const TERTIARY: Color = Color::from_rgb(0.2, 0.2, 0.24);
}

pub mod text {
    use iced::Color;
    pub const PRIMARY: Color = Color::from_rgb(0.9, 0.9, 0.9);
    pub const SECONDARY: Color = Color::from_rgb(0.7, 0.7, 0.7);
    pub const MUTED: Color = Color::from_rgb(0.5, 0.5, 0.5);
}
```

#### 5.2 Apply Theme in Views

**File**: `descartes-gui/src/main.rs`

Replace inline colors with theme constants in error banner:

```rust
// Line 471-484: Replace inline colors
let error_banner = container(
    row![
        text(error).style(|_| text::Style {
            color: Some(crate::theme::ERROR),
        }),
        button("Dismiss").on_press(Message::DismissError),
    ]
    .spacing(10),
)
.padding(10)
.style(|_| container::Style {
    background: Some(iced::Background::Color(crate::theme::background::SECONDARY)),
    ..Default::default()
});
```

**File**: `descartes-gui/src/views/output.rs`

Apply theme to output container styling.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo clippy -p descartes-gui -- -D warnings` shows no unused constant warnings
- [ ] `cargo build -p descartes-gui` succeeds

#### Manual Verification:
- [ ] Error banner uses theme ERROR color
- [ ] Output view uses theme background
- [ ] Visual consistency across views

---

## Phase 6: Implement scud-eval Claude Direct Mode

### Overview
Implement `run_claude_direct()` in scud-eval to run a single Claude Code session that completes all tasks.

### Changes Required:

#### 6.1 Implement run_claude_direct

**File**: `scud-eval/src/runner.rs`

Replace the placeholder with actual implementation:

```rust
pub fn run_claude_direct(workspace: &Path, taskset: &TaskSet) -> Result<()> {
    let started_at = std::time::Instant::now();
    println!("Running Claude Direct mode for taskset: {}", taskset.name);

    // Generate a comprehensive prompt for single-session completion
    let prompt = generate_direct_prompt(taskset);

    // Write prompt to temp file for reference
    let prompt_file = workspace.join("eval-prompt.md");
    std::fs::write(&prompt_file, &prompt)?;

    // Build claude command
    let mut cmd = std::process::Command::new("claude");
    cmd.current_dir(workspace)
        .arg("-p")
        .arg(&prompt)
        .arg("--dangerously-skip-permissions")
        .arg("--output-format")
        .arg("stream-json");

    // Run and stream output
    println!("Starting Claude Code session...");
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    if let Some(stdout) = child.stdout.take() {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                // Print progress
                if line.contains("\"type\":\"assistant\"") {
                    print!(".");
                    std::io::stdout().flush()?;
                }
            }
        }
    }
    println!();

    let status = child.wait()?;
    let elapsed = started_at.elapsed();

    if status.success() {
        println!("Claude Direct completed successfully in {:.1}s", elapsed.as_secs_f64());
    } else {
        println!("Claude Direct failed with status: {:?}", status.code());
    }

    Ok(())
}

fn generate_direct_prompt(taskset: &TaskSet) -> String {
    format!(r#"# Evaluation Task Set: {}

{}

## Instructions

You are completing a benchmark evaluation. Complete ALL tasks below in dependency order.

**Rules:**
1. Complete each task fully before moving to the next
2. After completing each task, commit with message: `[TASK-ID] description`
3. Mark each task done: `scud set-status <id> done`
4. Continue until ALL tasks show status "done"
5. Your context will auto-compact as needed - this is normal

## Task Graph (SCG format)

```
{}
```

## Start Now

Begin with tasks that have no dependencies. Work through the entire task set until complete.
"#,
        taskset.name,
        taskset.description,
        taskset.scg_content,
    )
}
```

#### 6.2 Update cmd_run to Use Claude Direct

**File**: `scud-eval/src/main.rs`

The existing dispatch already calls `run_claude_direct`, just need to pass taskset:

```rust
ExecutionMode::ClaudeDirect => {
    let taskset = tasksets::builtin_tasksets()
        .into_iter()
        .find(|t| t.name == config.taskset_name)
        .ok_or_else(|| anyhow::anyhow!("Taskset not found: {}", config.taskset_name))?;
    runner::run_claude_direct(&workspace.path, &taskset)?;
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-eval` succeeds
- [ ] `cargo test -p scud-eval` passes

#### Manual Verification:
- [ ] `scud-eval run --mode claude-direct --tasks eval-trivial` launches Claude Code
- [ ] Tasks are completed in the workspace
- [ ] Commits are created with task ID prefixes

---

## Phase 7: Add Interactive REPL to scud-cli

### Overview
Add a `scud repl` command that provides an interactive session for task management.

### Changes Required:

#### 7.1 Create REPL Module

**File**: `scud-cli/src/commands/repl.rs`

```rust
//! Interactive REPL for SCUD task management

use anyhow::Result;
use std::io::{self, BufRead, Write};

use crate::storage::Storage;
use crate::models::task::TaskStatus;

pub fn run() -> Result<()> {
    println!("SCUD Interactive REPL");
    println!("Type 'help' for commands, 'quit' to exit");
    println!();

    let storage = Storage::new(None);
    let stdin = io::stdin();

    loop {
        print!("scud> ");
        io::stdout().flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts.first().map(|s| *s).unwrap_or("");
        let args = &parts[1..];

        match cmd {
            "help" | "?" => print_help(),
            "quit" | "exit" | "q" => {
                println!("Goodbye!");
                break;
            }
            "list" | "ls" => cmd_list(&storage, args)?,
            "next" => cmd_next(&storage)?,
            "show" => cmd_show(&storage, args)?,
            "status" => cmd_status(&storage, args)?,
            "waves" => cmd_waves(&storage)?,
            "stats" => cmd_stats(&storage)?,
            _ => println!("Unknown command: {}. Type 'help' for commands.", cmd),
        }
    }

    Ok(())
}

fn print_help() {
    println!("Commands:");
    println!("  list [status]     - List tasks (optional: pending, done, etc.)");
    println!("  next              - Show next available task");
    println!("  show <id>         - Show task details");
    println!("  status <id> <s>   - Set task status (pending, in-progress, done, etc.)");
    println!("  waves             - Show parallel execution waves");
    println!("  stats             - Show completion statistics");
    println!("  help              - Show this help");
    println!("  quit              - Exit REPL");
}

fn cmd_list(storage: &Storage, args: &[&str]) -> Result<()> {
    let phase = storage.load_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    let status_filter = args.first().map(|s| *s);

    for task in &phase.tasks {
        let status_str = format!("{:?}", task.status).to_lowercase();
        if status_filter.is_none() || status_str.contains(status_filter.unwrap()) {
            println!("{:12} {:10} {}", task.id, status_str, task.title);
        }
    }
    Ok(())
}

fn cmd_next(storage: &Storage) -> Result<()> {
    let phase = storage.load_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    if let Some(task) = phase.find_next_task() {
        println!("Next task: {} - {}", task.id, task.title);
        println!("Complexity: {}, Priority: {:?}", task.complexity, task.priority);
        if !task.description.is_empty() {
            println!("\n{}", task.description);
        }
    } else {
        println!("No tasks available (all done or blocked)");
    }
    Ok(())
}

fn cmd_show(storage: &Storage, args: &[&str]) -> Result<()> {
    let task_id = args.first()
        .ok_or_else(|| anyhow::anyhow!("Usage: show <task_id>"))?;

    let phase = storage.load_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    let task = phase.get_task(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

    println!("ID:          {}", task.id);
    println!("Title:       {}", task.title);
    println!("Status:      {:?}", task.status);
    println!("Complexity:  {}", task.complexity);
    println!("Priority:    {:?}", task.priority);
    if !task.dependencies.is_empty() {
        println!("Depends on:  {}", task.dependencies.join(", "));
    }
    if !task.description.is_empty() {
        println!("\nDescription:\n{}", task.description);
    }

    Ok(())
}

fn cmd_status(storage: &Storage, args: &[&str]) -> Result<()> {
    if args.len() < 2 {
        anyhow::bail!("Usage: status <task_id> <status>");
    }

    let task_id = args[0];
    let status_str = args[1];

    let status = match status_str.to_lowercase().as_str() {
        "pending" | "p" => TaskStatus::Pending,
        "in-progress" | "ip" | "i" => TaskStatus::InProgress,
        "done" | "d" => TaskStatus::Done,
        "blocked" | "b" => TaskStatus::Blocked,
        "review" | "r" => TaskStatus::Review,
        _ => anyhow::bail!("Unknown status: {}", status_str),
    };

    let tag = storage.get_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    storage.update_task_status(&tag, task_id, status)?;
    println!("Updated {} to {:?}", task_id, status);

    Ok(())
}

fn cmd_waves(storage: &Storage) -> Result<()> {
    let phase = storage.load_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    let actionable = phase.get_actionable_tasks();
    let pending: Vec<_> = actionable.iter()
        .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Failed))
        .collect();

    let waves = scud_core::compute_waves(&pending);

    for (i, wave) in waves.waves.iter().enumerate() {
        println!("Wave {}: {}", i + 1, wave.tasks.join(", "));
    }

    if !waves.circular_deps.is_empty() {
        println!("\nCircular dependencies: {}", waves.circular_deps.join(", "));
    }

    Ok(())
}

fn cmd_stats(storage: &Storage) -> Result<()> {
    let phase = storage.load_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active group"))?;

    let stats = phase.get_stats();

    println!("Total:       {}", stats.total);
    println!("Pending:     {}", stats.pending);
    println!("In Progress: {}", stats.in_progress);
    println!("Done:        {}", stats.done);
    println!("Blocked:     {}", stats.blocked);

    if stats.total > 0 {
        let progress = (stats.done as f64 / stats.total as f64) * 100.0;
        println!("\nProgress:    {:.1}%", progress);
    }

    Ok(())
}
```

#### 7.2 Register REPL Command

**File**: `scud-cli/src/main.rs`

Add the command to the CLI:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Interactive REPL for task management
    Repl,
}

// In match arms:
Commands::Repl => commands::repl::run()?,
```

#### 7.3 Add Module Declaration

**File**: `scud-cli/src/commands/mod.rs`

```rust
pub mod repl;
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` succeeds
- [ ] `cargo test -p scud-cli` passes

#### Manual Verification:
- [ ] `scud repl` starts interactive session
- [ ] Commands work: list, next, show, status, waves, stats
- [ ] Ctrl+D or 'quit' exits cleanly

---

## Testing Strategy

### Unit Tests
- Config serialization/deserialization with new swarm fields
- SwarmDefaults::from_config() with scud::Config
- REPL command parsing

### Integration Tests
- GUI launches without descartes dependency
- ScudBridge task operations work
- scud-eval Claude Direct mode executes

### Manual Testing Steps
1. `cargo build --workspace` - Verify workspace builds
2. `descartes-gui` - Launch GUI, load tasks, compute waves
3. Click "Start" on a task - Verify single task execution
4. Click "Start Swarm" - Verify swarm runs with correct settings
5. `scud-eval run --mode claude-direct --tasks eval-trivial` - Verify Claude Direct
6. `scud repl` - Test interactive commands

## Migration Notes

### For Users
- Move `.descartes/config.toml` swarm settings to `.scud/config.toml`:
  ```toml
  [swarm]
  harness = "claude"
  round_size = 3
  default_tag = "refactor"
  ```
- Agent definitions should be in `.scud/agents/` (not `.descartes/agents/`)

### For Developers
- `descartes` crate is removed - use `scud-cli` for config, `scud-core` for types
- GUI imports change from `descartes::` to `scud::` and `scud_core::`

## References

- Research: `thoughts/shared/research/2026-01-23-descartes-eval-crate-review.md`
- Previous plan: `thoughts/shared/plans/2026-01-22-scud-descartes-merger-sprites-integration.md`
- GUI code review: `thoughts/shared/research/2026-01-22-descartes-scud-gui-code-review.md`
- Iced 0.14 release: https://github.com/iced-rs/iced/releases/tag/0.14.0
