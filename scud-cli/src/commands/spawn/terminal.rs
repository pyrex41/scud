//! Terminal detection and spawning functionality
//!
//! Supports Kitty, WezTerm, iTerm2, Zellij, and tmux with auto-detection based on environment variables.
//! Supports multiple AI harnesses: Claude Code, OpenCode.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// Supported AI coding harnesses
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Harness {
    /// Claude Code CLI (default)
    #[default]
    Claude,
    /// OpenCode CLI
    OpenCode,
}

impl Harness {
    /// Parse harness from string
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Harness::Claude),
            "opencode" | "open-code" => Ok(Harness::OpenCode),
            other => anyhow::bail!(
                "Unknown harness: '{}'. Supported: claude, opencode",
                other
            ),
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
        }
    }

    /// Binary name to search for
    pub fn binary_name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
        }
    }

    /// Generate the command to run with a prompt and optional model
    pub fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
        match self {
            Harness::Claude => {
                let model_flag = model
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                format!(
                    r#"'{}' "$(cat '{}')" --dangerously-skip-permissions{}"#,
                    binary_path,
                    prompt_file.display(),
                    model_flag
                )
            }
            Harness::OpenCode => {
                let model_flag = model
                    .map(|m| format!(" --model {}", m))
                    .unwrap_or_default();
                format!(
                    r#"'{}'{} run "$(cat '{}')""#,
                    binary_path,
                    model_flag,
                    prompt_file.display()
                )
            }
        }
    }
}

/// Cached paths to harness binaries
static CLAUDE_PATH: OnceLock<String> = OnceLock::new();
static OPENCODE_PATH: OnceLock<String> = OnceLock::new();

/// Generate shell initialization prefix that sources user's profile.
/// This ensures PATH and other environment variables are properly set up
/// in spawned shells (which don't run as login shells).
fn shell_init_prefix() -> &'static str {
    // Source common shell initialization files to set up PATH
    // This handles cases where node, python, etc. are installed via nvm, pyenv, homebrew, etc.
    r#"
# Source shell profile for PATH setup
[ -f /etc/profile ] && . /etc/profile
[ -f ~/.profile ] && . ~/.profile
[ -f ~/.bash_profile ] && . ~/.bash_profile
[ -f ~/.bashrc ] && . ~/.bashrc
[ -f ~/.zshrc ] && . ~/.zshrc 2>/dev/null
# Add common paths
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
# Source nvm if present
[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh"
# Source bun if present
[ -s "$HOME/.bun/_bun" ] && . "$HOME/.bun/_bun"
export PATH="$HOME/.bun/bin:$PATH"
"#
}

/// Find the full path to a harness binary.
/// Caches the result for subsequent calls.
pub fn find_harness_binary(harness: Harness) -> Result<&'static str> {
    let cache = match harness {
        Harness::Claude => &CLAUDE_PATH,
        Harness::OpenCode => &OPENCODE_PATH,
    };

    // Check if already cached
    if let Some(path) = cache.get() {
        return Ok(path.as_str());
    }

    let binary_name = harness.binary_name();

    // Try `which <binary>` to find it in PATH
    let output = Command::new("which")
        .arg(binary_name)
        .output()
        .context(format!("Failed to run 'which {}'", binary_name))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            // Cache and return
            let _ = cache.set(path);
            return Ok(cache.get().unwrap().as_str());
        }
    }

    // Common installation paths as fallback
    let common_paths: &[&str] = match harness {
        Harness::Claude => &[
            "/opt/homebrew/bin/claude",
            "/usr/local/bin/claude",
            "/usr/bin/claude",
        ],
        Harness::OpenCode => &[
            "/opt/homebrew/bin/opencode",
            "/usr/local/bin/opencode",
            "/usr/bin/opencode",
        ],
    };

    for path in common_paths {
        if std::path::Path::new(path).exists() {
            let _ = cache.set(path.to_string());
            return Ok(cache.get().unwrap().as_str());
        }
    }

    // Try home-relative paths
    if let Ok(home) = std::env::var("HOME") {
        let home_paths: Vec<String> = match harness {
            Harness::Claude => vec![
                format!("{}/.local/bin/claude", home),
                format!("{}/.claude/local/claude", home),
            ],
            Harness::OpenCode => vec![
                format!("{}/.local/bin/opencode", home),
                format!("{}/.bun/bin/opencode", home),
            ],
        };

        for path in home_paths {
            if std::path::Path::new(&path).exists() {
                let _ = cache.set(path);
                return Ok(cache.get().unwrap().as_str());
            }
        }
    }

    let install_hint = match harness {
        Harness::Claude => "Install with: npm install -g @anthropic-ai/claude-code",
        Harness::OpenCode => "Install with: curl -fsSL https://opencode.ai/install | bash",
    };

    anyhow::bail!(
        "Could not find '{}' binary. Please ensure it is installed and in PATH.\n{}",
        binary_name,
        install_hint
    )
}

/// Find the full path to the claude binary (convenience wrapper).
pub fn find_claude_binary() -> Result<&'static str> {
    find_harness_binary(Harness::Claude)
}

/// Supported terminal emulators
#[derive(Debug, Clone, PartialEq)]
pub enum Terminal {
    Kitty,
    Wezterm,
    ITerm2,
    Zellij,
    Tmux,
}

impl Terminal {
    /// Display name for the terminal
    pub fn name(&self) -> &'static str {
        match self {
            Terminal::Kitty => "kitty",
            Terminal::Wezterm => "wezterm",
            Terminal::ITerm2 => "iterm2",
            Terminal::Zellij => "zellij",
            Terminal::Tmux => "tmux",
        }
    }
}

/// Detect the current terminal emulator from environment variables
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

/// Parse terminal name from string argument
pub fn parse_terminal(name: &str) -> Result<Terminal> {
    match name.to_lowercase().as_str() {
        "kitty" => Ok(Terminal::Kitty),
        "wezterm" => Ok(Terminal::Wezterm),
        "iterm" | "iterm2" => Ok(Terminal::ITerm2),
        "zellij" => Ok(Terminal::Zellij),
        "tmux" => Ok(Terminal::Tmux),
        "auto" => Ok(detect_terminal()),
        other => anyhow::bail!(
            "Unknown terminal: {}. Supported: kitty, wezterm, iterm2, zellij, tmux, auto",
            other
        ),
    }
}

/// Check if required terminal binary is available
pub fn check_terminal_available(terminal: &Terminal) -> Result<()> {
    let binary = match terminal {
        Terminal::Kitty => "kitty",
        Terminal::Wezterm => "wezterm",
        Terminal::ITerm2 => "osascript", // iTerm2 uses AppleScript
        Terminal::Zellij => "zellij",
        Terminal::Tmux => "tmux",
    };

    let result = Command::new("which")
        .arg(binary)
        .output()
        .context(format!("Failed to check for {} binary", binary))?;

    if !result.status.success() {
        anyhow::bail!("{} is not installed or not in PATH", binary);
    }

    Ok(())
}

/// Spawn a new terminal window/pane with the given command
pub fn spawn_terminal(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
) -> Result<()> {
    // Default to Claude harness for backwards compatibility
    spawn_terminal_with_harness_and_model(terminal, task_id, prompt, working_dir, session_name, Harness::Claude, None)
}

/// Spawn a new terminal window/pane with the given command using a specific harness
pub fn spawn_terminal_with_harness(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
) -> Result<()> {
    spawn_terminal_with_harness_and_model(terminal, task_id, prompt, working_dir, session_name, harness, None)
}

/// Spawn a new terminal window/pane with the given command using a specific harness and model
pub fn spawn_terminal_with_harness_and_model(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
    model: Option<&str>,
) -> Result<()> {
    // Find harness binary path upfront to fail fast if not found
    let binary_path = find_harness_binary(harness)?;

    match terminal {
        Terminal::Kitty => spawn_kitty(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::Wezterm => spawn_wezterm(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::ITerm2 => spawn_iterm2(task_id, prompt, working_dir, binary_path, harness, model),
        Terminal::Zellij => spawn_zellij(task_id, prompt, working_dir, session_name, binary_path, harness, model),
        Terminal::Tmux => spawn_tmux(task_id, prompt, working_dir, session_name, binary_path, harness, model),
    }
}

/// Spawn in Kitty terminal using remote control
fn spawn_kitty(task_id: &str, prompt: &str, working_dir: &Path, binary_path: &str, harness: Harness, model: Option<&str>) -> Result<()> {
    let title = format!("task-{}", task_id);

    // Write prompt to temp file to avoid shell escaping issues
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Interactive mode with SCUD_TASK_ID for hook integration
    // The Stop hook will read SCUD_TASK_ID and auto-complete the task
    // Use full path to harness binary to avoid PATH issues in spawned shells
    // Source shell profile to ensure PATH includes node, etc.
    let harness_cmd = harness.command(binary_path, &prompt_file, model);
    let bash_cmd = format!(
        r#"{init}
export SCUD_TASK_ID='{task_id}' ; {cmd} ; rm -f '{prompt}' ; exec bash"#,
        init = shell_init_prefix(),
        task_id = task_id,
        cmd = harness_cmd,
        prompt = prompt_file.display()
    );

    let status = Command::new("kitty")
        .args(["@", "launch", "--type=window"])
        .arg(format!("--title={}", title))
        .arg(format!("--cwd={}", working_dir.display()))
        .arg("bash")
        .arg("-lc")
        .arg(&bash_cmd)
        .status()
        .context("Failed to spawn Kitty window")?;

    if !status.success() {
        anyhow::bail!("Kitty launch failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Spawn in WezTerm terminal
fn spawn_wezterm(task_id: &str, prompt: &str, working_dir: &Path, binary_path: &str, harness: Harness, model: Option<&str>) -> Result<()> {
    // Write prompt to temp file to avoid shell escaping issues
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Interactive mode with SCUD_TASK_ID for hook integration
    // Use full path to harness binary to avoid PATH issues in spawned shells
    // Source shell profile to ensure PATH includes node, etc.
    let harness_cmd = harness.command(binary_path, &prompt_file, model);
    let bash_cmd = format!(
        r#"{init}
export SCUD_TASK_ID='{task_id}' ; {cmd} ; rm -f '{prompt}' ; exec bash"#,
        init = shell_init_prefix(),
        task_id = task_id,
        cmd = harness_cmd,
        prompt = prompt_file.display()
    );

    let status = Command::new("wezterm")
        .args(["cli", "spawn", "--new-window"])
        .arg(format!("--cwd={}", working_dir.display()))
        .arg("--")
        .arg("bash")
        .arg("-lc")
        .arg(&bash_cmd)
        .status()
        .context("Failed to spawn WezTerm window")?;

    if !status.success() {
        anyhow::bail!("WezTerm spawn failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Spawn in iTerm2 on macOS using AppleScript
fn spawn_iterm2(task_id: &str, prompt: &str, working_dir: &Path, binary_path: &str, harness: Harness, model: Option<&str>) -> Result<()> {
    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    let title = format!("task-{}", task_id);
    // Interactive mode with SCUD_TASK_ID for hook integration
    // Use full path to harness binary to avoid PATH issues
    // Source shell profile to ensure PATH includes node, etc.
    // Note: AppleScript requires different escaping, so we build the command manually here
    let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
    let harness_cmd = match harness {
        Harness::Claude => format!(
            r#"'{}' \"$(cat '{}')\" --dangerously-skip-permissions{}"#,
            binary_path,
            prompt_file.display(),
            model_flag
        ),
        Harness::OpenCode => format!(
            r#"'{}'{} run \"$(cat '{}')\""#,
            binary_path,
            model_flag,
            prompt_file.display()
        ),
    };
    // For iTerm2, source profile inline (can't use multi-line easily in AppleScript)
    let full_cmd = format!(
        r#"source ~/.bash_profile 2>/dev/null; source ~/.zshrc 2>/dev/null; export PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; [ -s \"$HOME/.nvm/nvm.sh\" ] && source \"$HOME/.nvm/nvm.sh\"; cd '{}' && export SCUD_TASK_ID='{}' && {} ; rm -f '{}'"#,
        working_dir.display(),
        task_id,
        harness_cmd,
        prompt_file.display()
    );

    let script = format!(
        r#"tell application "iTerm"
    create window with default profile
    tell current session of current window
        set name to "{}"
        write text "{}"
    end tell
end tell"#,
        title,
        full_cmd.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .context("Failed to spawn iTerm2 window")?;

    if !status.success() {
        anyhow::bail!("iTerm2 spawn failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Spawn in Zellij terminal using pane management
///
/// Creates a tab if needed via `zellij action new-tab --name <session>`,
/// then spawns a named pane via `zellij action new-pane --name task-{id} --direction right`.
/// Sets SCUD_TASK_ID environment variable for hook integration.
fn spawn_zellij(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    binary_path: &str,
    harness: Harness,
    model: Option<&str>,
) -> Result<()> {
    let pane_name = format!("task-{}", task_id);

    // Write prompt to temp file to avoid shell escaping issues
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Check if we're inside a Zellij session
    let inside_zellij = std::env::var("ZELLIJ").is_ok();

    if inside_zellij {
        // We're inside Zellij - use `zellij action` commands

        // First, try to create a new tab with the session name if it doesn't exist
        // Zellij doesn't have a way to check if a tab exists, so we just try to
        // create one and use the current tab if spawning in an existing session
        let _ = Command::new("zellij")
            .args(["action", "new-tab", "--name", session_name])
            .output();

        // Interactive mode with SCUD_TASK_ID for hook integration
        // Use full path to harness binary to avoid PATH issues in spawned shells
        // Source shell profile to ensure PATH includes node, etc.
        let harness_cmd = harness.command(binary_path, &prompt_file, model);
        let bash_cmd = format!(
            r#"{init}
cd '{wd}' && export SCUD_TASK_ID='{task_id}' ; {cmd} ; rm -f '{prompt}' ; exec bash"#,
            init = shell_init_prefix(),
            wd = working_dir.display(),
            task_id = task_id,
            cmd = harness_cmd,
            prompt = prompt_file.display()
        );

        // Spawn a new pane to the right with the task name
        let status = Command::new("zellij")
            .args([
                "action",
                "new-pane",
                "--name",
                &pane_name,
                "--direction",
                "right",
                "--",
                "bash",
                "-c",
                &bash_cmd,
            ])
            .status()
            .context("Failed to spawn Zellij pane")?;

        if !status.success() {
            anyhow::bail!("Zellij pane spawn failed with exit code: {:?}", status.code());
        }
    } else {
        // We're outside Zellij - need to start a new session or attach to existing one
        // Use `zellij run` to spawn with a command in a new session
        // Use full path to harness binary to avoid PATH issues in spawned shells
        // Source shell profile to ensure PATH includes node, etc.

        let harness_cmd = harness.command(binary_path, &prompt_file, model);
        let bash_cmd = format!(
            r#"{init}
cd '{wd}' && export SCUD_TASK_ID='{task_id}' ; {cmd} ; rm -f '{prompt}' ; exec bash"#,
            init = shell_init_prefix(),
            wd = working_dir.display(),
            task_id = task_id,
            cmd = harness_cmd,
            prompt = prompt_file.display()
        );

        // Start a new Zellij session with the command
        let status = Command::new("zellij")
            .args([
                "--session",
                session_name,
                "run",
                "--name",
                &pane_name,
                "--",
                "bash",
                "-c",
                &bash_cmd,
            ])
            .current_dir(working_dir)
            .status()
            .context("Failed to start Zellij session")?;

        if !status.success() {
            anyhow::bail!(
                "Zellij session start failed with exit code: {:?}",
                status.code()
            );
        }
    }

    Ok(())
}

/// Focus a Zellij pane by name for attach functionality
///
/// Uses `zellij action go-to-tab-name` to switch to the tab containing the pane.
pub fn focus_zellij_pane(session_name: &str) -> Result<()> {
    // Check if we're inside Zellij
    let inside_zellij = std::env::var("ZELLIJ").is_ok();

    if inside_zellij {
        // Switch to the tab with the given name
        let status = Command::new("zellij")
            .args(["action", "go-to-tab-name", session_name])
            .status()
            .context("Failed to switch Zellij tab")?;

        if !status.success() {
            anyhow::bail!(
                "Failed to switch to Zellij tab '{}': exit code {:?}",
                session_name,
                status.code()
            );
        }
    } else {
        // Attach to the Zellij session from outside
        let status = Command::new("zellij")
            .args(["attach", session_name])
            .status()
            .context("Failed to attach to Zellij session")?;

        if !status.success() {
            anyhow::bail!(
                "Failed to attach to Zellij session '{}': exit code {:?}",
                session_name,
                status.code()
            );
        }
    }

    Ok(())
}

/// Check if a Zellij session exists
pub fn zellij_session_exists(session_name: &str) -> bool {
    Command::new("zellij")
        .args(["list-sessions"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim() == session_name || line.starts_with(&format!("{} ", session_name)))
        })
        .unwrap_or(false)
}

/// Spawn in tmux session
fn spawn_tmux(task_id: &str, prompt: &str, working_dir: &Path, session_name: &str, binary_path: &str, harness: Harness, model: Option<&str>) -> Result<()> {
    let window_name = format!("task-{}", task_id);

    // Check if session exists
    let session_exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !session_exists {
        // Create new session with control window
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-n", "ctrl"])
            .arg("-c")
            .arg(working_dir)
            .status()
            .context("Failed to create tmux session")?;
    }

    // Create new window for this task and capture its index
    // Use -P -F to print the new window's index
    let new_window_output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            session_name,
            "-n",
            &window_name,
            "-P", // Print info about new window
            "-F",
            "#{window_index}", // Format: just the index
        ])
        .arg("-c")
        .arg(working_dir)
        .output()
        .context("Failed to create tmux window")?;

    if !new_window_output.status.success() {
        anyhow::bail!(
            "Failed to create window: {}",
            String::from_utf8_lossy(&new_window_output.stderr)
        );
    }

    let window_index = String::from_utf8_lossy(&new_window_output.stdout)
        .trim()
        .to_string();

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Send command to the window BY INDEX (not name, which can be ambiguous)
    // Interactive mode with SCUD_TASK_ID for hook integration
    // Use full path to harness binary to avoid PATH issues in spawned shells
    // Source shell profile to ensure PATH includes node, etc.
    let harness_cmd = harness.command(binary_path, &prompt_file, model);
    // For tmux, we send a multi-line script via send-keys
    // First source profiles, then run the harness command
    let full_cmd = format!(
        r#"source ~/.bash_profile 2>/dev/null; source ~/.zshrc 2>/dev/null; export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"; [ -s "$HOME/.nvm/nvm.sh" ] && source "$HOME/.nvm/nvm.sh"; export SCUD_TASK_ID='{}' ; {} ; rm -f '{}'"#,
        task_id,
        harness_cmd,
        prompt_file.display()
    );

    let target = format!("{}:{}", session_name, window_index);
    let send_result = Command::new("tmux")
        .args(["send-keys", "-t", &target, &full_cmd, "Enter"])
        .output()
        .context("Failed to send command to tmux window")?;

    if !send_result.status.success() {
        anyhow::bail!(
            "Failed to send keys: {}",
            String::from_utf8_lossy(&send_result.stderr)
        );
    }

    Ok(())
}

/// Spawn a new terminal window/pane with Ralph loop enabled
/// The agent will keep running until the completion promise is detected
pub fn spawn_terminal_ralph(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
) -> Result<()> {
    // Default to Claude harness
    spawn_terminal_ralph_with_harness(
        terminal,
        task_id,
        prompt,
        working_dir,
        session_name,
        completion_promise,
        Harness::Claude,
    )
}

/// Spawn a new terminal window/pane with Ralph loop enabled using a specific harness
pub fn spawn_terminal_ralph_with_harness(
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
    harness: Harness,
) -> Result<()> {
    // Find harness binary path upfront to fail fast if not found
    let binary_path = find_harness_binary(harness)?;

    match terminal {
        Terminal::Tmux => spawn_tmux_ralph(
            task_id,
            prompt,
            working_dir,
            session_name,
            completion_promise,
            binary_path,
            harness,
        ),
        // For other terminals, fall back to regular spawn
        // Ralph loop requires bash scripting that's easier in tmux
        _ => spawn_terminal_with_harness(terminal, task_id, prompt, working_dir, session_name, harness),
    }
}

/// Spawn in tmux session with Ralph loop wrapper
fn spawn_tmux_ralph(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
    binary_path: &str,
    harness: Harness,
) -> Result<()> {
    let window_name = format!("ralph-{}", task_id);

    // Check if session exists
    let session_exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !session_exists {
        // Create new session with control window
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-n", "ctrl"])
            .arg("-c")
            .arg(working_dir)
            .status()
            .context("Failed to create tmux session")?;
    }

    // Create new window for this task
    let new_window_output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            session_name,
            "-n",
            &window_name,
            "-P",
            "-F",
            "#{window_index}",
        ])
        .arg("-c")
        .arg(working_dir)
        .output()
        .context("Failed to create tmux window")?;

    if !new_window_output.status.success() {
        anyhow::bail!(
            "Failed to create window: {}",
            String::from_utf8_lossy(&new_window_output.stderr)
        );
    }

    let window_index = String::from_utf8_lossy(&new_window_output.stdout)
        .trim()
        .to_string();

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-ralph-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Build the harness-specific command for the ralph script
    // We need to inline this since the script is a bash heredoc
    let harness_cmd = match harness {
        Harness::Claude => format!(
            "'{binary_path}' \"$(cat '{prompt_file}')\" --dangerously-skip-permissions",
            binary_path = binary_path,
            prompt_file = prompt_file.display()
        ),
        Harness::OpenCode => format!(
            "'{binary_path}' run \"$(cat '{prompt_file}')\"",
            binary_path = binary_path,
            prompt_file = prompt_file.display()
        ),
    };

    // Create a Ralph loop script that:
    // 1. Runs the harness with the prompt
    // 2. Checks if the task was marked done (via scud show)
    // 3. If not done, loops back and runs the harness again with the same prompt
    // 4. Continues until task is done or max iterations
    // Use full path to harness binary to avoid PATH issues in spawned shells
    // Source shell profile to ensure PATH includes node, etc.
    let ralph_script = format!(
        r#"
# Source shell profile for PATH setup
[ -f /etc/profile ] && . /etc/profile
[ -f ~/.profile ] && . ~/.profile
[ -f ~/.bash_profile ] && . ~/.bash_profile
[ -f ~/.bashrc ] && . ~/.bashrc
[ -f ~/.zshrc ] && . ~/.zshrc 2>/dev/null
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
[ -s "$HOME/.nvm/nvm.sh" ] && . "$HOME/.nvm/nvm.sh"
[ -s "$HOME/.bun/_bun" ] && . "$HOME/.bun/_bun"

export SCUD_TASK_ID='{task_id}'
export RALPH_PROMISE='{promise}'
export RALPH_MAX_ITER=50
export RALPH_ITER=0

echo "🔄 Ralph loop starting for task {task_id}"
echo "   Harness: {harness_name}"
echo "   Completion promise: {promise}"
echo "   Max iterations: $RALPH_MAX_ITER"
echo ""

while true; do
    RALPH_ITER=$((RALPH_ITER + 1))
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "🔄 RALPH ITERATION $RALPH_ITER / $RALPH_MAX_ITER"
    echo "═══════════════════════════════════════════════════════════"
    echo ""

    # Run harness with the prompt (using full path)
    {harness_cmd}

    # Check if task is done
    TASK_STATUS=$(scud show {task_id} 2>/dev/null | grep -i "status:" | awk '{{print $2}}')

    if [ "$TASK_STATUS" = "done" ]; then
        echo ""
        echo "✅ Task {task_id} completed successfully after $RALPH_ITER iterations!"
        rm -f '{prompt_file}'
        break
    fi

    # Check max iterations
    if [ $RALPH_ITER -ge $RALPH_MAX_ITER ]; then
        echo ""
        echo "⚠️  Ralph loop: Max iterations ($RALPH_MAX_ITER) reached for task {task_id}"
        echo "   Task status: $TASK_STATUS"
        rm -f '{prompt_file}'
        break
    fi

    # Small delay before next iteration
    echo ""
    echo "🔄 Task not yet complete (status: $TASK_STATUS). Continuing loop..."
    sleep 2
done
"#,
        task_id = task_id,
        promise = completion_promise,
        prompt_file = prompt_file.display(),
        harness_name = harness.name(),
        harness_cmd = harness_cmd,
    );

    // Write the Ralph script to a temp file
    let script_file = std::env::temp_dir().join(format!("scud-ralph-script-{}.sh", task_id));
    std::fs::write(&script_file, &ralph_script)?;

    // Make it executable and run it
    let cmd = format!(
        "chmod +x '{}' && '{}'",
        script_file.display(),
        script_file.display()
    );

    let target = format!("{}:{}", session_name, window_index);
    let send_result = Command::new("tmux")
        .args(["send-keys", "-t", &target, &cmd, "Enter"])
        .output()
        .context("Failed to send command to tmux window")?;

    if !send_result.status.success() {
        anyhow::bail!(
            "Failed to send keys: {}",
            String::from_utf8_lossy(&send_result.stderr)
        );
    }

    Ok(())
}

/// Check if a tmux session exists
pub fn tmux_session_exists(session_name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Attach to a tmux session
pub fn tmux_attach(session_name: &str) -> Result<()> {
    // Use exec to replace current process with tmux attach
    let status = Command::new("tmux")
        .args(["attach", "-t", session_name])
        .status()
        .context("Failed to attach to tmux session")?;

    if !status.success() {
        anyhow::bail!("tmux attach failed");
    }

    Ok(())
}

/// Setup the control window in a tmux session with monitoring script
pub fn setup_tmux_control_window(session_name: &str, tag: &str) -> Result<()> {
    let control_script = format!(
        r#"watch -n 5 'echo "=== SCUD Spawn Monitor: {} ===" && echo && scud stats --tag {} && echo && scud whois --tag {} && echo && echo "Ready tasks:" && scud next-batch --tag {} --limit 5 2>/dev/null | head -20'"#,
        session_name, tag, tag, tag
    );

    let target = format!("{}:ctrl", session_name);
    Command::new("tmux")
        .args(["send-keys", "-t", &target, &control_script, "Enter"])
        .status()
        .context("Failed to setup control window")?;

    Ok(())
}
