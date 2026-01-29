//! Spawn command - Launch parallel Claude Code agents in tmux sessions
//!
//! This module provides functionality to:
//! - Spawn multiple tmux windows with Claude Code sessions
//! - Generate task-specific prompts for each agent
//! - Track spawn session state for TUI integration
//! - Install Claude Code hooks for automatic task completion

pub mod agent;
pub mod hooks;
pub mod monitor;
pub mod terminal;
pub mod tui;

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;
use crate::sync::claude_tasks;

use self::monitor::SpawnSession;
use self::terminal::Harness;

/// Information about a task to spawn
struct TaskInfo<'a> {
    task: &'a Task,
    tag: String,
}

/// Main entry point for the spawn command
#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    limit: usize,
    all_tags: bool,
    dry_run: bool,
    session: Option<String>,
    attach: bool,
    monitor: bool,
    claim: bool,
    harness_arg: &str,
    model_arg: &str,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available
    terminal::check_tmux_available()?;

    // Load all phases for cross-tag dependency checking
    let all_phases = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&all_phases);

    // Determine phase tag
    let phase_tag = if all_tags {
        "all".to_string()
    } else {
        resolve_group_tag(&storage, tag, true)?
    };

    // Get ready tasks
    let ready_tasks = get_ready_tasks(&all_phases, &all_tasks_flat, &phase_tag, limit, all_tags)?;

    if ready_tasks.is_empty() {
        println!("{}", "No ready tasks to spawn.".yellow());
        println!("Check: scud list --status pending");
        return Ok(());
    }

    // Parse harness
    let harness = Harness::parse(harness_arg)?;

    // Generate session name
    let session_name = session.unwrap_or_else(|| format!("scud-{}", phase_tag));

    // Display spawn plan
    println!("{}", "SCUD Spawn".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<20} {}", "Terminal:".dimmed(), "tmux".green());
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().green());
    println!("{:<20} {}", "Model:".dimmed(), model_arg.green());
    println!("{:<20} {}", "Session:".dimmed(), session_name.cyan());
    println!("{:<20} {}", "Tasks:".dimmed(), ready_tasks.len());
    println!();

    for (i, info) in ready_tasks.iter().enumerate() {
        println!(
            "  {} {} {} | {}",
            format!("[{}]", i + 1).dimmed(),
            info.tag.dimmed(),
            info.task.id.cyan(),
            info.task.title
        );
    }
    println!();

    if dry_run {
        println!("{}", "Dry run - no terminals spawned.".yellow());
        return Ok(());
    }

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Check and install Claude Code hooks for automatic task completion
    if !hooks::hooks_installed(&working_dir) {
        println!(
            "{}",
            "Installing Claude Code hooks for task completion...".dimmed()
        );
        if let Err(e) = hooks::install_hooks(&working_dir) {
            println!(
                "  {} Hook installation: {}",
                "!".yellow(),
                e.to_string().dimmed()
            );
        } else {
            println!(
                "  {} Hooks installed (tasks auto-complete on agent stop)",
                "✓".green()
            );
        }
    }

    // Sync tasks to Claude Code's Tasks format
    // This enables agents to see tasks via TaskList tool
    let task_list_id = claude_tasks::task_list_id(&phase_tag);
    if !all_tags {
        // Single tag mode - sync the specific phase
        if let Some(phase) = all_phases.get(&phase_tag) {
            match claude_tasks::sync_phase(phase, &phase_tag) {
                Ok(sync_path) => {
                    let path_str: String = sync_path.display().to_string();
                    println!("  {} Synced tasks to: {}", "✓".green(), path_str.dimmed());
                }
                Err(e) => {
                    let err_str: String = e.to_string();
                    println!("  {} Task sync failed: {}", "!".yellow(), err_str.dimmed());
                }
            }
        }
    } else {
        // All tags mode - sync all phases
        match claude_tasks::sync_phases(&all_phases) {
            Ok(paths) => {
                let count: usize = paths.len();
                println!(
                    "  {} Synced {} phases to Claude Tasks format",
                    "✓".green(),
                    count
                );
            }
            Err(e) => {
                let err_str: String = e.to_string();
                println!("  {} Task sync failed: {}", "!".yellow(), err_str.dimmed());
            }
        }
    }

    // Create spawn session metadata
    let mut spawn_session = SpawnSession::new(
        &session_name,
        &phase_tag,
        "tmux",
        &working_dir.to_string_lossy(),
    );

    // Spawn agents
    println!("{}", "Spawning agents...".green());

    let mut success_count = 0;
    let mut claimed_tasks: Vec<(String, String)> = Vec::new(); // (task_id, tag) pairs for claiming

    for info in &ready_tasks {
        // Resolve agent config (harness, model, prompt) from task's agent_type
        let config = agent::resolve_agent_config(
            info.task,
            &info.tag,
            harness,
            Some(model_arg),
            &working_dir,
        );

        // Warn if agent type was specified but definition not found
        if info.task.agent_type.is_some() && !config.from_agent_def {
            println!(
                "  {} Agent '{}' not found, using CLI defaults",
                "!".yellow(),
                info.task.agent_type.as_deref().unwrap_or("unknown")
            );
        }

        match terminal::spawn_terminal_with_task_list(
            &info.task.id,
            &config.prompt,
            &working_dir,
            &session_name,
            config.harness,
            config.model.as_deref(),
            &task_list_id,
        ) {
            Ok(window_index) => {
                println!(
                    "  {} Spawned: {} | {} [{}] {}:{}",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed(),
                    config.display_info().dimmed(),
                    session_name.dimmed(),
                    window_index.dimmed(),
                );
                spawn_session.add_agent(&info.task.id, &info.task.title, &info.tag);
                success_count += 1;

                // Track for claiming
                if claim {
                    claimed_tasks.push((info.task.id.clone(), info.tag.clone()));
                }
            }
            Err(e) => {
                println!("  {} Failed: {} - {}", "✗".red(), info.task.id.red(), e);
            }
        }

        // Small delay between spawns to avoid overwhelming the system
        if success_count < ready_tasks.len() {
            thread::sleep(Duration::from_millis(500));
        }
    }

    // Claim tasks (mark as in-progress) if requested
    if claim && !claimed_tasks.is_empty() {
        println!();
        println!("{}", "Claiming tasks...".dimmed());
        for (task_id, task_tag) in &claimed_tasks {
            // Reload phase and update task status
            match storage.load_group(task_tag) {
                Ok(mut phase) => {
                    if let Some(task) = phase.get_task_mut(task_id) {
                        task.set_status(TaskStatus::InProgress);
                        if let Err(e) = storage.update_group(task_tag, &phase) {
                            println!(
                                "  {} Claim failed: {} - {}",
                                "!".yellow(),
                                task_id,
                                e.to_string().dimmed()
                            );
                        } else {
                            println!(
                                "  {} Claimed: {} → {}",
                                "✓".green(),
                                task_id.cyan(),
                                "in-progress".yellow()
                            );
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "  {} Claim failed: {} - {}",
                        "!".yellow(),
                        task_id,
                        e.to_string().dimmed()
                    );
                }
            }
        }
    }

    // Setup control window for tmux
    if let Err(e) = terminal::setup_tmux_control_window(&session_name, &phase_tag) {
        println!(
            "  {} Control window setup: {}",
            "!".yellow(),
            e.to_string().dimmed()
        );
    }

    // Save session metadata
    if let Err(e) = monitor::save_session(project_root.as_ref(), &spawn_session) {
        println!(
            "  {} Session metadata: {}",
            "!".yellow(),
            e.to_string().dimmed()
        );
    }

    // Summary
    println!();
    println!(
        "{} {} of {} agents spawned",
        "Summary:".blue().bold(),
        success_count,
        ready_tasks.len()
    );

    println!();
    println!(
        "To attach: {}",
        format!("tmux attach -t {}", session_name).cyan()
    );
    println!(
        "To list:   {}",
        format!("tmux list-windows -t {}", session_name).dimmed()
    );

    // Monitor takes priority over attach
    if monitor {
        println!();
        println!("Starting monitor...");
        // Small delay to let agents start
        thread::sleep(Duration::from_secs(1));
        return tui::run(project_root, &session_name, false); // spawn mode, not swarm
    }

    // Attach if requested
    if attach {
        println!();
        println!("Attaching to session...");
        terminal::tmux_attach(&session_name)?;
    }

    Ok(())
}

/// Run the TUI monitor for a spawn or swarm session
pub fn run_monitor(
    project_root: Option<PathBuf>,
    session: Option<String>,
    swarm_mode: bool,
) -> Result<()> {
    use crate::commands::swarm::session as swarm_session;
    use colored::Colorize;

    // Debug: show project root being used
    let project_root_display = project_root
        .as_ref()
        .and_then(|p| p.to_str())
        .unwrap_or("current directory");

    let mode_label = if swarm_mode { "swarm" } else { "spawn" };
    eprintln!(
        "{} Monitor ({}) looking for sessions in: {}",
        "DEBUG:".yellow(),
        mode_label,
        project_root_display
    );

    // List available sessions based on mode
    let session_name = match session {
        Some(s) => s,
        None => {
            let sessions = if swarm_mode {
                swarm_session::list_sessions(project_root.as_ref())?
            } else {
                monitor::list_sessions(project_root.as_ref())?
            };
            eprintln!(
                "{} Found {} {} session(s): {:?}",
                "DEBUG:".yellow(),
                sessions.len(),
                mode_label,
                sessions
            );
            if sessions.is_empty() {
                let cmd = if swarm_mode {
                    "scud swarm"
                } else {
                    "scud spawn"
                };
                eprintln!(
                    "{} No {} sessions found in: {}",
                    "DEBUG:".yellow(),
                    mode_label,
                    project_root_display
                );
                eprintln!(
                    "{} Run: {} --project {} (if needed)",
                    "HINT:".cyan(),
                    cmd,
                    project_root_display
                );
                anyhow::bail!("No {} sessions found. Run: {}", mode_label, cmd);
            }
            if sessions.len() == 1 {
                sessions[0].clone()
            } else {
                println!(
                    "{}",
                    format!("Available {} sessions:", mode_label).cyan().bold()
                );
                for (i, s) in sessions.iter().enumerate() {
                    println!("  {} {}", format!("[{}]", i + 1).dimmed(), s);
                }
                anyhow::bail!(
                    "Multiple {} sessions found. Specify one with --session <name>",
                    mode_label
                );
            }
        }
    };

    tui::run(project_root, &session_name, swarm_mode)
}

/// List spawn sessions
pub fn run_sessions(project_root: Option<PathBuf>, verbose: bool) -> Result<()> {
    use colored::Colorize;

    let sessions = monitor::list_sessions(project_root.as_ref())?;

    if sessions.is_empty() {
        println!("{}", "No spawn sessions found.".dimmed());
        println!("Run: scud spawn -m --limit 3");
        return Ok(());
    }

    println!("{}", "Spawn Sessions:".cyan().bold());
    println!();

    for session_name in &sessions {
        if verbose {
            // Load full session data
            match monitor::load_session(project_root.as_ref(), session_name) {
                Ok(session) => {
                    let stats = monitor::SpawnStats::from(&session);
                    println!(
                        "  {} {} agents ({} running, {} done)",
                        session_name.cyan(),
                        format!("[{}]", stats.total_agents).dimmed(),
                        stats.running.to_string().green(),
                        stats.completed.to_string().blue()
                    );
                    println!(
                        "    {} Tag: {}, Terminal: {}",
                        "│".dimmed(),
                        session.tag,
                        session.terminal
                    );
                    println!(
                        "    {} Created: {}",
                        "└".dimmed(),
                        session.created_at.dimmed()
                    );
                    println!();
                }
                Err(_) => {
                    println!("  {} {}", session_name, "(unable to load)".red());
                }
            }
        } else {
            println!("  {}", session_name);
        }
    }

    if !verbose {
        println!();
        println!(
            "{}",
            "Use -v for details, or: scud monitor --session <name>".dimmed()
        );
    }

    Ok(())
}

/// Discover all tmux sessions (not just spawn sessions)
pub fn run_discover_sessions(_project_root: Option<PathBuf>) -> Result<()> {
    use colored::Colorize;

    // Get all tmux sessions
    let output = std::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}:#{session_attached}"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to list tmux sessions: {}", e))?;

    if !output.status.success() {
        println!("{}", "No tmux sessions found or tmux not running.".dimmed());
        return Ok(());
    }

    let sessions_output = String::from_utf8_lossy(&output.stdout);
    let sessions: Vec<&str> = sessions_output.lines().collect();

    if sessions.is_empty() {
        println!("{}", "No tmux sessions found.".dimmed());
        return Ok(());
    }

    println!("{}", "Discovered Sessions:".cyan().bold());
    println!();

    for session_line in sessions {
        if let Some((session_name, attached)) = session_line.split_once(':') {
            let attached_indicator = if attached == "1" {
                "(attached)".green()
            } else {
                "(detached)".dimmed()
            };
            println!("  {} {}", session_name.cyan(), attached_indicator);
        }
    }

    println!();
    println!(
        "{}",
        "Use 'scud attach <session>' to attach to a session.".dimmed()
    );

    Ok(())
}

/// Attach to a tmux session
pub fn run_attach_session(_project_root: Option<PathBuf>, session_name: &str) -> Result<()> {
    use colored::Colorize;

    // Check if tmux is available
    terminal::check_tmux_available()?;

    // Check if session exists
    if !terminal::tmux_session_exists(session_name) {
        anyhow::bail!(
            "Session '{}' does not exist. Use 'scud discover' to list available sessions.",
            session_name
        );
    }

    println!("Attaching to session '{}'...", session_name.cyan());
    terminal::tmux_attach(session_name)?;

    Ok(())
}

/// Detach from current tmux session
pub fn run_detach_session(_project_root: Option<PathBuf>) -> Result<()> {
    use colored::Colorize;

    // Check if we're in a tmux session
    if std::env::var("TMUX").is_err() {
        println!("{}", "Not currently in a tmux session.".yellow());
        return Ok(());
    }

    // Send detach command to tmux
    let output = std::process::Command::new("tmux")
        .args(["detach"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to detach: {}", e))?;

    if output.status.success() {
        println!("{}", "Detached from tmux session.".green());
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to detach: {}", stderr);
    }

    Ok(())
}

/// Get ready tasks for spawning
fn get_ready_tasks<'a>(
    all_phases: &'a std::collections::HashMap<String, crate::models::phase::Phase>,
    all_tasks_flat: &[&Task],
    phase_tag: &str,
    limit: usize,
    all_tags: bool,
) -> Result<Vec<TaskInfo<'a>>> {
    let mut ready_tasks: Vec<TaskInfo<'a>> = Vec::new();

    if all_tags {
        // Collect from all phases
        for (tag, phase) in all_phases {
            for task in &phase.tasks {
                if is_task_ready(task, phase, all_tasks_flat) {
                    ready_tasks.push(TaskInfo {
                        task,
                        tag: tag.clone(),
                    });
                }
            }
        }
    } else {
        // Single phase
        let phase = all_phases
            .get(phase_tag)
            .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

        for task in &phase.tasks {
            if is_task_ready(task, phase, all_tasks_flat) {
                ready_tasks.push(TaskInfo {
                    task,
                    tag: phase_tag.to_string(),
                });
            }
        }
    }

    // Truncate to limit
    ready_tasks.truncate(limit);

    Ok(ready_tasks)
}

/// Check if a task is ready to be spawned
fn is_task_ready(
    task: &Task,
    phase: &crate::models::phase::Phase,
    all_tasks_flat: &[&Task],
) -> bool {
    // Must be pending
    if task.status != TaskStatus::Pending {
        return false;
    }

    // Must not be expanded (we want subtasks, not parent tasks)
    if task.is_expanded() {
        return false;
    }

    // If it's a subtask, parent must be expanded
    if let Some(ref parent_id) = task.parent_id {
        let parent_expanded = phase
            .get_task(parent_id)
            .map(|p| p.is_expanded())
            .unwrap_or(false);
        if !parent_expanded {
            return false;
        }
    }

    // All dependencies must be met
    task.has_dependencies_met_refs(all_tasks_flat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::phase::Phase;
    use crate::models::task::Task;

    #[test]
    fn test_is_task_ready_basic() {
        let mut phase = Phase::new("test".to_string());
        let task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        phase.add_task(task);

        let all_tasks: Vec<&Task> = phase.tasks.iter().collect();
        assert!(is_task_ready(&phase.tasks[0], &phase, &all_tasks));
    }

    #[test]
    fn test_is_task_ready_in_progress() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.set_status(TaskStatus::InProgress);
        phase.add_task(task);

        let all_tasks: Vec<&Task> = phase.tasks.iter().collect();
        assert!(!is_task_ready(&phase.tasks[0], &phase, &all_tasks));
    }

    #[test]
    fn test_is_task_ready_blocked_by_deps() {
        let mut phase = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "First".to_string(), "Desc".to_string());

        let mut task2 = Task::new("2".to_string(), "Second".to_string(), "Desc".to_string());
        task2.dependencies = vec!["1".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);

        let all_tasks: Vec<&Task> = phase.tasks.iter().collect();

        // Task 1 is ready (no deps)
        assert!(is_task_ready(&phase.tasks[0], &phase, &all_tasks));
        // Task 2 is NOT ready (dep not done)
        assert!(!is_task_ready(&phase.tasks[1], &phase, &all_tasks));
    }
}
