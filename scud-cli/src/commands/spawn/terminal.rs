//! Terminal detection and spawning functionality
//!
//! Supports Kitty, WezTerm, iTerm2, and tmux with auto-detection based on environment variables.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Supported terminal emulators
#[derive(Debug, Clone, PartialEq)]
pub enum Terminal {
    Kitty,
    Wezterm,
    ITerm2,
    Tmux,
}

impl Terminal {
    /// Display name for the terminal
    pub fn name(&self) -> &'static str {
        match self {
            Terminal::Kitty => "kitty",
            Terminal::Wezterm => "wezterm",
            Terminal::ITerm2 => "iterm2",
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

    // Default to tmux as universal fallback
    Terminal::Tmux
}

/// Parse terminal name from string argument
pub fn parse_terminal(name: &str) -> Result<Terminal> {
    match name.to_lowercase().as_str() {
        "kitty" => Ok(Terminal::Kitty),
        "wezterm" => Ok(Terminal::Wezterm),
        "iterm" | "iterm2" => Ok(Terminal::ITerm2),
        "tmux" => Ok(Terminal::Tmux),
        "auto" => Ok(detect_terminal()),
        other => anyhow::bail!("Unknown terminal: {}. Supported: kitty, wezterm, iterm2, tmux, auto", other),
    }
}

/// Check if required terminal binary is available
pub fn check_terminal_available(terminal: &Terminal) -> Result<()> {
    let binary = match terminal {
        Terminal::Kitty => "kitty",
        Terminal::Wezterm => "wezterm",
        Terminal::ITerm2 => "osascript", // iTerm2 uses AppleScript
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
    match terminal {
        Terminal::Kitty => spawn_kitty(task_id, prompt, working_dir),
        Terminal::Wezterm => spawn_wezterm(task_id, prompt, working_dir),
        Terminal::ITerm2 => spawn_iterm2(task_id, prompt, working_dir),
        Terminal::Tmux => spawn_tmux(task_id, prompt, working_dir, session_name),
    }
}

/// Spawn in Kitty terminal using remote control
fn spawn_kitty(task_id: &str, prompt: &str, working_dir: &Path) -> Result<()> {
    let title = format!("task-{}", task_id);

    // Write prompt to temp file to avoid shell escaping issues
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Interactive mode with SCUD_TASK_ID for hook integration
    // The Stop hook will read SCUD_TASK_ID and auto-complete the task
    let bash_cmd = format!(
        r#"export SCUD_TASK_ID='{}' ; claude "$(cat '{}')" --dangerously-skip-permissions ; rm -f '{}' ; exec bash"#,
        task_id,
        prompt_file.display(),
        prompt_file.display()
    );

    let status = Command::new("kitty")
        .args(["@", "launch", "--type=window"])
        .arg(format!("--title={}", title))
        .arg(format!("--cwd={}", working_dir.display()))
        .arg("bash")
        .arg("-c")
        .arg(&bash_cmd)
        .status()
        .context("Failed to spawn Kitty window")?;

    if !status.success() {
        anyhow::bail!("Kitty launch failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Spawn in WezTerm terminal
fn spawn_wezterm(task_id: &str, prompt: &str, working_dir: &Path) -> Result<()> {
    // Write prompt to temp file to avoid shell escaping issues
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    // Interactive mode with SCUD_TASK_ID for hook integration
    let bash_cmd = format!(
        r#"export SCUD_TASK_ID='{}' ; claude "$(cat '{}')" --dangerously-skip-permissions ; rm -f '{}' ; exec bash"#,
        task_id,
        prompt_file.display(),
        prompt_file.display()
    );

    let status = Command::new("wezterm")
        .args(["cli", "spawn", "--new-window"])
        .arg(format!("--cwd={}", working_dir.display()))
        .arg("--")
        .arg("bash")
        .arg("-c")
        .arg(&bash_cmd)
        .status()
        .context("Failed to spawn WezTerm window")?;

    if !status.success() {
        anyhow::bail!("WezTerm spawn failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Spawn in iTerm2 on macOS using AppleScript
fn spawn_iterm2(task_id: &str, prompt: &str, working_dir: &Path) -> Result<()> {
    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, prompt)?;

    let title = format!("task-{}", task_id);
    // Interactive mode with SCUD_TASK_ID for hook integration
    let claude_cmd = format!(
        r#"cd '{}' && export SCUD_TASK_ID='{}' && claude \"$(cat '{}')\" --dangerously-skip-permissions ; rm -f '{}'"#,
        working_dir.display(),
        task_id,
        prompt_file.display(),
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
        claude_cmd.replace('\\', "\\\\").replace('"', "\\\"")
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

/// Spawn in tmux session
fn spawn_tmux(task_id: &str, prompt: &str, working_dir: &Path, session_name: &str) -> Result<()> {
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
            "-t", session_name,
            "-n", &window_name,
            "-P",                    // Print info about new window
            "-F", "#{window_index}", // Format: just the index
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
    let claude_cmd = format!(
        r#"export SCUD_TASK_ID='{}' ; claude "$(cat '{}')" --dangerously-skip-permissions ; rm -f '{}'"#,
        task_id,
        prompt_file.display(),
        prompt_file.display()
    );

    let target = format!("{}:{}", session_name, window_index);
    let send_result = Command::new("tmux")
        .args(["send-keys", "-t", &target, &claude_cmd, "Enter"])
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
    match terminal {
        Terminal::Tmux => spawn_tmux_ralph(task_id, prompt, working_dir, session_name, completion_promise),
        // For other terminals, fall back to regular spawn
        // Ralph loop requires bash scripting that's easier in tmux
        _ => spawn_terminal(terminal, task_id, prompt, working_dir, session_name),
    }
}

/// Spawn in tmux session with Ralph loop wrapper
fn spawn_tmux_ralph(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
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
            "-t", session_name,
            "-n", &window_name,
            "-P",
            "-F", "#{window_index}",
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

    // Create a Ralph loop script that:
    // 1. Runs Claude with the prompt
    // 2. Checks if the task was marked done (via scud show)
    // 3. If not done, loops back and runs Claude again with the same prompt
    // 4. Continues until task is done or max iterations
    let ralph_script = format!(
        r#"
export SCUD_TASK_ID='{task_id}'
export RALPH_PROMISE='{promise}'
export RALPH_MAX_ITER=50
export RALPH_ITER=0

echo "🔄 Ralph loop starting for task {task_id}"
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

    # Run Claude with the prompt
    claude "$(cat '{prompt_file}')" --dangerously-skip-permissions

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
