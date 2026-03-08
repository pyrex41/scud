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
    /// Rho CLI (rho-cli)
    Rho,
    /// Direct Anthropic API (no external CLI needed)
    #[cfg(feature = "direct-api")]
    DirectApi,
}

impl Harness {
    /// Parse harness from string
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Harness::Claude),
            "opencode" | "open-code" | "xai" => Ok(Harness::OpenCode),
            "cursor" | "cursor-agent" => Ok(Harness::Cursor),
            "rho" | "rho-cli" => Ok(Harness::Rho),
            #[cfg(feature = "direct-api")]
            "direct-api" | "direct" | "api" => Ok(Harness::DirectApi),
            other => anyhow::bail!(
                "Unknown harness: '{}'. Supported: claude, opencode, cursor, rho",
                other
            ),
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Cursor => "cursor",
            Harness::Rho => "rho",
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => "direct-api",
        }
    }

    /// Binary name to search for
    pub fn binary_name(&self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::OpenCode => "opencode",
            Harness::Cursor => "agent",
            Harness::Rho => "rho-cli",
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => "scud",
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
            Harness::Rho => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                format!(
                    r#"'{}'{} --prompt-file '{}'"#,
                    binary_path,
                    model_flag,
                    prompt_file.display()
                )
            }
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => {
                let model_flag = model.map(|m| format!(" --model {}", m)).unwrap_or_default();
                format!(
                    r#"'{}' agent-exec --prompt-file '{}'{}"#,
                    binary_path,
                    prompt_file.display(),
                    model_flag
                )
            }
        }
    }
}

/// Normalize a CLI model argument into an optional override.
///
/// For `rho`, the legacy SCUD default (`xai/grok-code-fast-1`) is treated as
/// "no explicit override" so rho can honor project/global rho configuration
/// (e.g. `RHO.md` and `~/.rho/models.toml`).
pub fn normalize_model_override<'a>(harness: Harness, model_arg: &'a str) -> Option<&'a str> {
    let trimmed = model_arg.trim();
    if trimmed.is_empty() {
        return None;
    }
    if matches!(harness, Harness::Rho) && trimmed == "xai/grok-code-fast-1" {
        return None;
    }
    Some(trimmed)
}

/// Cached paths to harness binaries
static CLAUDE_PATH: OnceLock<String> = OnceLock::new();
static OPENCODE_PATH: OnceLock<String> = OnceLock::new();
static CURSOR_PATH: OnceLock<String> = OnceLock::new();
static RHO_PATH: OnceLock<String> = OnceLock::new();
#[cfg(feature = "direct-api")]
static SCUD_PATH: OnceLock<String> = OnceLock::new();

fn harness_env_override(harness: Harness) -> Option<String> {
    match harness {
        Harness::Rho => std::env::var("SCUD_RHO_BIN")
            .ok()
            .or_else(|| std::env::var("RHO_CLI_BIN").ok()),
        _ => None,
    }
}

fn rho_candidate_is_placeholder(binary_path: &str) -> Result<bool> {
    let output = Command::new(binary_path)
        .output()
        .with_context(|| format!("Failed probing rho binary '{}'", binary_path))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr).to_lowercase();

    Ok(
        combined.contains("rho-cli placeholder - cli structure ready")
            || combined.contains("rho-cli-stub is a legacy placeholder"),
    )
}

fn candidate_is_usable(
    harness: Harness,
    path: &str,
    skipped_rho_placeholders: &mut Vec<String>,
) -> Result<bool> {
    if matches!(harness, Harness::Rho) && rho_candidate_is_placeholder(path)? {
        skipped_rho_placeholders.push(path.to_string());
        return Ok(false);
    }
    Ok(true)
}

/// Find the full path to a harness binary.
/// Caches the result for subsequent calls.
pub fn find_harness_binary(harness: Harness) -> Result<&'static str> {
    let cache = match harness {
        Harness::Claude => &CLAUDE_PATH,
        Harness::OpenCode => &OPENCODE_PATH,
        Harness::Cursor => &CURSOR_PATH,
        Harness::Rho => &RHO_PATH,
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => &SCUD_PATH,
    };

    // Check if already cached
    if let Some(path) = cache.get() {
        return Ok(path.as_str());
    }

    let mut skipped_rho_placeholders = Vec::new();

    // Optional explicit override for harness binary path.
    if let Some(override_path) = harness_env_override(harness) {
        let override_path = override_path.trim().to_string();
        if override_path.is_empty() {
            anyhow::bail!("Harness override path is empty");
        }
        if !std::path::Path::new(&override_path).exists() {
            anyhow::bail!("Harness override path does not exist: {}", override_path);
        }
        if candidate_is_usable(harness, &override_path, &mut skipped_rho_placeholders)? {
            let _ = cache.set(override_path);
            return Ok(cache.get().unwrap().as_str());
        }
        anyhow::bail!(
            "Harness override points to a placeholder rho binary: {}\nUse a functional rho-cli binary (for example: ~/projects/rho/target/release/rho-cli).",
            override_path
        );
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
            if candidate_is_usable(harness, &path, &mut skipped_rho_placeholders)? {
                // Cache and return
                let _ = cache.set(path);
                return Ok(cache.get().unwrap().as_str());
            }
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
        Harness::Rho => &[
            "/opt/homebrew/bin/rho-cli",
            "/usr/local/bin/rho-cli",
            "/usr/bin/rho-cli",
        ],
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => &[
            "/opt/homebrew/bin/scud",
            "/usr/local/bin/scud",
            "/usr/bin/scud",
        ],
    };

    for path in common_paths {
        if std::path::Path::new(path).exists()
            && candidate_is_usable(harness, path, &mut skipped_rho_placeholders)?
        {
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
            Harness::Cursor => vec![format!("{}/.local/bin/agent", home)],
            Harness::Rho => vec![
                format!("{}/.cargo/bin/rho-cli", home),
                format!("{}/.local/bin/rho-cli", home),
                format!("{}/projects/rho/target/release/rho-cli", home),
                format!("{}/projects/rho/target/debug/rho-cli", home),
            ],
            #[cfg(feature = "direct-api")]
            Harness::DirectApi => vec![
                format!("{}/.local/bin/scud", home),
                format!("{}/.cargo/bin/scud", home),
            ],
        };

        for path in home_paths {
            if std::path::Path::new(&path).exists()
                && candidate_is_usable(harness, &path, &mut skipped_rho_placeholders)?
            {
                let _ = cache.set(path);
                return Ok(cache.get().unwrap().as_str());
            }
        }
    }

    if matches!(harness, Harness::Rho) && !skipped_rho_placeholders.is_empty() {
        let listed = skipped_rho_placeholders.join(", ");
        anyhow::bail!(
            "Detected placeholder rho binaries: {}.\nUse a functional rho-cli binary (set SCUD_RHO_BIN or install/build rho-agent).",
            listed
        );
    }

    let install_hint = match harness {
        Harness::Claude => "Install with: npm install -g @anthropic-ai/claude-code",
        Harness::OpenCode => "Install with: curl -fsSL https://opencode.ai/install | bash",
        Harness::Cursor => "Install with: curl https://cursor.com/install -fsSL | bash",
        Harness::Rho => "Install/build a functional rho-cli (set SCUD_RHO_BIN=/absolute/path/to/rho-cli if needed)",
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => "Install with: cargo install scud-cli --features direct-api",
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

/// Configuration for spawning an agent in a tmux window.
///
/// Consolidates all spawn parameters into a single struct, replacing the
/// previous 7 public spawn functions with one entry point.
pub struct SpawnConfig<'a> {
    pub task_id: &'a str,
    pub prompt: &'a str,
    pub working_dir: &'a Path,
    pub session_name: &'a str,
    pub harness: Harness,
    pub model: Option<&'a str>,
    pub task_list_id: Option<&'a str>,
}

impl<'a> SpawnConfig<'a> {
    /// Create a minimal spawn config with defaults (Claude harness, no model override).
    pub fn new(
        task_id: &'a str,
        prompt: &'a str,
        working_dir: &'a Path,
        session_name: &'a str,
    ) -> Self {
        Self {
            task_id,
            prompt,
            working_dir,
            session_name,
            harness: Harness::Claude,
            model: None,
            task_list_id: None,
        }
    }
}

/// Spawn an AI agent in a tmux window.
/// Returns the tmux window index for easy attachment (e.g., "3" for session:3).
pub fn spawn_tmux_agent(config: &SpawnConfig) -> Result<String> {
    let binary_path = find_harness_binary(config.harness)?;

    let window_name = format!("task-{}", config.task_id);

    ensure_tmux_session(config.session_name, config.working_dir)?;

    let window_index = create_tmux_window(config.session_name, &window_name, config.working_dir)?;

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-prompt-{}.txt", config.task_id));
    std::fs::write(&prompt_file, config.prompt)?;

    let harness_cmd = config.harness.command(binary_path, &prompt_file, config.model);

    // Build the task list ID export line if provided
    let task_list_export = config
        .task_list_id
        .map(|id| format!("export CLAUDE_CODE_TASK_LIST_ID='{}'\n", id))
        .unwrap_or_default();

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
        task_id = config.task_id,
        task_list_export = task_list_export,
        harness_cmd = harness_cmd,
        prompt_file = prompt_file.display()
    );

    let script_file = std::env::temp_dir().join(format!("scud-spawn-{}.sh", config.task_id));
    std::fs::write(&script_file, &spawn_script)?;

    send_tmux_command(config.session_name, &window_index, &format!("bash '{}'", script_file.display()))?;

    Ok(window_index)
}

/// Spawn an AI agent in a tmux window with a Ralph retry loop.
/// The agent re-runs until the task is marked done or max iterations reached.
pub fn spawn_ralph_agent(config: &SpawnConfig, completion_promise: &str) -> Result<()> {
    let binary_path = find_harness_binary(config.harness)?;

    let window_name = format!("ralph-{}", config.task_id);

    ensure_tmux_session(config.session_name, config.working_dir)?;

    let window_index = create_tmux_window(config.session_name, &window_name, config.working_dir)?;

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("scud-ralph-{}.txt", config.task_id));
    std::fs::write(&prompt_file, config.prompt)?;

    // Build the harness-specific command for the ralph script
    // We need to inline this since the script is a bash heredoc
    let harness_cmd = match config.harness {
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
        Harness::Rho => format!(
            "'{binary_path}' --prompt-file '{prompt_file}'",
            binary_path = binary_path,
            prompt_file = prompt_file.display()
        ),
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => format!(
            "'{binary_path}' agent-exec --prompt-file '{prompt_file}'",
            binary_path = binary_path,
            prompt_file = prompt_file.display()
        ),
    };

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
        task_id = config.task_id,
        promise = completion_promise,
        prompt_file = prompt_file.display(),
        harness_name = config.harness.name(),
        harness_cmd = harness_cmd,
    );

    let script_file = std::env::temp_dir().join(format!("scud-ralph-script-{}.sh", config.task_id));
    std::fs::write(&script_file, &ralph_script)?;

    send_tmux_command(config.session_name, &window_index, &format!("bash '{}'", script_file.display()))?;

    Ok(())
}

// ============================================================================
// Tmux helpers (shared by spawn_tmux_agent and spawn_ralph_agent)
// ============================================================================

/// Ensure a tmux session exists, creating it if needed.
fn ensure_tmux_session(session_name: &str, working_dir: &Path) -> Result<()> {
    let exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !exists {
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-n", "ctrl"])
            .arg("-c")
            .arg(working_dir)
            .status()
            .context("Failed to create tmux session")?;
    }

    Ok(())
}

/// Create a new tmux window and return its index.
fn create_tmux_window(session_name: &str, window_name: &str, working_dir: &Path) -> Result<String> {
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            session_name,
            "-n",
            window_name,
            "-P",
            "-F",
            "#{window_index}",
        ])
        .arg("-c")
        .arg(working_dir)
        .output()
        .context("Failed to create tmux window")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to create window: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Send a command to a tmux window by index.
fn send_tmux_command(session_name: &str, window_index: &str, cmd: &str) -> Result<()> {
    let target = format!("{}:{}", session_name, window_index);
    let result = Command::new("tmux")
        .args(["send-keys", "-t", &target, cmd, "Enter"])
        .output()
        .context("Failed to send command to tmux window")?;

    if !result.status.success() {
        anyhow::bail!(
            "Failed to send keys: {}",
            String::from_utf8_lossy(&result.stderr)
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
    let prompt_patterns = [
        "$ ", // bash default
        "% ", // zsh default
        "> ", // fish, some custom prompts
        "# ", // root shell
        "❯ ", // starship, some modern prompts
        "→ ", // some custom prompts
    ];

    for pattern in prompt_patterns {
        if last_line.ends_with(pattern) || last_line.ends_with(pattern.trim()) {
            return true;
        }
    }

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

/// Spawn a command in a tmux window (simpler than spawn_tmux_agent — just runs a raw command)
pub fn spawn_in_tmux(
    session_name: &str,
    window_name: &str,
    command: &str,
    working_dir: &Path,
) -> Result<()> {
    ensure_tmux_session(session_name, working_dir)?;

    let window_index = create_tmux_window(session_name, window_name, working_dir)?;

    send_tmux_command(session_name, &window_index, command)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn make_executable_script(contents: &str) -> tempfile::TempPath {
        let file = tempfile::NamedTempFile::new().expect("create temp file");
        fs::write(file.path(), contents).expect("write temp script");
        let mut perms = fs::metadata(file.path()).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(file.path(), perms).expect("set executable permission");
        file.into_temp_path()
    }

    #[test]
    fn normalize_model_override_treats_rho_legacy_default_as_none() {
        let model = normalize_model_override(Harness::Rho, "xai/grok-code-fast-1");
        assert!(model.is_none());
    }

    #[test]
    fn normalize_model_override_keeps_non_empty_non_legacy_values() {
        let model = normalize_model_override(Harness::Claude, "sonnet");
        assert_eq!(model, Some("sonnet"));
    }

    #[test]
    fn rho_placeholder_probe_detects_placeholder_output() {
        let script = make_executable_script(
            "#!/usr/bin/env bash\necho 'rho-cli placeholder - CLI structure ready'\n",
        );
        let is_placeholder =
            rho_candidate_is_placeholder(script.to_str().expect("temp path utf8")).unwrap();
        assert!(is_placeholder);
    }

    #[test]
    fn rho_placeholder_probe_accepts_non_placeholder_binary() {
        let script =
            make_executable_script("#!/usr/bin/env bash\necho 'functional rho cli'\nexit 1\n");
        let is_placeholder =
            rho_candidate_is_placeholder(script.to_str().expect("temp path utf8")).unwrap();
        assert!(!is_placeholder);
    }
}
