//! Run command - Launch a single AI agent with an arbitrary prompt
//!
//! This provides a quick way to spin up an agent without needing a task in the graph.
//! Useful for ad-hoc tasks, testing harnesses, or one-off operations.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::spawn::terminal::{self, Harness};

/// Main entry point for the run command
pub fn run(
    project_root: Option<PathBuf>,
    prompt: &str,
    harness_arg: &str,
    model: &str,
    session: Option<String>,
    attach: bool,
    name: Option<String>,
) -> Result<()> {
    // Check tmux is available
    terminal::check_tmux_available()?;

    // Parse harness
    let harness = Harness::parse(harness_arg)?;

    // Generate unique ID for this run
    let run_id = name.unwrap_or_else(|| {
        let timestamp = chrono::Local::now().format("%H%M%S").to_string();
        format!("run-{}", timestamp)
    });

    // Generate session name
    let session_name = session.unwrap_or_else(|| "scud-run".to_string());

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Display what we're doing
    println!("{}", "SCUD Run".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<15} {}", "Harness:".dimmed(), harness.name().green());
    println!("{:<15} {}", "Model:".dimmed(), model.green());
    println!("{:<15} {}", "Session:".dimmed(), session_name.cyan());
    println!("{:<15} {}", "Window:".dimmed(), run_id.cyan());
    println!();
    println!("{}", "Prompt:".dimmed());
    // Truncate long prompts for display
    let display_prompt = if prompt.len() > 200 {
        format!("{}...", &prompt[..200])
    } else {
        prompt.to_string()
    };
    println!("  {}", display_prompt.italic());
    println!();

    // Spawn the agent
    match terminal::spawn_terminal_with_harness_and_model(
        &run_id,
        prompt,
        &working_dir,
        &session_name,
        harness,
        Some(model),
    ) {
        Ok(window_index) => {
            println!(
                "{} Agent spawned: {}:{}",
                "✓".green(),
                session_name.cyan(),
                window_index.cyan()
            );
            println!();
            println!(
                "To attach: {}",
                format!("tmux attach -t {}:{}", session_name, window_index).cyan()
            );

            // Attach if requested
            if attach {
                println!();
                println!("Attaching to session...");
                terminal::tmux_attach(&session_name)?;
            }
        }
        Err(e) => {
            println!("{} Failed to spawn agent: {}", "✗".red(), e);
            return Err(e);
        }
    }

    Ok(())
}
