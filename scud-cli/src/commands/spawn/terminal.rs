//! Terminal spawning functionality
//!
//! Spawns AI coding agents in tmux sessions for parallel task execution.
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
    /// Cursor Agent CLI
    Cursor,
}

impl Harness {
    /// Parse harness from string
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Harness::Claude),
            "opencode" | "open-code" => Ok(Harness::OpenCode),
            "cursor" | "cursor-agent" => Ok(Harness::Cursor),
            other => anyhow::bail!("Unknown harness: '{}'. Supported: claude, opencode, cursor", other),
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Cursor => "cursor",
        }
    }

    /// Binary name to search for
    pub fn binary_name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Cursor => "agent",
        }
    }

    /// Generate the command to run with a prompt and optional model
    pub fn command(&self, binary_path: &str, prompt_file: &Path, model: Option<&str>) -> String {
        match self {
            Harness::Claude => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                format!(
                    r#"'{}' "$(cat '{}')" --dangerously-skip-permissions{}"#,
                    binary_path,
                    prompt_file.display(),
                    model_flag
                )
            }
            Harness::OpenCode => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                // Use --variant minimal to reduce reasoning overhead and avoid
                // "reasoning part not found" errors with some models
                format!(
                    r#"'{}'{} run --variant minimal "$(cat '{}')""#,
                    binary_path,
                    model_flag,
                    prompt_file.display()
                )
            }
            Harness::Cursor => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                format!(
                    r#"'{}' -p{} "$(cat '{}')""#,
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
static CURSOR_PATH: OnceLock<String> = OnceLock::new();

/// Find the full path to a harness binary.
/// Caches the result for subsequent calls.
pub fn find_harness_binary(harness: Harness) -> Result<&'static str> {
    let cache = match harness {
        Harness::Claude => &CLAUDE_PATH,
        Harness::OpenCode => &OPENCODE_PATH,
        Harness::Cursor => &CURSOR_PATH,
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
        Harness::Cursor => &[
            "/opt/homebrew/bin/agent",
            "/usr/local/bin/agent",
            "/usr/bin/agent",
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
            Harness::Cursor => vec![
                format!("{}/.local/bin/agent", home),
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
        Harness::Cursor => "Install with: curl https://cursor.com/install -fsSL | bash",
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

/// Check if tmux is available
pub fn check_tmux_available() -> Result<()> {
    let result = Command::new("which")
        .arg("tmux")
        .output()
        .context("Failed to check for tmux binary")?;

    if !result.status.success() {
        anyhow::bail!("tmux is not installed or not in PATH. Install with: brew install tmux (macOS) or apt install tmux (Linux)");
    }

    Ok(())
}

/// Spawn a new tmux window with the given command
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3)
pub fn spawn_terminal(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
) -> Result<String> {
    // Default to Claude harness for backwards compatibility
    spawn_terminal_with_harness_and_model(
        task_id,
        prompt,
        working_dir,
        session_name,
        Harness::Claude,
        None,
    )
}

/// Spawn a new tmux window with the given command using a specific harness
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3)
pub fn spawn_terminal_with_harness(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
) -> Result<String> {
    spawn_terminal_with_harness_and_model(task_id, prompt, working_dir, session_name, harness, None)
}

/// Spawn a new tmux window with the given command using a specific harness and model
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3)
pub fn spawn_terminal_with_harness_and_model(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
    model: Option<&str>,
) -> Result<String> {
    // Find harness binary path upfront to fail fast if not found
    let binary_path = find_harness_binary(harness)?;
    spawn_tmux(
        task_id,
        prompt,
        working_dir,
        session_name,
        binary_path,
        harness,
        model,
        None, // No task list ID (legacy mode)
    )
}

/// Spawn a new tmux window with Claude Code task list integration
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3)
///
/// This variant sets `CLAUDE_CODE_TASK_LIST_ID` so agents can see SCUD tasks
/// via the native `TaskList` tool.
pub fn spawn_terminal_with_task_list(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    harness: Harness,
    model: Option<&str>,
    task_list_id: &str,
) -> Result<String> {
    let binary_path = find_harness_binary(harness)?;
    spawn_tmux(
        task_id,
        prompt,
        working_dir,
        session_name,
        binary_path,
        harness,
        model,
        Some(task_list_id),
    )
}

/// Spawn in tmux session
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3)
#[allow(clippy::too_many_arguments)]
fn spawn_tmux(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    binary_path: &str,
    harness: Harness,
    model: Option<&str>,
    task_list_id: Option<&str>,
) -> Result<String> {
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

    // Build the task list ID export line if provided
    let task_list_export = task_list_id
        .map(|id| format!("export CLAUDE_CODE_TASK_LIST_ID='{}'\n", id))
        .unwrap_or_default();

    // Write a bash script to handle shell-agnostic execution
    // This ensures it works even if the user's shell is fish, zsh, etc.
    let spawn_script = format!(
        r#"#!/usr/bin/env bash
# Source shell profile for PATH setup
source ~/.bash_profile 2>/dev/null
source ~/.zshrc 2>/dev/null
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
[ -s "$HOME/.nvm/nvm.sh" ] && source "$HOME/.nvm/nvm.sh"

export SCUD_TASK_ID='{task_id}'
{task_list_export}{harness_cmd}
rm -f '{prompt_file}'
"#,
        task_id = task_id,
        task_list_export = task_list_export,
        harness_cmd = harness_cmd,
        prompt_file = prompt_file.display()
    );

    let script_file = std::env::temp_dir().join(format!("scud-spawn-{}.sh", task_id));
    std::fs::write(&script_file, &spawn_script)?;

    // Run the script with bash explicitly (works in any shell including fish)
    let run_cmd = format!("bash '{}'", script_file.display());

    let target = format!("{}:{}", session_name, window_index);
    let send_result = Command::new("tmux")
        .args(["send-keys", "-t", &target, &run_cmd, "Enter"])
        .output()
        .context("Failed to send command to tmux window")?;

    if !send_result.status.success() {
        anyhow::bail!(
            "Failed to send keys: {}",
            String::from_utf8_lossy(&send_result.stderr)
        );
    }

    Ok(window_index)
}

/// Spawn a new tmux window with Ralph loop enabled
/// The agent will keep running until the completion promise is detected
pub fn spawn_terminal_ralph(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
) -> Result<()> {
    // Default to Claude harness
    spawn_terminal_ralph_with_harness(
        task_id,
        prompt,
        working_dir,
        session_name,
        completion_promise,
        Harness::Claude,
    )
}

/// Spawn a new tmux window with Ralph loop enabled using a specific harness
pub fn spawn_terminal_ralph_with_harness(
    task_id: &str,
    prompt: &str,
    working_dir: &Path,
    session_name: &str,
    completion_promise: &str,
    harness: Harness,
) -> Result<()> {
    // Find harness binary path upfront to fail fast if not found
    let binary_path = find_harness_binary(harness)?;
    spawn_tmux_ralph(
        task_id,
        prompt,
        working_dir,
        session_name,
        completion_promise,
        binary_path,
        harness,
    )
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
            "'{binary_path}' run --variant minimal \"$(cat '{prompt_file}')\"",
            binary_path = binary_path,
            prompt_file = prompt_file.display()
        ),
        Harness::Cursor => format!(
            "'{binary_path}' -p \"$(cat '{prompt_file}')\"",
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
        r#"#!/usr/bin/env bash
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

    // Run it with bash explicitly (works in any shell including fish)
    let cmd = format!("bash '{}'", script_file.display());

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

/// Check if a specific window exists in a tmux session
pub fn tmux_window_exists(session_name: &str, window_name: &str) -> bool {
    let output = Command::new("tmux")
        .args(["list-windows", "-t", session_name, "-F", "#{window_name}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let windows = String::from_utf8_lossy(&out.stdout);
            windows
                .lines()
                .any(|w| w == window_name || w.starts_with(&format!("{}-", window_name)))
        }
        _ => false,
    }
}

/// Check if a tmux pane shows a shell prompt (indicating process completed/crashed)
pub fn tmux_pane_shows_prompt(session_name: &str, window_name: &str) -> bool {
    let window_target = format!("{}:{}", session_name, window_name);

    // Capture last line of pane content
    let output = std::process::Command::new("tmux")
        .args(["capture-pane", "-t", &window_target, "-p", "-S", "-1"])
        .output();

    let Ok(output) = output else {
        return false;
    };

    if !output.status.success() {
        return false;
    }

    let last_line = String::from_utf8_lossy(&output.stdout);
    let last_line = last_line.trim();

    // Common shell prompt patterns
    // These indicate the agent process has exited and we're back at shell
    let prompt_patterns = [
        "$ ", // bash default
        "% ", // zsh default
        "> ", // fish, some custom prompts
        "# ", // root shell
        "❯ ", // starship, some modern prompts
        "→ ", // some custom prompts
    ];

    // Check if line ends with a prompt pattern
    for pattern in prompt_patterns {
        if last_line.ends_with(pattern) || last_line.ends_with(pattern.trim()) {
            return true;
        }
    }

    // Also check for common prompt formats: user@host, (env), etc followed by prompt
    if last_line.contains('@')
        && (last_line.ends_with('$') || last_line.ends_with('%') || last_line.ends_with('>'))
    {
        return true;
    }

    false
}

/// Kill a specific tmux window
pub fn kill_tmux_window(session_name: &str, window_name: &str) -> Result<()> {
    let target = format!("{}:{}", session_name, window_name);
    Command::new("tmux")
        .args(["kill-window", "-t", &target])
        .output()?;
    Ok(())
}

/// Spawn a command in a tmux window (simpler than spawn_tmux which does more setup)
pub fn spawn_in_tmux(
    session_name: &str,
    window_name: &str,
    command: &str,
    working_dir: &Path,
) -> Result<()> {
    // Check if session exists, create if not
    let session_exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !session_exists {
        // Create new session with a control window
        Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                session_name,
                "-n",
                "ctrl",
                "-c",
                &working_dir.to_string_lossy(),
            ])
            .output()
            .context("Failed to create tmux session")?;
    }

    // Create new window for this task
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            session_name,
            "-n",
            window_name,
            "-c",
            &working_dir.to_string_lossy(),
            "-P",
            "-F",
            "#{window_index}",
        ])
        .output()
        .context("Failed to create tmux window")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to create tmux window: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let window_index = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Send the command to the window
    let send_result = Command::new("tmux")
        .args([
            "send-keys",
            "-t",
            &format!("{}:{}", session_name, window_index),
            command,
            "Enter",
        ])
        .output()
        .context("Failed to send command to tmux window")?;

    if !send_result.status.success() {
        anyhow::bail!(
            "Failed to send command: {}",
            String::from_utf8_lossy(&send_result.stderr)
        );
    }

    Ok(())
}
