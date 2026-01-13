//! Backpressure validation for maintaining code quality during automated execution.
//!
//! # Overview
//!
//! Backpressure is a quality gate mechanism that runs programmatic validation
//! after each wave of task execution. It prevents bad code from accumulating by
//! catching issues early - if validation fails, the affected tasks are marked as
//! `Failed` so they can be re-attempted or debugged.
//!
//! ## What Backpressure Validates
//!
//! - **Build/compile checks** - Ensures code compiles successfully
//! - **Linting** - Catches style issues and common mistakes
//! - **Type checking** - Validates type correctness (for typed languages)
//! - **Tests** - Runs the test suite to catch regressions
//!
//! ## Workflow Integration
//!
//! In swarm mode, backpressure runs after each wave completes:
//!
//! ```text
//! Wave 1: [Task A, Task B] -> Backpressure Check -> Pass? -> Wave 2
//!                                    |
//!                                    v
//!                             Fail? -> Mark tasks as Failed
//! ```
//!
//! This creates a feedback loop where AI agents can see which tasks caused
//! validation failures and attempt repairs.
//!
//! # Configuration
//!
//! Backpressure commands are configured in `.scud/config.toml`:
//!
//! ```toml
//! [swarm.backpressure]
//! commands = ["cargo build", "cargo test", "cargo clippy -- -D warnings"]
//! stop_on_failure = true  # Stop at first failure (default: true)
//! timeout_secs = 300      # Per-command timeout (default: 300 = 5 minutes)
//! ```
//!
//! If no configuration is found, backpressure auto-detects commands based on
//! project type (Rust, Node.js, Python, Go).
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use scud::backpressure::{BackpressureConfig, run_validation};
//!
//! // Load configuration (auto-detects if not configured)
//! let config = BackpressureConfig::load(None).expect("Failed to load config");
//!
//! // Or create a custom configuration
//! let custom_config = BackpressureConfig {
//!     commands: vec![
//!         "cargo build".to_string(),
//!         "cargo test".to_string(),
//!     ],
//!     stop_on_failure: true,
//!     timeout_secs: 300,
//! };
//!
//! // Run validation in a working directory
//! let working_dir = Path::new(".");
//! let result = run_validation(working_dir, &custom_config).expect("Validation failed");
//!
//! if result.all_passed {
//!     println!("All checks passed!");
//! } else {
//!     println!("Failures: {:?}", result.failures);
//!     for cmd_result in &result.results {
//!         if !cmd_result.passed {
//!             println!("  {} failed with code {:?}", cmd_result.command, cmd_result.exit_code);
//!             println!("  stderr: {}", cmd_result.stderr);
//!         }
//!     }
//! }
//! ```
//!
//! # Auto-Detection
//!
//! When no explicit configuration exists, backpressure detects project type:
//!
//! | Project Type | Detected By | Default Commands |
//! |--------------|-------------|------------------|
//! | Rust | `Cargo.toml` | `cargo build`, `cargo test` |
//! | Node.js | `package.json` | Scripts: `build`, `test`, `lint`, `typecheck` |
//! | Python | `pyproject.toml` or `setup.py` | `pytest` |
//! | Go | `go.mod` | `go build ./...`, `go test ./...` |

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Backpressure configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackpressureConfig {
    /// Commands to run for validation (in order)
    pub commands: Vec<String>,
    /// Whether to stop on first failure
    #[serde(default = "default_stop_on_failure")]
    pub stop_on_failure: bool,
    /// Timeout per command in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_stop_on_failure() -> bool {
    true
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}

impl BackpressureConfig {
    /// Load backpressure config from project
    pub fn load(project_root: Option<&PathBuf>) -> Result<Self> {
        let root = project_root
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let config_path = root.join(".scud").join("config.toml");

        if !config_path.exists() {
            // Try to auto-detect based on project type
            return Ok(Self::auto_detect(&root));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: toml::Value = toml::from_str(&content)?;

        // Look for [swarm.backpressure] section
        if let Some(swarm) = config.get("swarm") {
            if let Some(bp) = swarm.get("backpressure") {
                let bp_config: BackpressureConfig = bp.clone().try_into()?;
                return Ok(bp_config);
            }
        }

        // Fallback to auto-detection
        Ok(Self::auto_detect(&root))
    }

    /// Auto-detect backpressure commands based on project type
    fn auto_detect(root: &Path) -> Self {
        let mut commands = Vec::new();

        // Rust project
        if root.join("Cargo.toml").exists() {
            commands.push("cargo build".to_string());
            commands.push("cargo test".to_string());
        }

        // Node.js project
        if root.join("package.json").exists() {
            // Check for common scripts
            if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
                if content.contains("\"build\"") {
                    commands.push("npm run build".to_string());
                }
                if content.contains("\"test\"") {
                    commands.push("npm test".to_string());
                }
                if content.contains("\"lint\"") {
                    commands.push("npm run lint".to_string());
                }
                if content.contains("\"typecheck\"") {
                    commands.push("npm run typecheck".to_string());
                }
            }
        }

        // Python project
        if (root.join("pyproject.toml").exists() || root.join("setup.py").exists())
            && (root.join("pytest.ini").exists() || root.join("pyproject.toml").exists())
        {
            commands.push("pytest".to_string());
        }

        // Go project
        if root.join("go.mod").exists() {
            commands.push("go build ./...".to_string());
            commands.push("go test ./...".to_string());
        }

        Self {
            commands,
            stop_on_failure: true,
            timeout_secs: 300,
        }
    }
}

/// Result of running validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether all checks passed
    pub all_passed: bool,
    /// List of failures (command names that failed)
    pub failures: Vec<String>,
    /// Detailed results per command
    pub results: Vec<CommandResult>,
}

/// Result of a single command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Command that was run
    pub command: String,
    /// Whether it passed
    pub passed: bool,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Stdout (truncated)
    pub stdout: String,
    /// Stderr (truncated)
    pub stderr: String,
    /// Duration in seconds
    pub duration_secs: f64,
}

/// Run backpressure validation
pub fn run_validation(working_dir: &Path, config: &BackpressureConfig) -> Result<ValidationResult> {
    let mut results = Vec::new();
    let mut failures = Vec::new();
    let mut all_passed = true;

    for cmd_str in &config.commands {
        println!("      Running: {}", cmd_str);

        let start = std::time::Instant::now();
        let result = run_command(working_dir, cmd_str, config.timeout_secs);
        let duration = start.elapsed().as_secs_f64();

        match result {
            Ok((exit_code, stdout, stderr)) => {
                let passed = exit_code == 0;
                if !passed {
                    all_passed = false;
                    failures.push(cmd_str.clone());
                }

                results.push(CommandResult {
                    command: cmd_str.clone(),
                    passed,
                    exit_code: Some(exit_code),
                    stdout: truncate_output(&stdout, 1000),
                    stderr: truncate_output(&stderr, 1000),
                    duration_secs: duration,
                });

                if !passed && config.stop_on_failure {
                    break;
                }
            }
            Err(e) => {
                all_passed = false;
                failures.push(format!("{} (error: {})", cmd_str, e));

                results.push(CommandResult {
                    command: cmd_str.clone(),
                    passed: false,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    duration_secs: duration,
                });

                if config.stop_on_failure {
                    break;
                }
            }
        }
    }

    Ok(ValidationResult {
        all_passed,
        failures,
        results,
    })
}

/// Run a single command using sh -c for proper shell execution with timeout
fn run_command(
    working_dir: &Path,
    cmd_str: &str,
    timeout_secs: u64,
) -> Result<(i32, String, String)> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    if cmd_str.trim().is_empty() {
        anyhow::bail!("Empty command");
    }

    // Use sh -c to properly handle complex commands with pipes, redirections, etc.
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();
    let poll_interval = Duration::from_millis(100);

    // Poll for completion with timeout
    loop {
        match child.try_wait()? {
            Some(status) => {
                // Process completed - read output
                let mut stdout = String::new();
                let mut stderr = String::new();

                if let Some(mut stdout_pipe) = child.stdout.take() {
                    let _ = stdout_pipe.read_to_string(&mut stdout);
                }
                if let Some(mut stderr_pipe) = child.stderr.take() {
                    let _ = stderr_pipe.read_to_string(&mut stderr);
                }

                let exit_code = status.code().unwrap_or(-1);
                return Ok((exit_code, stdout, stderr));
            }
            None => {
                // Process still running - check timeout
                if start.elapsed() > timeout {
                    // Kill the process
                    let _ = child.kill();
                    let _ = child.wait(); // Reap the zombie
                    anyhow::bail!(
                        "Command timed out after {} seconds: {}",
                        timeout_secs,
                        cmd_str
                    );
                }
                std::thread::sleep(poll_interval);
            }
        }
    }
}

/// Truncate output to max length
fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        format!("{}...[truncated]", &output[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_auto_detect_rust() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let config = BackpressureConfig::auto_detect(tmp.path());
        assert!(config.commands.contains(&"cargo build".to_string()));
        assert!(config.commands.contains(&"cargo test".to_string()));
    }

    #[test]
    fn test_auto_detect_empty() {
        let tmp = TempDir::new().unwrap();
        let config = BackpressureConfig::auto_detect(tmp.path());
        assert!(config.commands.is_empty());
    }

    #[test]
    fn test_truncate_output() {
        assert_eq!(truncate_output("short", 100), "short");

        let long = "a".repeat(200);
        let truncated = truncate_output(&long, 50);
        assert!(truncated.contains("truncated"));
        assert!(truncated.len() < 200);
    }

    #[test]
    fn test_run_command_simple() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "echo hello", 60);
        assert!(result.is_ok());
        let (exit_code, stdout, _stderr) = result.unwrap();
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_run_command_with_quotes() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "echo 'hello world'", 60);
        assert!(result.is_ok());
        let (exit_code, stdout, _stderr) = result.unwrap();
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("hello world"));
    }

    #[test]
    fn test_run_command_with_pipe() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "echo hello | cat", 60);
        assert!(result.is_ok());
        let (exit_code, stdout, _stderr) = result.unwrap();
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_run_command_empty() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "", 60);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_command_whitespace_only() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "   ", 60);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_command_timeout() {
        let tmp = TempDir::new().unwrap();
        let result = run_command(tmp.path(), "sleep 5", 1);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("timed out"));
    }
}
