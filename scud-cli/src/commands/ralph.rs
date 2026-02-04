//! Ralph mode - Sequential iteration loop with fresh context per task
//!
//! Implements the Ralph methodology:
//! 1. Fresh context each iteration - spawns new agent each time
//! 2. One task per loop - focus, complete, validate
//! 3. Backpressure - tests/lint force self-correction
//!
//! Ralph is essentially "swarm with round_size=1" - it processes tasks
//! sequentially with validation between each task.

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::backpressure::{run_validation, BackpressureConfig};
use crate::commands::helpers::resolve_group_tag;
use crate::commands::spawn::monitor::{self, AgentStatus, SpawnSession};
use crate::commands::spawn::terminal::{self, Harness};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    max_iterations: usize,
    no_validate: bool,
    no_repair: bool,
    max_repair_attempts: usize,
    harness_arg: &str,
    model: Option<&str>,
    session_name: Option<String>,
    dry_run: bool,
    push: bool,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available
    terminal::check_tmux_available()?;

    // Resolve tag
    let effective_tag = resolve_group_tag(&storage, tag, true)?;

    // Parse harness
    let harness = Harness::parse(harness_arg)?;
    terminal::find_harness_binary(harness)?;

    // Generate session name
    let session_name = session_name.unwrap_or_else(|| format!("ralph-{}", effective_tag));

    // Load backpressure config
    let bp_config = BackpressureConfig::load(project_root.as_ref())?;

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Display header
    println!("{}", "SCUD Ralph Mode".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<20} {}", "Tag:".dimmed(), effective_tag.green());
    println!("{:<20} {}", "Terminal:".dimmed(), "tmux".cyan());
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().cyan());
    if let Some(m) = model {
        println!("{:<20} {}", "Model:".dimmed(), m.cyan());
    }
    println!(
        "{:<20} {}",
        "Validation:".dimmed(),
        if no_validate {
            "skip".yellow()
        } else {
            "enabled".green()
        }
    );
    println!(
        "{:<20} {}",
        "Repair:".dimmed(),
        if no_repair {
            "disabled".yellow()
        } else {
            format!("up to {} attempts", max_repair_attempts).green()
        }
    );
    if max_iterations > 0 {
        println!(
            "{:<20} {}",
            "Max iterations:".dimmed(),
            max_iterations.to_string().cyan()
        );
    }
    println!();

    if dry_run {
        return run_dry_run(&storage, &effective_tag);
    }

    // Create spawn session for TUI monitoring
    let mut spawn_session = SpawnSession::new(
        &session_name,
        &effective_tag,
        "tmux",
        &working_dir.to_string_lossy(),
    );

    // Main Ralph loop
    run_ralph_loop(
        &storage,
        &mut spawn_session,
        &effective_tag,
        max_iterations,
        no_validate,
        no_repair,
        max_repair_attempts,
        harness,
        model,
        &session_name,
        &working_dir,
        &bp_config,
        push,
        &project_root,
    )
}

fn run_dry_run(storage: &Storage, tag: &str) -> Result<()> {
    println!("{}", "Dry run - showing execution plan:".yellow());
    println!();

    let phases = storage.load_tasks()?;
    let phase = phases.get(tag);

    if let Some(phase) = phase {
        let pending: Vec<_> = phase
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect();

        println!("Tasks to process ({}):", pending.len());
        for (i, task) in pending.iter().enumerate() {
            println!("  {}. {} - {}", i + 1, task.id.cyan(), task.title);
        }
    } else {
        println!("No tasks found for tag: {}", tag);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_ralph_loop(
    storage: &Storage,
    spawn_session: &mut SpawnSession,
    tag: &str,
    max_iterations: usize,
    no_validate: bool,
    no_repair: bool,
    max_repair_attempts: usize,
    harness: Harness,
    model: Option<&str>,
    session_name: &str,
    working_dir: &PathBuf,
    bp_config: &BackpressureConfig,
    push: bool,
    project_root: &Option<PathBuf>,
) -> Result<()> {
    let mut iteration = 0;
    let mut completed_count = 0;
    let mut failed_count = 0;

    loop {
        // Check iteration limit
        if max_iterations > 0 && iteration >= max_iterations {
            println!(
                "{}",
                format!("Reached max iterations: {}", max_iterations).yellow()
            );
            break;
        }

        iteration += 1;
        println!();
        println!(
            "{}",
            format!("═══════════════ ITERATION {} ═══════════════", iteration)
                .cyan()
                .bold()
        );

        // Get next task
        let task = get_next_task(storage, tag)?;

        let Some((task_id, task_title, task_description)) = task else {
            println!(
                "{}",
                "No more tasks available. Ralph complete!".green().bold()
            );
            break;
        };

        println!("Task: {} - {}", task_id.cyan(), task_title);

        // Mark task in-progress
        storage.update_task_status(tag, &task_id, TaskStatus::InProgress)?;

        // Add to spawn session for monitoring
        spawn_session.add_agent(&task_id, &task_title, tag);
        spawn_session.update_agent_status(&task_id, AgentStatus::Running);
        monitor::save_session(project_root.as_ref(), spawn_session)?;

        // Spawn agent with fresh context
        let window_name = format!("task-{}", task_id);
        spawn_ralph_agent(
            &task_id,
            &task_title,
            &task_description,
            harness,
            model,
            session_name,
            &window_name,
            working_dir,
        )?;

        // Wait for agent completion
        println!("  {} Waiting for agent to complete...", "→".dimmed());
        wait_for_agent_completion(session_name, &window_name)?;
        println!("  {} Agent completed", "✓".green());

        // Run backpressure validation
        if !no_validate && !bp_config.commands.is_empty() {
            println!("  {} Running validation...", "→".dimmed());
            let validation = run_validation(working_dir, bp_config)?;

            if !validation.all_passed {
                println!("  {} Validation failed", "✗".red());

                if no_repair {
                    // Mark task as failed and continue
                    storage.update_task_status(tag, &task_id, TaskStatus::Failed)?;
                    spawn_session.update_agent_status(&task_id, AgentStatus::Failed);
                    monitor::save_session(project_root.as_ref(), spawn_session)?;
                    failed_count += 1;
                    continue;
                }

                // Attempt repairs
                let repaired = run_repair_loop(
                    &task_id,
                    &task_title,
                    max_repair_attempts,
                    harness,
                    model,
                    session_name,
                    working_dir,
                    bp_config,
                    &validation,
                )?;

                if !repaired {
                    println!(
                        "  {} Repair failed after {} attempts",
                        "✗".red(),
                        max_repair_attempts
                    );
                    storage.update_task_status(tag, &task_id, TaskStatus::Failed)?;
                    spawn_session.update_agent_status(&task_id, AgentStatus::Failed);
                    monitor::save_session(project_root.as_ref(), spawn_session)?;
                    failed_count += 1;
                    continue;
                }
            }
            println!("  {} Validation passed", "✓".green());
        }

        // Mark task complete
        storage.update_task_status(tag, &task_id, TaskStatus::Done)?;
        spawn_session.update_agent_status(&task_id, AgentStatus::Completed);
        monitor::save_session(project_root.as_ref(), spawn_session)?;
        completed_count += 1;

        // Git push if enabled
        if push {
            println!("  {} Pushing to remote...", "→".dimmed());
            if let Err(e) = git_push(working_dir) {
                println!("  {} Push failed: {}", "!".yellow(), e);
            } else {
                println!("  {} Pushed", "✓".green());
            }
        }

        println!("  {} Task {} complete", "✓".green().bold(), task_id);
    }

    // Final summary
    println!();
    println!(
        "{}",
        "═══════════════ SUMMARY ═══════════════"
            .cyan()
            .bold()
    );
    println!("  Iterations: {}", iteration);
    println!("  Completed:  {} tasks", completed_count);
    println!("  Failed:     {} tasks", failed_count);

    Ok(())
}

/// Get next pending task with satisfied dependencies
fn get_next_task(storage: &Storage, tag: &str) -> Result<Option<(String, String, String)>> {
    let phases = storage.load_tasks()?;
    let phase = phases.get(tag);

    let Some(phase) = phase else {
        return Ok(None);
    };

    // Find first pending task with no blocking dependencies
    for task in &phase.tasks {
        if task.status != TaskStatus::Pending {
            continue;
        }

        // Check if all dependencies are done
        let deps_satisfied = task.dependencies.iter().all(|dep_id| {
            phase
                .tasks
                .iter()
                .find(|t| t.id == *dep_id)
                .map(|t| t.status == TaskStatus::Done)
                .unwrap_or(true) // Treat missing deps as satisfied
        });

        if deps_satisfied {
            return Ok(Some((
                task.id.clone(),
                task.title.clone(),
                task.description.clone(),
            )));
        }
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn spawn_ralph_agent(
    task_id: &str,
    task_title: &str,
    task_description: &str,
    harness: Harness,
    model: Option<&str>,
    session_name: &str,
    window_name: &str,
    working_dir: &Path,
) -> Result<()> {
    // Generate prompt
    let prompt = generate_ralph_prompt(task_id, task_title, task_description);

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("ralph-prompt-{}.txt", task_id));
    std::fs::write(&prompt_file, &prompt)?;

    // Find harness binary
    let binary_path = terminal::find_harness_binary(harness)?;

    // Generate command
    let command = harness.command(binary_path, &prompt_file, model);

    // Spawn in tmux
    terminal::spawn_in_tmux(session_name, window_name, &command, working_dir)?;

    Ok(())
}

fn generate_ralph_prompt(task_id: &str, task_title: &str, task_description: &str) -> String {
    format!(
        r#"You are working on task: {} - {}

## Task Description

{}

## Instructions

1. Study the codebase to understand current state (don't assume functionality is missing)
2. Implement the required changes completely - no placeholders or stubs
3. Run tests to verify your implementation works
4. When tests pass, commit your changes: `git add -A && git commit -m "feat: {}"`

IMPORTANT:
- Complete the entire task in this session
- If you encounter blockers, document them clearly before stopping
- Do NOT leave partial implementations
"#,
        task_id, task_title, task_description, task_title
    )
}

fn wait_for_agent_completion(session_name: &str, window_name: &str) -> Result<()> {
    use std::thread;
    use std::time::Duration;

    // Poll tmux window until it's gone (agent exited)
    loop {
        let output = std::process::Command::new("tmux")
            .args(["list-windows", "-t", session_name, "-F", "#{window_name}"])
            .output()?;

        let windows = String::from_utf8_lossy(&output.stdout);
        if !windows.lines().any(|w| w == window_name) {
            break;
        }

        thread::sleep(Duration::from_secs(5));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_repair_loop(
    task_id: &str,
    task_title: &str,
    max_attempts: usize,
    harness: Harness,
    model: Option<&str>,
    session_name: &str,
    working_dir: &Path,
    bp_config: &BackpressureConfig,
    initial_failure: &crate::backpressure::ValidationResult,
) -> Result<bool> {
    let mut last_failure = initial_failure.clone();

    for attempt in 1..=max_attempts {
        println!(
            "  {} Repair attempt {}/{}...",
            "→".dimmed(),
            attempt,
            max_attempts
        );

        // Generate repair prompt
        let repair_prompt = generate_repair_prompt(task_id, task_title, &last_failure);

        // Write prompt to temp file
        let prompt_file =
            std::env::temp_dir().join(format!("ralph-repair-{}-{}.txt", task_id, attempt));
        std::fs::write(&prompt_file, &repair_prompt)?;

        // Find harness binary
        let binary_path = terminal::find_harness_binary(harness)?;

        // Generate command
        let command = harness.command(binary_path, &prompt_file, model);

        // Spawn repair agent
        let window_name = format!("repair-{}-{}", task_id, attempt);
        terminal::spawn_in_tmux(session_name, &window_name, &command, working_dir)?;

        // Wait for repair agent
        wait_for_agent_completion(session_name, &window_name)?;

        // Re-validate
        let validation = run_validation(working_dir, bp_config)?;
        if validation.all_passed {
            return Ok(true);
        }

        last_failure = validation;
    }

    Ok(false)
}

fn generate_repair_prompt(
    task_id: &str,
    task_title: &str,
    failure: &crate::backpressure::ValidationResult,
) -> String {
    let failures: Vec<String> = failure
        .results
        .iter()
        .filter(|r| !r.passed)
        .map(|r| format!("Command `{}` failed:\n{}\n{}", r.command, r.stdout, r.stderr))
        .collect();

    format!(
        r#"You are repairing validation failures for task: {} - {}

## Validation Failures

{}

## Instructions

1. Analyze the error output above
2. Fix the issues causing validation to fail
3. Run the failing commands to verify your fixes work
4. Commit your fixes: `git add -A && git commit -m "fix: repair validation for {}"`

IMPORTANT:
- Focus only on fixing the validation failures
- Do NOT add new features or refactor unrelated code
- Make minimal changes to fix the specific errors
"#,
        task_id,
        task_title,
        failures.join("\n\n"),
        task_id
    )
}

fn git_push(working_dir: &PathBuf) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["push"])
        .current_dir(working_dir)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
