//! Restart command - Reset a task to pending and spawn it in tmux
//!
//! Unlike spawn which filters for "ready" tasks, restart works on any task
//! regardless of current status (pending, in-progress, done, blocked).
//! Useful for re-running failed tasks or iterating on partially completed work.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::commands::spawn::terminal::{normalize_model_override, Harness};
use crate::commands::spawn::{agent, terminal};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Main entry point for the restart command
pub fn run(
    project_root: Option<PathBuf>,
    task_id: &str,
    tag: Option<&str>,
    harness_arg: &str,
    model: &str,
    session: Option<String>,
    attach: bool,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available
    terminal::check_tmux_available()?;

    // Resolve phase tag
    let phase_tag = resolve_group_tag(&storage, tag, true)?;

    // Load phase and find task
    let mut phase = storage.load_group(&phase_tag)?;
    let task = phase
        .get_task(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in phase '{}'", task_id, phase_tag))?
        .clone();

    // Parse harness
    let harness = Harness::parse(harness_arg)?;
    let model_override = normalize_model_override(harness, model);

    // Generate session name
    let session_name = session.unwrap_or_else(|| format!("scud-{}", phase_tag));

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Display restart info
    println!("{}", "SCUD Restart".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<15} {}", "Task:".dimmed(), task_id.cyan());
    println!("{:<15} {}", "Title:".dimmed(), task.title);
    println!(
        "{:<15} {}",
        "Old Status:".dimmed(),
        task.status.as_str().yellow()
    );
    println!("{:<15} {}", "Harness:".dimmed(), harness.name().green());
    println!(
        "{:<15} {}",
        "Model:".dimmed(),
        model_override.unwrap_or("default").green()
    );
    println!("{:<15} {}", "Session:".dimmed(), session_name.cyan());
    println!();

    // Step 1: Reset task status to Pending
    println!("{}", "Resetting task status...".dimmed());
    {
        let task_mut = phase.get_task_mut(task_id).unwrap();
        task_mut.set_status(TaskStatus::Pending);
        storage.update_group(&phase_tag, &phase)?;
    }
    println!(
        "  {} {} → {}",
        "✓".green(),
        task_id.cyan(),
        "pending".yellow()
    );

    // Step 2: Resolve agent config and spawn
    println!("{}", "Spawning agent...".dimmed());
    let config =
        agent::resolve_agent_config(&task, &phase_tag, harness, model_override, &working_dir);

    // Warn if agent type was specified but definition not found
    if task.agent_type.is_some() && !config.from_agent_def {
        println!(
            "  {} Agent '{}' not found, using CLI defaults",
            "!".yellow(),
            task.agent_type.as_deref().unwrap_or("unknown")
        );
    }

    let spawn_config = terminal::SpawnConfig {
        task_id,
        prompt: &config.prompt,
        working_dir: &working_dir,
        session_name: &session_name,
        harness: config.harness,
        model: config.model.as_deref(),
        task_list_id: None,
    };
    match terminal::spawn_tmux_agent(&spawn_config) {
        Ok(window_index) => {
            println!(
                "  {} Spawned: {} [{}] → {}:{}",
                "✓".green(),
                task_id.cyan(),
                config.display_info().dimmed(),
                session_name.dimmed(),
                window_index.dimmed(),
            );

            // Step 3: Update status to InProgress after successful spawn
            let mut phase = storage.load_group(&phase_tag)?;
            if let Some(task_mut) = phase.get_task_mut(task_id) {
                task_mut.set_status(TaskStatus::InProgress);
                storage.update_group(&phase_tag, &phase)?;
            }
            println!(
                "  {} {} → {}",
                "✓".green(),
                task_id.cyan(),
                "in-progress".green()
            );

            // Summary
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
            println!("  {} Failed to spawn: {}", "✗".red(), e);
            // Task remains in Pending state for easy retry
            return Err(e);
        }
    }

    Ok(())
}
