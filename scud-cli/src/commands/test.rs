//! Test command - Run tests and spawn repair agents until they pass
//!
//! This command runs a test/validation command and if it fails, spawns an agent
//! to fix the issues. It loops until the tests pass or max attempts is reached.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::agents::AgentDef;
use crate::backpressure::{self, BackpressureConfig, ValidationResult};
use crate::commands::spawn::terminal::{self, Harness};
use crate::storage::Storage;

/// Run the test command
#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    command: Option<&str>,
    max_attempts: usize,
    harness_arg: &str,
    agent_type: &str,
    session_name: Option<String>,
    attach: bool,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available
    terminal::check_tmux_available()?;

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Load backpressure config or use provided command
    let bp_config = if let Some(cmd) = command {
        BackpressureConfig {
            commands: vec![cmd.to_string()],
            stop_on_failure: true,
            timeout_secs: 300,
        }
    } else {
        let config = BackpressureConfig::load(project_root.as_ref())?;
        if config.commands.is_empty() {
            anyhow::bail!(
                "No test command provided and no backpressure commands configured.\n\
                 Use: scud test --command 'your test command'\n\
                 Or configure: scud config backpressure 'cargo test'"
            );
        }
        config
    };

    // Parse harness
    let harness = Harness::parse(harness_arg)?;
    terminal::find_harness_binary(harness)?;

    // Generate session name
    let session_name = session_name.unwrap_or_else(|| "scud-test".to_string());

    // Display header
    println!("{}", "SCUD Test & Fix".cyan().bold());
    println!("{}", "═".repeat(50));
    println!(
        "{:<20} {}",
        "Commands:".dimmed(),
        bp_config.commands.join(", ").cyan()
    );
    println!(
        "{:<20} {}",
        "Max attempts:".dimmed(),
        max_attempts.to_string().cyan()
    );
    println!(
        "{:<20} {}",
        "Repair agent:".dimmed(),
        agent_type.cyan()
    );
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().cyan());
    println!();

    // Main loop
    for attempt in 1..=max_attempts {
        println!(
            "{} {}/{}",
            "Attempt".blue().bold(),
            attempt,
            max_attempts
        );
        println!("{}", "-".repeat(40).blue());

        // Run validation
        println!("  {} Running tests...", "→".dimmed());
        let result = backpressure::run_validation(&working_dir, &bp_config)?;

        if result.all_passed {
            println!();
            println!(
                "{} All tests passed on attempt {}!",
                "✓".green().bold(),
                attempt
            );
            return Ok(());
        }

        // Tests failed - show errors
        println!();
        println!("  {} Tests failed:", "✗".red());
        for failure in &result.failures {
            println!("    - {}", failure.red());
        }

        // Get error output for the agent
        let error_output = format_error_output(&result);

        if attempt == max_attempts {
            println!();
            println!(
                "{} Max attempts ({}) reached. Tests still failing.",
                "!".red().bold(),
                max_attempts
            );
            return Err(anyhow::anyhow!("Tests failed after {} attempts", max_attempts));
        }

        // Spawn repair agent
        println!();
        println!(
            "  {} Spawning {} agent to fix...",
            "→".dimmed(),
            agent_type
        );

        let repair_marker = working_dir
            .join(".scud")
            .join(format!("test-repair-complete-{}", attempt));

        // Clean up any existing marker
        let _ = std::fs::remove_file(&repair_marker);

        spawn_repair_agent(
            &working_dir,
            &session_name,
            attempt,
            harness,
            agent_type,
            &bp_config.commands,
            &error_output,
            &repair_marker,
        )?;

        // Wait for repair to complete
        println!("  {} Waiting for repair agent...", "→".dimmed());
        wait_for_repair(&repair_marker, attach, &session_name)?;

        println!();
    }

    Ok(())
}

/// Format error output from validation result for the repair agent
fn format_error_output(result: &ValidationResult) -> String {
    let mut output = String::new();

    for cmd_result in &result.results {
        if !cmd_result.passed {
            output.push_str(&format!("Command: {}\n", cmd_result.command));
            output.push_str(&format!("Exit code: {:?}\n", cmd_result.exit_code));
            if !cmd_result.stdout.is_empty() {
                output.push_str(&format!("Stdout:\n{}\n", cmd_result.stdout));
            }
            if !cmd_result.stderr.is_empty() {
                output.push_str(&format!("Stderr:\n{}\n", cmd_result.stderr));
            }
            output.push('\n');
        }
    }

    output
}

/// Spawn a repair agent
#[allow(clippy::too_many_arguments)]
fn spawn_repair_agent(
    working_dir: &std::path::Path,
    session_name: &str,
    attempt: usize,
    default_harness: Harness,
    agent_type: &str,
    commands: &[String],
    error_output: &str,
    repair_marker: &std::path::Path,
) -> Result<()> {
    // Try to load agent definition
    let (harness, model) = match AgentDef::try_load(agent_type, working_dir) {
        Some(agent_def) => {
            let h = agent_def.harness().unwrap_or(default_harness);
            let m = agent_def.model().map(String::from);
            (h, m)
        }
        None => {
            println!(
                "    {} Agent '{}' not found, using defaults",
                "!".yellow(),
                agent_type
            );
            (default_harness, None)
        }
    };

    let commands_str = commands.join(" && ");
    let marker_path = repair_marker.display();

    let prompt = format!(
        r#"You are a repair agent fixing test/build failures.

## Failed Command(s)
{commands_str}

## Error Output
{error_output}

## Your Mission
1. Analyze the error output to understand what went wrong
2. Read the relevant source files mentioned in the errors
3. Fix the issues while preserving the intended functionality
4. Run the test command to verify your fix: {commands_str}

## Important
- Focus on fixing the specific errors shown above
- Don't refactor unrelated code
- After each fix attempt, re-run the tests to verify
- Keep trying until the tests pass

## When Done
When the tests pass, signal completion:
```bash
echo "SUCCESS" > {marker_path}
```

If you cannot fix the issue and need human help:
```bash
echo "BLOCKED: <reason>" > {marker_path}
```
"#,
        commands_str = commands_str,
        error_output = error_output,
        marker_path = marker_path,
    );

    let window_name = format!("repair-{}", attempt);

    terminal::spawn_terminal_with_harness_and_model(
        &window_name,
        &prompt,
        working_dir,
        session_name,
        harness,
        model.as_deref(),
    )?;

    let agent_info = if let Some(ref m) = model {
        format!("{}:{}", harness.name(), m)
    } else {
        harness.name().to_string()
    };

    println!(
        "    {} Spawned: {} [{}] {}:{}",
        "✓".green(),
        window_name.cyan(),
        agent_info.dimmed(),
        session_name.dimmed(),
        attempt
    );

    Ok(())
}

/// Wait for repair to complete by polling for marker file
fn wait_for_repair(
    marker_path: &std::path::Path,
    attach: bool,
    session_name: &str,
) -> Result<()> {
    // If attach mode, tell user to come back when done
    if attach {
        println!(
            "    {} Attaching to session. Mark completion with: echo SUCCESS > {}",
            "→".dimmed(),
            marker_path.display()
        );
        terminal::tmux_attach(session_name)?;

        // After detach, check if marker exists
        if marker_path.exists() {
            let content = std::fs::read_to_string(marker_path)?;
            std::fs::remove_file(marker_path)?;

            if content.starts_with("BLOCKED") {
                println!("    {} Agent reported: {}", "!".yellow(), content.trim());
            }
        }
        return Ok(());
    }

    // Poll for marker file
    let timeout = Duration::from_secs(3600); // 1 hour timeout
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_secs(5);

    println!(
        "    {} Attach with: tmux attach -t {}",
        "ℹ".dimmed(),
        session_name
    );

    loop {
        if start.elapsed() > timeout {
            println!(
                "    {} Repair timed out after 1 hour",
                "!".yellow()
            );
            return Ok(());
        }

        if marker_path.exists() {
            let content = std::fs::read_to_string(marker_path)?;
            std::fs::remove_file(marker_path)?;

            if content.starts_with("SUCCESS") {
                println!("    {} Repair agent completed", "✓".green());
            } else if content.starts_with("BLOCKED") {
                println!(
                    "    {} Agent reported blocked: {}",
                    "!".yellow(),
                    content.trim()
                );
                // Continue to next attempt anyway - maybe it partially fixed things
            } else {
                println!("    {} Repair agent finished", "✓".green());
            }

            return Ok(());
        }

        thread::sleep(poll_interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_error_output() {
        let result = ValidationResult {
            all_passed: false,
            failures: vec!["cargo test".to_string()],
            results: vec![backpressure::CommandResult {
                command: "cargo test".to_string(),
                passed: false,
                exit_code: Some(1),
                stdout: "running 2 tests".to_string(),
                stderr: "error: test failed".to_string(),
                duration_secs: 1.5,
            }],
        };

        let output = format_error_output(&result);
        assert!(output.contains("cargo test"));
        assert!(output.contains("Exit code: Some(1)"));
        assert!(output.contains("test failed"));
    }
}
