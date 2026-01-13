//! Backpressure configuration and execution
//!
//! Backpressure is the programmatic validation that prevents bad code from
//! being committed. This includes:
//! - Build/compile checks
//! - Linting
//! - Type checking
//! - Tests
//!
//! Configuration is stored in `.scud/config.toml`:
//! ```toml
//! [swarm.backpressure]
//! commands = ["cargo build", "cargo test", "cargo clippy"]
//! ```

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
        if root.join("pyproject.toml").exists() || root.join("setup.py").exists() {
            if root.join("pytest.ini").exists() || root.join("pyproject.toml").exists() {
                commands.push("pytest".to_string());
            }
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

/// Run a single command
fn run_command(
    working_dir: &Path,
    cmd_str: &str,
    _timeout_secs: u64,
) -> Result<(i32, String, String)> {
    // Parse command (simple split on spaces, handles basic cases)
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty command");
    }

    let output = Command::new(parts[0])
        .args(&parts[1..])
        .current_dir(working_dir)
        .output()?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((exit_code, stdout, stderr))
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
}
