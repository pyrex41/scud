//! Spawn command - Launch parallel Claude Code agents in tmux sessions
//!
//! This module provides functionality to:
//! - Spawn multiple tmux windows with Claude Code sessions
//! - Generate task-specific prompts for each agent
//! - Track spawn session state for TUI integration
//! - Install Claude Code hooks for automatic task completion

pub mod agent;
pub mod headless;
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
use crate::commands::task_selection::is_actionable_pending_task;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;
use crate::sync::claude_tasks;

use self::headless::StreamStore;
use self::monitor::SpawnSession;
use self::terminal::{normalize_model_override, Harness};

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
    headless: bool,
    harness_arg: &str,
    model_arg: &str,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available (only needed for non-headless mode)
    if !headless {
        terminal::check_tmux_available()?;
    }

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
    let model_override = normalize_model_override(harness, model_arg);

    // Generate session name
    let session_name = session.unwrap_or_else(|| format!("scud-{}", phase_tag));

    // Display spawn plan
    let terminal_type = if headless { "headless" } else { "tmux" };
    println!("{}", "SCUD Spawn".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<20} {}", "Mode:".dimmed(), terminal_type.green());
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().green());
    let model_display = model_override.unwrap_or("default");
    println!("{:<20} {}", "Model:".dimmed(), model_display.green());
    if !headless {
        println!("{:<20} {}", "Session:".dimmed(), session_name.cyan());
    }
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

    // Create stream store for headless mode
    let stream_store = if headless {
        Some(StreamStore::new())
    } else {
        None
    };

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

    // Create spawn session metadata (only for tmux mode)
    let mut spawn_session = if !headless {
        Some(SpawnSession::new(
            &session_name,
            &phase_tag,
            "tmux",
            &working_dir.to_string_lossy(),
        ))
    } else {
        None
    };

    // Spawn agents
    println!("{}", "Spawning agents...".green());

    let mut success_count = 0;
    let mut claimed_tasks: Vec<(String, String)> = Vec::new(); // (task_id, tag) pairs for claiming

    if headless {
        // Headless mode - use streaming runners
        let store = stream_store
            .as_ref()
            .expect("stream_store should be Some in headless mode");

        // Use tokio runtime for async headless spawning
        let rt = tokio::runtime::Runtime::new()?;
        let spawned_ids = rt.block_on(spawn_headless(
            &ready_tasks,
            &working_dir,
            harness,
            model_override,
            store,
        ))?;

        success_count = spawned_ids.len();

        // Track for claiming
        if claim {
            for task_id in &spawned_ids {
                if let Some(info) = ready_tasks.iter().find(|t| t.task.id == *task_id) {
                    claimed_tasks.push((task_id.clone(), info.tag.clone()));
                }
            }
        }
    } else {
        // tmux mode - use terminal spawning
        for info in &ready_tasks {
            // Resolve agent config (harness, model, prompt) from task's agent_type
            let config = agent::resolve_agent_config(
                info.task,
                &info.tag,
                harness,
                model_override,
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

            let spawn_config = terminal::SpawnConfig {
                task_id: &info.task.id,
                prompt: &config.prompt,
                working_dir: &working_dir,
                session_name: &session_name,
                harness: config.harness,
                model: config.model.as_deref(),
                task_list_id: Some(&task_list_id),
            };
            match terminal::spawn_tmux_agent(&spawn_config) {
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
                    if let Some(ref mut session) = spawn_session {
                        session.add_agent(&info.task.id, &info.task.title, &info.tag);
                    }
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

    // Setup control window and save session metadata (tmux mode only)
    if !headless {
        if let Err(e) = terminal::setup_tmux_control_window(&session_name, &phase_tag) {
            println!(
                "  {} Control window setup: {}",
                "!".yellow(),
                e.to_string().dimmed()
            );
        }

        if let Some(ref session) = spawn_session {
            if let Err(e) = monitor::save_session(project_root.as_ref(), session) {
                println!(
                    "  {} Session metadata: {}",
                    "!".yellow(),
                    e.to_string().dimmed()
                );
            }
        }
    }

    // Summary
    println!();
    println!(
        "{} {} of {} agents spawned",
        "Summary:".blue().bold(),
        success_count,
        ready_tasks.len()
    );

    if headless {
        println!();
        println!("To resume: {}", "scud attach <task_id>".cyan());
        println!("To list:   {}", "scud attach --list".dimmed());
    } else {
        println!();
        println!(
            "To attach: {}",
            format!("tmux attach -t {}", session_name).cyan()
        );
        println!(
            "To list:   {}",
            format!("tmux list-windows -t {}", session_name).dimmed()
        );
    }

    // Monitor takes priority over attach
    if monitor {
        println!();
        println!("Starting monitor...");
        // Small delay to let agents start
        thread::sleep(Duration::from_secs(1));
        return tui::run(project_root, &session_name, false, stream_store); // pass store for headless mode
    }

    // Attach if requested (tmux mode only)
    if attach && !headless {
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

    let mode_label = if swarm_mode { "swarm" } else { "spawn" };

    // List available sessions based on mode
    let session_name = match session {
        Some(s) => s,
        None => {
            let sessions = if swarm_mode {
                swarm_session::list_sessions(project_root.as_ref())?
            } else {
                monitor::list_sessions(project_root.as_ref())?
            };
            if sessions.is_empty() {
                let cmd = if swarm_mode {
                    "scud swarm"
                } else {
                    "scud spawn"
                };
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

    tui::run(project_root, &session_name, swarm_mode, None)
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
    if !is_actionable_pending_task(task, phase) {
        return false;
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

/// Spawn agents in headless mode (no tmux)
///
/// This function spawns agents using the HeadlessRunner infrastructure,
/// which captures streaming JSON output instead of using tmux terminals.
/// Session metadata is automatically saved when a session ID is assigned,
/// enabling session continuation via `scud attach`.
///
/// # Arguments
/// * `tasks` - Tasks to spawn agents for
/// * `working_dir` - Working directory for agent processes
/// * `harness` - Which harness to use (Claude or OpenCode)
/// * `model` - Optional model override
/// * `store` - StreamStore for capturing agent output
///
/// # Returns
/// Vector of task IDs that were successfully spawned
async fn spawn_headless(
    tasks: &[TaskInfo<'_>],
    working_dir: &std::path::Path,
    harness: Harness,
    model: Option<&str>,
    store: &StreamStore,
) -> Result<Vec<String>> {
    use crate::commands::attach::{save_session_metadata, SessionMetadata};

    // Create the appropriate runner for the harness
    let runner = headless::create_runner(harness)?;

    let mut spawned_task_ids = Vec::new();

    for info in tasks {
        // Create session in store
        store.create_session(&info.task.id, &info.tag);

        // Resolve agent config (handles agent_type, custom prompts, etc.)
        let config = agent::resolve_agent_config(info.task, &info.tag, harness, model, working_dir);

        // Start headless session
        match runner
            .start(
                &info.task.id,
                &config.prompt,
                working_dir,
                config.model.as_deref(),
            )
            .await
        {
            Ok(mut handle) => {
                // Set PID in store for interruption support
                if let Some(pid) = handle.pid() {
                    store.set_pid(&info.task.id, pid);
                }

                println!(
                    "  {} Spawned (headless): {} | {} [{}]",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed(),
                    config.display_info().dimmed(),
                );

                spawned_task_ids.push(info.task.id.clone());

                // Spawn background task to collect events and save session metadata
                let store_clone = store.clone();
                let task_id = info.task.id.clone();
                let tag = info.tag.clone();
                let harness_name = harness.name().to_string();
                let working_dir = working_dir.to_path_buf();

                tokio::spawn(async move {
                    while let Some(event) = handle.events.recv().await {
                        // Check for session ID assignment - save metadata for continuation
                        if let headless::StreamEventKind::SessionAssigned { ref session_id } =
                            event.kind
                        {
                            store_clone.set_session_id(&task_id, session_id);

                            // Save session metadata for `scud attach` continuation
                            let mut metadata =
                                SessionMetadata::new(&task_id, session_id, &tag, &harness_name);
                            if let Some(pid) = handle.pid() {
                                metadata = metadata.with_pid(pid);
                            }
                            if let Err(e) = save_session_metadata(&working_dir, &metadata) {
                                eprintln!(
                                    "  {} Failed to save session metadata for {}: {}",
                                    "!".yellow(),
                                    task_id,
                                    e
                                );
                            }
                        }

                        // Push event to store for TUI/GUI display
                        store_clone.push_event(&task_id, event);
                    }
                });
            }
            Err(e) => {
                // Record error in store
                store.push_event(&info.task.id, headless::StreamEvent::error(e.to_string()));
                println!(
                    "  {} Failed (headless): {} - {}",
                    "✗".red(),
                    info.task.id.red(),
                    e
                );
            }
        }
    }

    Ok(spawned_task_ids)
}
