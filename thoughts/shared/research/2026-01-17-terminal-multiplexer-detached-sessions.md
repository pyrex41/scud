---
date: 2026-01-17T23:53:15+00:00
researcher: Claude
git_commit: 9a714524ede712921ba57237aa06624cca65ca27
branch: master
repository: scud
topic: "Terminal Multiplexer Detached Sessions - Current Implementation"
tags: [research, codebase, terminal, tmux, zellij, wezterm, spawn, swarm]
status: complete
last_updated: 2026-01-17
last_updated_by: Claude
---

# Research: Terminal Multiplexer Detached Sessions - Current Implementation

**Date**: 2026-01-17T23:53:15+00:00
**Researcher**: Claude
**Git Commit**: 9a714524ede712921ba57237aa06624cca65ca27
**Branch**: master
**Repository**: scud

## Research Question

How does SCUD currently handle terminal multiplexer sessions, and what is the current state of support for detached/background sessions (where tabs are available to attach to but don't require immediate visual display)?

## Summary

SCUD currently supports 5 terminal environments (Kitty, WezTerm, iTerm2, Zellij, Tmux) with **tmux being the most feature-complete for detached operation**. The current implementation:

1. **Tmux already supports detached sessions** - creates sessions with `-d` flag and allows later attachment
2. **Zellij spawns panes/sessions that can be attached later**, but requires being "inside" zellij for full pane management
3. **WezTerm, Kitty, iTerm2** spawn visible windows immediately (no true detached mode)

The tmux implementation is already the closest to "spawn in background, attach when wanted" - it creates named windows in a detached session and provides attach commands.

## Detailed Findings

### 1. Current Terminal Detection & Selection

**Location**: `scud-cli/src/commands/spawn/terminal.rs:218-242`

```rust
pub fn detect_terminal() -> Terminal {
    // Check for Kitty
    if std::env::var("KITTY_PID").is_ok() || std::env::var("KITTY_WINDOW_ID").is_ok() {
        return Terminal::Kitty;
    }
    // Check for WezTerm
    if std::env::var("WEZTERM_UNIX_SOCKET").is_ok() || std::env::var("WEZTERM_PANE").is_ok() {
        return Terminal::Wezterm;
    }
    // Check for iTerm2 (macOS)
    if std::env::var("ITERM_SESSION_ID").is_ok() {
        return Terminal::ITerm2;
    }
    // Check for Zellij
    if std::env::var("ZELLIJ").is_ok() || std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        return Terminal::Zellij;
    }
    // Default to tmux as universal fallback
    Terminal::Tmux
}
```

**Key observation**: Tmux is the default fallback, which is good since it's most suitable for detached operation.

### 2. Tmux Implementation (Best for Detached Sessions)

**Location**: `scud-cli/src/commands/spawn/terminal.rs:629-710`

The tmux spawning already works in detached mode:

```rust
fn spawn_tmux(task_id: &str, prompt: &str, working_dir: &Path, session_name: &str, ...) -> Result<()> {
    // Check if session exists
    let session_exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !session_exists {
        // Create new session DETACHED (-d flag)
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-n", "ctrl"])
            .arg("-c")
            .arg(working_dir)
            .status()?;
    }

    // Create new window for this task
    let new_window_output = Command::new("tmux")
        .args([
            "new-window",
            "-t", session_name,
            "-n", &window_name,
            "-P", "-F", "#{window_index}",
        ])
        .arg("-c")
        .arg(working_dir)
        .output()?;

    // Send command to the window
    let target = format!("{}:{}", session_name, window_index);
    Command::new("tmux")
        .args(["send-keys", "-t", &target, &full_cmd, "Enter"])
        .output()?;
}
```

**What this means**: Tmux sessions are created detached by default. The user sees output about the spawn but doesn't need to attach.

**Post-spawn output** (`scud-cli/src/commands/spawn/mod.rs:292-302`):
```rust
if terminal == Terminal::Tmux {
    println!();
    println!(
        "To attach: {}",
        format!("tmux attach -t {}", session_name).cyan()
    );
    println!(
        "To list:   {}",
        format!("tmux list-windows -t {}", session_name).dimmed()
    );
}
```

### 3. Zellij Implementation

**Location**: `scud-cli/src/commands/spawn/terminal.rs:468-573`

Zellij has two modes:

**Inside Zellij** (when `ZELLIJ` env var is set):
```rust
if inside_zellij {
    // Creates a new tab (may steal focus)
    let _ = Command::new("zellij")
        .args(["action", "new-tab", "--name", session_name])
        .output();

    // Spawns a new pane to the right
    let status = Command::new("zellij")
        .args([
            "action", "new-pane",
            "--name", &pane_name,
            "--direction", "right",
            "--", "bash", "-c", &bash_cmd,
        ])
        .status()?;
}
```

**Outside Zellij**:
```rust
else {
    // Start a new Zellij session with the command
    let status = Command::new("zellij")
        .args([
            "--session", session_name,
            "run", "--name", &pane_name,
            "--", "bash", "-c", &bash_cmd,
        ])
        .current_dir(working_dir)
        .status()?;
}
```

**Issue**: When outside zellij, `zellij --session <name> run` appears to be a foreground/blocking operation - it attaches to the session immediately.

### 4. WezTerm Implementation

**Location**: `scud-cli/src/commands/spawn/terminal.rs:368-401`

```rust
fn spawn_wezterm(...) -> Result<()> {
    let status = Command::new("wezterm")
        .args(["cli", "spawn", "--new-window"])
        .arg(format!("--cwd={}", working_dir.display()))
        .arg("--")
        .arg("bash")
        .arg("-lc")
        .arg(&bash_cmd)
        .status()?;
}
```

**Note**: WezTerm's `cli spawn --new-window` creates a new visible window. There's no built-in "detached" mode like tmux.

### 5. Control Window & Monitoring

**Location**: `scud-cli/src/commands/spawn/terminal.rs:964-977`

Tmux gets a special "ctrl" window for monitoring:

```rust
pub fn setup_tmux_control_window(session_name: &str, tag: &str) -> Result<()> {
    let control_script = format!(
        r#"watch -n 5 'echo "=== SCUD Spawn Monitor: {} ===" && echo && scud stats --tag {} && echo && scud whois --tag {} && echo && echo "Ready tasks:" && scud next-batch --tag {} --limit 5 2>/dev/null | head -20'"#,
        session_name, tag, tag, tag
    );

    let target = format!("{}:ctrl", session_name);
    Command::new("tmux")
        .args(["send-keys", "-t", &target, &control_script, "Enter"])
        .status()?;
}
```

### 6. Attach Functionality

**Tmux attach** (`terminal.rs:949-961`):
```rust
pub fn tmux_attach(session_name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["attach", "-t", session_name])
        .status()?;
}
```

**Zellij attach** (`terminal.rs:575-613`):
```rust
pub fn focus_zellij_pane(session_name: &str) -> Result<()> {
    if inside_zellij {
        // Switch to the tab with the given name
        Command::new("zellij")
            .args(["action", "go-to-tab-name", session_name])
            .status()?;
    } else {
        // Attach to the Zellij session from outside
        Command::new("zellij")
            .args(["attach", session_name])
            .status()?;
    }
}
```

### 7. Session Metadata Storage

**Location**: `scud-cli/src/commands/spawn/monitor.rs`

Sessions are tracked in `.scud/spawn/<session-name>.json`:
```rust
pub struct SpawnSession {
    pub session_name: String,
    pub tag: String,
    pub terminal: String,
    pub created_at: String,
    pub working_dir: String,
    pub agents: Vec<AgentState>,
}
```

This allows SCUD to know which agents are running even without terminal interaction.

## Architecture Documentation

### Current Terminal Behavior Matrix

| Terminal | Detached Spawn | Later Attach | Multi-Agent | Control Window |
|----------|----------------|--------------|-------------|----------------|
| **Tmux** | Yes (`-d`) | `tmux attach -t` | Yes (windows) | Yes (ctrl) |
| **Zellij** | Partial | `zellij attach` | Yes (panes) | No |
| **WezTerm** | No | N/A | Yes (windows) | No |
| **Kitty** | No | N/A | Yes (windows) | No |
| **iTerm2** | No | N/A | Yes (windows) | No |

### Spawn Flow (Tmux)

```
User runs: scud spawn --limit 3
    |
    v
spawn/mod.rs: detect_terminal() -> Tmux (or --terminal tmux)
    |
    v
terminal.rs: spawn_tmux()
    |
    +-- tmux new-session -d -s scud-<tag> -n ctrl  (detached!)
    |
    +-- For each task:
    |       tmux new-window -t scud-<tag> -n task-<id>
    |       tmux send-keys -t scud-<tag>:<idx> "<cmd>" Enter
    |
    v
Output: "To attach: tmux attach -t scud-<tag>"
```

### Spawn Flow (Zellij - Inside)

```
User runs: scud spawn --limit 3  (from within zellij)
    |
    v
detect_terminal() -> Zellij (ZELLIJ env var set)
    |
    v
terminal.rs: spawn_zellij()
    |
    +-- zellij action new-tab --name scud-<tag>  (may steal focus)
    |
    +-- For each task:
            zellij action new-pane --name task-<id> --direction right
```

## Code References

| File | Line | Description |
|------|------|-------------|
| `scud-cli/src/commands/spawn/terminal.rs` | 196-216 | Terminal enum definition |
| `scud-cli/src/commands/spawn/terminal.rs` | 218-242 | Auto-detection logic |
| `scud-cli/src/commands/spawn/terminal.rs` | 629-710 | Tmux spawning (detached) |
| `scud-cli/src/commands/spawn/terminal.rs` | 468-573 | Zellij spawning |
| `scud-cli/src/commands/spawn/terminal.rs` | 368-401 | WezTerm spawning |
| `scud-cli/src/commands/spawn/terminal.rs` | 949-961 | Tmux attach |
| `scud-cli/src/commands/spawn/terminal.rs` | 575-613 | Zellij focus/attach |
| `scud-cli/src/commands/spawn/mod.rs` | 292-302 | Post-spawn attach instructions |
| `scud-cli/src/commands/spawn/monitor.rs` | 32-41 | Session metadata structure |

## Related Research

- `thoughts/shared/research/2026-01-16-agent-types-model-routing.md` - Agent types and model routing

## Open Questions

1. **Zellij detached mode**: Can `zellij` be started in a truly detached way (like `tmux -d`) from outside a session? Current implementation seems to attach immediately.

2. **WezTerm multiplexing**: WezTerm has a multiplexer mode (`wezterm connect`) - could this provide detached operation?

3. **Session persistence**: What happens to tmux sessions after system restart? Should SCUD track "orphaned" sessions?

4. **Alacritty role**: Alacritty is just a terminal emulator, not a multiplexer - so it relies on running inside tmux/zellij for multi-agent work. The crash was likely an Alacritty or OS-level issue, not SCUD.
