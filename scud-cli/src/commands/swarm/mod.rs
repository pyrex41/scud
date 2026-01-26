//! Swarm mode - Parallel execution with multiple strategies
//!
//! Executes tasks using parallel agents with two main modes:
//!
//! ## Wave Mode (default)
//! Executes tasks in dependency-order waves using parallel agents.
//! After each wave, runs backpressure validation (build, lint, test).
//!
//! Flow:
//! 1. [Optional] Research phase: Smart model analyzes tasks, may expand complex ones
//! 2. Build phase: Fast models execute tasks in parallel rounds
//! 3. Validate phase: Runs backpressure tests (compile, lint, test), smart model fixes issues
//! 4. Repeat for next wave
//!
//! ## Beads Mode (`--swarm-mode beads`)
//! Continuous ready-task polling inspired by Beads/Gas Town patterns.
//! Tasks execute immediately when dependencies are met, no batch waiting.
//!
//! Flow:
//! 1. Query for ready tasks (all dependencies Done)
//! 2. Claim task (mark in-progress)
//! 3. Spawn agent
//! 4. Immediately loop back to step 1 (no waiting for batch)
//!
//! Usage:
//!   scud swarm --tag <tag>                           # Wave mode with tmux (default)
//!   scud swarm --tag <tag> --swarm-mode beads        # Beads continuous execution
//!   scud swarm --tag <tag> --swarm-mode extensions   # Wave mode with async subprocesses
//!   scud swarm --tag <tag> --no-research             # Skip research, use tasks as-is
//!   scud swarm --tag <tag> --no-validate             # Skip backpressure validation

pub mod beads;
pub mod events;
pub mod session;
pub mod transcript;

/// Re-export backpressure module for backward compatibility.
///
/// The canonical location is now [`crate::backpressure`], but this re-export
/// maintains the old path `scud::commands::swarm::backpressure` for existing code.
pub use crate::backpressure;

use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::commands::helpers::resolve_group_tag;
use crate::commands::spawn::agent;
use crate::commands::spawn::hooks;
use crate::commands::spawn::monitor::{self, SpawnSession};
use crate::commands::spawn::terminal::{self, Harness};
use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;
use std::path::Path;

use self::session::{acquire_session_lock, RoundState, SwarmSession, WaveState, WaveSummary};
use crate::agents::AgentDef;
use crate::attribution::{attribute_failure, AttributionConfidence};
use crate::backpressure::{BackpressureConfig, ValidationResult};
use crate::transcript_watcher::TranscriptWatcher;

/// Swarm execution mode
pub use crate::SwarmMode;

/// Main entry point for the swarm command
#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    round_size: usize,
    all_tags: bool,
    harness_arg: &str,
    swarm_mode: SwarmMode,
    dry_run: bool,
    session_name: Option<String>,
    no_research: bool,
    no_validate: bool,
    review: bool,
    review_all: bool,
    no_repair: bool,
    max_repair_attempts: usize,
    no_worktree: bool,
    salvo_dir: Option<PathBuf>,
    stale_timeout_minutes: Option<u64>,
) -> Result<()> {
    let effective_tag = tag.unwrap_or("default");

    if round_size == 0 {
        anyhow::bail!("--round-size must be at least 1");
    }

    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available (only in tmux mode)
    if matches!(swarm_mode, SwarmMode::Tmux) {
        terminal::check_tmux_available()?;
    }

    // Determine phase tag
    let phase_tag = if all_tags {
        "all".to_string()
    } else {
        resolve_group_tag(&storage, tag, true)?
    };

    // Acquire session lock to prevent concurrent swarm runs on same tag
    // Lock is held for the duration of the function and released on drop
    let _session_lock = if !dry_run {
        Some(acquire_session_lock(project_root.as_ref(), &phase_tag)?)
    } else {
        None
    };

    // Parse harness and validate binary exists
    let harness = Harness::parse(harness_arg)?;
    terminal::find_harness_binary(harness)?;

    // Generate session name
    let session_name = session_name.unwrap_or_else(|| format!("swarm-{}", effective_tag));

    // Get working directory
    let original_working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Determine actual working directory (may be a salvo worktree)
    let (working_dir, is_salvo_worktree, main_project_root) =
        if !no_worktree && tag.is_some() && !all_tags {
            let tag_name = tag.unwrap();
            match crate::commands::salvo::ensure_worktree(
                &original_working_dir,
                tag_name,
                salvo_dir.as_deref(),
            ) {
                Ok(wt_path) => (wt_path, true, Some(original_working_dir.clone())),
                Err(e) => {
                    eprintln!("Warning: Could not create salvo worktree: {}", e);
                    eprintln!("Running in-place (use --no-worktree to suppress this warning)");
                    (original_working_dir.clone(), false, None)
                }
            }
        } else {
            (original_working_dir.clone(), false, None)
        };

    // Load backpressure configuration
    let bp_config = BackpressureConfig::load(project_root.as_ref())?;

    // Start transcript watcher in background thread
    if !dry_run {
        let watcher_session = session_name.clone();
        let watcher_root = working_dir.clone();
        let _watcher_handle = std::thread::spawn(move || {
            let db = std::sync::Arc::new(crate::db::Database::new(&watcher_root));
            if db.initialize().is_err() {
                return;
            }
            let watcher = TranscriptWatcher::new(&watcher_root, db);
            if let Err(e) = watcher.watch(&watcher_session) {
                eprintln!("Transcript watcher error: {}", e);
            }
        });
    }

    // Display header
    println!("{}", "SCUD Swarm Mode".cyan().bold());
    println!("{}", "═".repeat(50));
    println!("{:<20} {}", "Tag:".dimmed(), phase_tag.green());
    println!(
        "{:<20} {}",
        "Round size:".dimmed(),
        round_size.to_string().cyan()
    );
    println!(
        "{:<20} {}",
        "Research:".dimmed(),
        if no_research {
            "skip".yellow()
        } else {
            "enabled".green()
        }
    );
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
        "Mode:".dimmed(),
        match swarm_mode {
            SwarmMode::Tmux => "tmux (waves)".cyan(),
            SwarmMode::Extensions => "extensions (waves)".green(),
            SwarmMode::Server => "server (opencode)".magenta(),
            SwarmMode::Beads => "beads (continuous)".yellow(),
        }
    );
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().cyan());
    println!(
        "{:<20} {}",
        "Review:".dimmed(),
        if review_all {
            "all tasks".green()
        } else if review {
            "sample (3 per wave)".green()
        } else {
            "disabled".yellow()
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

    if !bp_config.commands.is_empty() && !no_validate {
        println!(
            "{:<20} {}",
            "Backpressure:".dimmed(),
            bp_config.commands.join(", ").dimmed()
        );
    }
    println!();

    if dry_run {
        return run_dry_run(project_root, &phase_tag, round_size, all_tags);
    }

    // Install hooks if needed
    if !hooks::hooks_installed(&working_dir) {
        println!("{}", "Installing Claude Code hooks...".dimmed());
        if let Err(e) = hooks::install_hooks(&working_dir) {
            println!(
                "  {} Hook installation: {}",
                "!".yellow(),
                e.to_string().dimmed()
            );
        } else {
            println!("  {} Hooks installed", "✓".green());
        }
    }

    // Initialize swarm session
    let terminal_mode = match swarm_mode {
        SwarmMode::Tmux => "tmux",
        SwarmMode::Extensions => "extensions",
        SwarmMode::Beads => "beads",
        SwarmMode::Server => "server",
    };
    let mut swarm_session = SwarmSession::new(
        &session_name,
        &phase_tag,
        terminal_mode,
        &working_dir.to_string_lossy(),
        round_size,
    );

    // Compute stale timeout duration
    let stale_timeout = stale_timeout_minutes.map(|m| Duration::from_secs(m * 60));

    // Create EventWriter for SQLite event logging (Phase 1b)
    let event_writer = events::EventWriter::new(&working_dir, &session_name).ok();

    // Detect orphan in-progress tasks (tasks with no running tmux window)
    // Only applicable in tmux mode
    let all_phases = storage.load_tasks()?;
    if matches!(swarm_mode, SwarmMode::Tmux) {
        let orphans = find_orphan_tasks(&all_phases, &phase_tag, all_tags, &session_name);

        if !orphans.is_empty() {
            println!();
            println!(
                "{}",
                "Detected orphan in-progress tasks (no tmux window):".yellow()
            );
            for (task_id, tag) in &orphans {
                println!(
                    "  {} {} (tag: {})",
                    "*".yellow(),
                    task_id.cyan(),
                    tag.dimmed()
                );
            }
            println!();

            // Prompt user for action
            let choices = vec![
                "Reset to pending and re-run",
                "Kill existing windows (if any) and restart",
                "Skip and continue (leave as in-progress)",
                "Abort",
            ];

            let selection = dialoguer::Select::new()
                .with_prompt("How should orphan tasks be handled?")
                .items(&choices)
                .default(0)
                .interact()?;

            match selection {
                0 => {
                    // Reset to pending
                    for (task_id, tag) in &orphans {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Pending);
                                storage.update_group(tag, &phase)?;
                                println!("  {} {} -> pending", "v".green(), task_id);
                            }
                        }
                    }
                }
                1 => {
                    // Kill and restart - first try to kill any matching windows
                    for (task_id, _) in &orphans {
                        let window_name = format!("task-{}", task_id);
                        let _ = terminal::kill_tmux_window(&session_name, &window_name);
                    }
                    // Reset to pending so they'll be picked up
                    for (task_id, tag) in &orphans {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Pending);
                                storage.update_group(tag, &phase)?;
                                println!("  {} {} -> pending (will re-spawn)", "v".green(), task_id);
                            }
                        }
                    }
                }
                2 => {
                    // Skip - do nothing, leave as in-progress
                    println!("{}", "Leaving orphan tasks as in-progress.".dimmed());
                }
                3 => {
                    // Abort
                    anyhow::bail!("Aborted by user");
                }
            _ => {}
            }
            println!();
        }
    }

    // === BEADS MODE: Continuous execution ===
    // If beads mode is selected, run the continuous polling loop instead of waves
    if matches!(swarm_mode, SwarmMode::Beads) {
        let beads_config = beads::BeadsConfig {
            max_concurrent: round_size, // Reuse round_size as max concurrent
            poll_interval: Duration::from_secs(3),
        };

        // Check tmux availability for beads mode (uses tmux by default)
        terminal::check_tmux_available()?;

        let result = beads::run_beads_loop(
            &storage,
            &phase_tag,
            all_tags,
            &working_dir,
            &session_name,
            harness,
            &beads_config,
            &mut swarm_session,
        )?;

        // Save session state
        session::save_session(project_root.as_ref(), &swarm_session)?;

        // Final summary
        println!();
        println!("{}", "Beads Session Summary".blue().bold());
        println!("{}", "═".repeat(40).blue());
        println!(
            "  Tasks completed: {}",
            result.tasks_completed.to_string().green()
        );
        println!(
            "  Tasks failed:    {}",
            if result.tasks_failed > 0 {
                result.tasks_failed.to_string().red()
            } else {
                "0".to_string().green()
            }
        );
        println!(
            "  Duration:        {}",
            format!("{:.1}s", result.total_duration.as_secs_f64()).cyan()
        );

        return Ok(());
    }

    // === WAVE MODE: Batch execution ===
    // Main loop: execute waves until all tasks done
    let mut wave_number = 1;
    loop {
        // Load fresh task state (must reload each iteration to see completed tasks)
        let all_phases = storage.load_tasks()?;

        // Compute waves from current state
        let waves = compute_waves_from_tasks(&all_phases, &phase_tag, all_tags)?;

        if waves.is_empty() {
            println!();
            println!("{}", "All tasks complete!".green().bold());
            break;
        }

        // Get first wave (tasks with no pending dependencies)
        let wave_tasks = &waves[0];

        if wave_tasks.is_empty() {
            println!();
            println!("{}", "No ready tasks in current wave.".yellow());

            let in_progress_count = count_in_progress(&all_phases, &phase_tag, all_tags);
            if in_progress_count > 0 {
                // Check for stale in-progress tasks whose tmux windows are gone (1d)
                if matches!(swarm_mode, SwarmMode::Tmux) {
                    let orphans =
                        find_orphan_tasks(&all_phases, &phase_tag, all_tags, &session_name);
                    for (task_id, tag) in &orphans {
                        println!(
                            "  {} {} has no tmux window, resetting to pending",
                            "⚠".yellow(),
                            task_id.cyan()
                        );
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Pending);
                                let _ = storage.update_group(tag, &phase);
                            }
                        }
                    }
                    if !orphans.is_empty() {
                        continue; // Re-check waves with reset tasks
                    }
                }

                println!(
                    "Waiting for {} in-progress task(s) to complete...",
                    in_progress_count.to_string().cyan()
                );
                thread::sleep(Duration::from_secs(10));
                continue;
            } else {
                println!("Check for blocked tasks: scud list --status blocked");
                break;
            }
        }

        println!();
        println!(
            "{} {} - {} task(s)",
            "Wave".blue().bold(),
            wave_number.to_string().cyan(),
            wave_tasks.len()
        );
        println!("{}", "-".repeat(40).blue());

        // Track wave state
        let mut wave_state = WaveState::new(wave_number);
        let wave_start = std::time::Instant::now();

        // Emit wave started event
        if let Some(ref writer) = event_writer {
            let _ = writer.log_wave_started(wave_number, wave_tasks.len());
        }

        // === PHASE 1: RESEARCH (optional, first wave only) ===
        if !no_research && wave_number == 1 {
            println!();
            println!("  {} Analyzing tasks...", "Research:".magenta());
            // TODO: Smart model could expand complex tasks here
            println!("    {} Task analysis complete", "✓".green());
        }

        // === PHASE 2: BUILD ===
        let num_rounds = wave_tasks.len().div_ceil(round_size);
        for (round_idx, round_tasks) in wave_tasks.chunks(round_size).enumerate() {
            println!();
            println!(
                "  {} {}/{} - {} task(s)",
                "Round".yellow(),
                round_idx + 1,
                num_rounds,
                round_tasks.len()
            );

            // Spawn agents for this round based on swarm mode
            // Note: Agents self-orient using scud CLI commands (scud list, scud show, etc.)
            let round_state = match swarm_mode {
                SwarmMode::Tmux => {
                    let state = execute_round(
                        &storage,
                        round_tasks,
                        &working_dir,
                        &session_name,
                        round_idx,
                        harness,
                        event_writer.as_ref(),
                    )?;

                    // Create/update spawn proxy for monitor visibility (tmux mode only)
                    let _proxy_path = create_and_update_spawn_proxy(
                        &storage,
                        project_root.as_ref(),
                        &session_name,
                        &phase_tag,
                        &working_dir,
                        &swarm_session,
                        Some(&state),
                    )?;

                    // Wait for round completion (tmux mode - poll for status changes)
                    println!("    Waiting for round completion...");
                    wait_for_round_completion(
                        &storage,
                        round_tasks,
                        &session_name,
                        stale_timeout,
                        event_writer.as_ref(),
                    )?;

                    state
                }
                SwarmMode::Extensions => {
                    // Extensions mode: synchronous execution with async subprocess runner
                    execute_round_extensions(
                        &storage,
                        round_tasks,
                        &working_dir,
                        round_idx,
                        harness,
                    )?
                }
                SwarmMode::Server => {
                    // Server mode: use OpenCode Server for agent orchestration
                    execute_round_server(
                        &storage,
                        round_tasks,
                        &working_dir,
                        round_idx,
                    )?
                }
                SwarmMode::Beads => {
                    // Beads mode is handled earlier with early return
                    // This branch should never be reached
                    unreachable!("Beads mode should exit before wave loop")
                }
            };

            wave_state.rounds.push(round_state.clone());
            println!("    {} Round {} complete", "✓".green(), round_idx + 1);
        }

        // === PHASE 3: VALIDATE (optional) ===
        if !no_validate && !bp_config.commands.is_empty() {
            println!();
            println!("  {} Running backpressure checks...", "Validate:".magenta());

            let validation_result = backpressure::run_validation(&working_dir, &bp_config)?;

            if validation_result.all_passed {
                println!("    {} All checks passed", "✓".green());

                // Emit validation passed event
                if let Some(ref writer) = event_writer {
                    let _ = writer.log_validation_passed();
                }

                // Mark all tasks as done
                for (task_id, tag) in wave_state.task_tags() {
                    if let Ok(mut phase) = storage.load_group(&tag) {
                        if let Some(task) = phase.get_task_mut(&task_id) {
                            task.set_status(TaskStatus::Done);
                            let _ = storage.update_group(&tag, &phase);
                        }
                    }
                }
            } else {
                println!("    {} Some checks failed:", "!".yellow());
                for failure in &validation_result.failures {
                    println!("      - {}", failure.red());
                }

                // Emit validation failed event
                if let Some(ref writer) = event_writer {
                    let _ = writer.log_validation_failed(&validation_result.failures);
                }

                if no_repair {
                    // Old behavior: mark all tasks as failed
                    let task_tags = wave_state.task_tags();
                    for (task_id, tag) in &task_tags {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Failed);
                                let _ = storage.update_group(tag, &phase);
                            }
                        }
                    }
                    println!(
                        "    {} Marked {} task(s) as failed",
                        "!".yellow(),
                        task_tags.len()
                    );
                } else {
                    // New behavior: run repair loop
                    let repaired = run_repair_loop(
                        &storage,
                        &working_dir,
                        &session_name,
                        &bp_config,
                        &wave_state,
                        &validation_result,
                        max_repair_attempts,
                    )?;

                    if !repaired {
                        println!("    {} Wave failed after repair attempts", "!".red());
                    }
                }
            }

            wave_state.validation = Some(validation_result);
        }

        // Generate wave summary (just what was done - not context accumulation)
        let summary = WaveSummary {
            wave_number,
            tasks_completed: wave_state.all_task_ids(),
            files_changed: collect_changed_files(&working_dir, wave_state.start_commit.as_deref())
                .unwrap_or_default(),
        };
        wave_state.summary = Some(summary.clone());

        // === PHASE 4: REVIEW (optional) ===
        if (review || review_all) && !dry_run {
            // Build task list for review
            let wave_tasks: Vec<(String, String)> = wave_state
                .task_tags()
                .iter()
                .filter_map(|(id, tag)| {
                    storage
                        .load_group(tag)
                        .ok()
                        .and_then(|phase| phase.get_task(id).map(|t| (id.clone(), t.title.clone())))
                })
                .collect();

            if !wave_tasks.is_empty() {
                let review_result = spawn_reviewer(
                    &working_dir,
                    &session_name,
                    &summary,
                    &wave_tasks,
                    review_all,
                )?;

                if !review_result.all_passed && !review_result.tasks_to_improve.is_empty() {
                    println!(
                        "    {} Reviewer found issues in: {}",
                        "!".yellow(),
                        review_result.tasks_to_improve.join(", ")
                    );

                    // Spawn improvement agents for flagged tasks
                    for task_id in &review_result.tasks_to_improve {
                        // Find task and spawn builder to improve
                        if let Some((task, _tag)) =
                            find_task_with_tag(&storage, task_id, &wave_state.task_tags())
                        {
                            let prompt = format!(
                                "Improve SCUD task {}: {}\n\nThe reviewer flagged this task for improvements. \
                                 Review the implementation and make it better. When done: scud set-status {} done",
                                task.id, task.title, task.id
                            );

                            // Use builder agent for improvements
                            if let Some(agent_def) = AgentDef::try_load("builder", &working_dir) {
                                let harness = agent_def.harness()?;
                                let model = agent_def.model();

                                terminal::spawn_terminal_with_harness_and_model(
                                    &format!("improve-{}", task_id),
                                    &prompt,
                                    &working_dir,
                                    &session_name,
                                    harness,
                                    model,
                                )?;

                                println!(
                                    "    {} Spawned improvement agent for {}",
                                    "✓".green(),
                                    task_id
                                );
                            }
                        }
                    }
                } else {
                    println!("    {} Review complete, all tasks approved", "✓".green());
                }
            }
        }

        // Emit wave completed event
        if let Some(ref writer) = event_writer {
            let _ = writer.log_wave_completed(
                wave_number,
                wave_start.elapsed().as_millis() as u64,
            );
        }

        // Save session state
        swarm_session.waves.push(wave_state);
        session::save_session(project_root.as_ref(), &swarm_session)?;

        wave_number += 1;
    }

    // Final summary
    // Final bridge update for spawn monitor/TUI compatibility
    create_and_update_spawn_proxy(
        &storage,
        project_root.as_ref(),
        &session_name,
        &phase_tag,
        &working_dir,
        &swarm_session,
        None, // Final update - include all rounds
    )?;

    println!();
    println!("{}", "Swarm Session Summary".blue().bold());
    println!("{}", "═".repeat(40).blue());
    println!(
        "  Waves completed: {}",
        swarm_session.waves.len().to_string().green()
    );

    let total_tasks: usize = swarm_session
        .waves
        .iter()
        .flat_map(|w| &w.rounds)
        .map(|r| r.task_ids.len())
        .sum();
    println!("  Tasks executed: {}", total_tasks.to_string().green());

    println!("  {} Spawn proxy updated for monitor/TUI", "✓".green());

    // Auto-sync worktree results back to main
    if is_salvo_worktree {
        if let (Some(main_root), Some(tag_name)) = (&main_project_root, &tag) {
            if let Err(e) =
                crate::commands::salvo::sync_to_main(main_root, &working_dir, tag_name)
            {
                eprintln!("Warning: Failed to sync salvo back to main: {}", e);
                eprintln!("Run manually: scud salvo sync {}", tag_name);
            }
        }
    }

    Ok(())
}

fn create_and_update_spawn_proxy(
    storage: &Storage,
    project_root: Option<&PathBuf>,
    session_name: &str,
    phase_tag: &str,
    working_dir: &Path,
    swarm_session: &SwarmSession,
    latest_round: Option<&RoundState>,
) -> Result<Option<PathBuf>> {
    let all_phases = storage.load_tasks()?;

    // Try to load existing proxy session, or create new one
    let mut spawn_session = match monitor::load_session(project_root, session_name) {
        Ok(existing) => existing,
        Err(_) => SpawnSession::new(
            session_name,
            phase_tag,
            "tmux",
            &working_dir.to_string_lossy(),
        ),
    };

    // Get tasks to add (either from latest round or all tasks)
    let tasks_to_add: Vec<String> = match latest_round {
        Some(round) => round.task_ids.clone(),
        None => swarm_session
            .waves
            .iter()
            .flat_map(|w| w.all_task_ids())
            .collect(),
    };

    // Add new agents (skip duplicates)
    let existing_task_ids: std::collections::HashSet<String> = spawn_session
        .agents
        .iter()
        .map(|a| a.task_id.clone())
        .collect();

    for task_id in &tasks_to_add {
        if !existing_task_ids.contains(task_id) {
            if let Some((title, tag)) = find_task_title_tag(&all_phases, task_id) {
                spawn_session.add_agent(task_id, &title, &tag);
            }
        }
    }

    let session_file = monitor::save_session(project_root, &spawn_session)?;
    Ok(Some(session_file))
}

fn find_task_title_tag<'a>(
    phases: &'a HashMap<String, crate::models::phase::Phase>,
    task_id: &str,
) -> Option<(String, String)> {
    for (tag, phase) in phases {
        if let Some(task) = phase.get_task(task_id) {
            return Some((task.title.clone(), tag.clone()));
        }
    }
    None
}

/// Task info for wave computation
#[derive(Clone)]
struct TaskInfo<'a> {
    task: &'a Task,
    tag: String,
}

/// Compute execution waves from current task state
fn compute_waves_from_tasks<'a>(
    all_phases: &'a HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> Result<Vec<Vec<TaskInfo<'a>>>> {
    use std::collections::HashSet;

    let mut actionable: Vec<TaskInfo<'a>> = Vec::new();

    let phase_tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases
            .keys()
            .filter(|t| t.as_str() == phase_tag)
            .collect()
    };

    for tag in phase_tags {
        if let Some(phase) = all_phases.get(tag) {
            for task in &phase.tasks {
                if is_task_actionable(task, phase) {
                    actionable.push(TaskInfo {
                        task,
                        tag: tag.clone(),
                    });
                }
            }
        }
    }

    if actionable.is_empty() {
        return Ok(Vec::new());
    }

    // Kahn's algorithm for wave computation
    let task_ids: HashSet<String> = actionable.iter().map(|t| t.task.id.clone()).collect();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    // Collect in-progress task IDs for blocking check
    let in_progress_ids: HashSet<String> = {
        let tags: Vec<&str> = if all_tags {
            all_phases.keys().map(|s| s.as_str()).collect()
        } else {
            vec![phase_tag]
        };

        tags.iter()
            .filter_map(|tag| all_phases.get(*tag))
            .flat_map(|phase| &phase.tasks)
            .filter(|t| t.status == TaskStatus::InProgress)
            .map(|t| t.id.clone())
            .collect()
    };

    for info in &actionable {
        in_degree.entry(info.task.id.clone()).or_insert(0);
        for dep in &info.task.dependencies {
            if task_ids.contains(dep) {
                // Dependency is pending - will be in a wave
                *in_degree.entry(info.task.id.clone()).or_insert(0) += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(info.task.id.clone());
            } else if in_progress_ids.contains(dep) {
                // Dependency is in-progress - block this task
                // Set very high in-degree so it never becomes ready
                *in_degree.entry(info.task.id.clone()).or_insert(0) += 1000;
            }
            // If dep is Done/Failed/etc, it's satisfied - do nothing
        }
    }

    let mut waves: Vec<Vec<TaskInfo<'a>>> = Vec::new();
    let mut remaining = in_degree.clone();

    while !remaining.is_empty() {
        let ready: Vec<String> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        if ready.is_empty() {
            break; // Circular dependency
        }

        let wave: Vec<TaskInfo<'a>> = actionable
            .iter()
            .filter(|t| ready.contains(&t.task.id))
            .cloned()
            .collect();

        for task_id in &ready {
            remaining.remove(task_id);
            if let Some(deps) = dependents.get(task_id) {
                for dep_id in deps {
                    if let Some(deg) = remaining.get_mut(dep_id) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        waves.push(wave);
    }

    Ok(waves)
}

fn is_task_actionable(task: &Task, phase: &Phase) -> bool {
    if task.status != TaskStatus::Pending {
        return false;
    }
    if task.is_expanded() {
        return false;
    }
    if let Some(ref parent_id) = task.parent_id {
        let parent_expanded = phase
            .get_task(parent_id)
            .map(|p| p.is_expanded())
            .unwrap_or(false);
        if !parent_expanded {
            return false;
        }
    }
    true
}

fn count_in_progress(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> usize {
    let tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases
            .keys()
            .filter(|t| t.as_str() == phase_tag)
            .collect()
    };

    tags.iter()
        .filter_map(|tag| all_phases.get(*tag))
        .flat_map(|phase| &phase.tasks)
        .filter(|t| t.status == TaskStatus::InProgress)
        .count()
}

/// Check if a tmux window exists for a task
fn tmux_window_exists_for_task(session_name: &str, task_id: &str) -> bool {
    let window_name = format!("task-{}", task_id);
    terminal::tmux_window_exists(session_name, &window_name)
}

/// Find in-progress tasks that have no running tmux window (orphans)
fn find_orphan_tasks(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
    session_name: &str,
) -> Vec<(String, String)> {
    // (task_id, tag) pairs
    let tags: Vec<&str> = if all_tags {
        all_phases.keys().map(|s| s.as_str()).collect()
    } else {
        vec![phase_tag]
    };

    let mut orphans = Vec::new();

    for tag in tags {
        if let Some(phase) = all_phases.get(tag) {
            for task in &phase.tasks {
                if task.status == TaskStatus::InProgress
                    && !tmux_window_exists_for_task(session_name, &task.id)
                {
                    orphans.push((task.id.clone(), tag.to_string()));
                }
            }
        }
    }

    orphans
}

fn execute_round(
    storage: &Storage,
    tasks: &[TaskInfo],
    working_dir: &std::path::Path,
    session_name: &str,
    round_idx: usize,
    default_harness: Harness,
    event_writer: Option<&events::EventWriter>,
) -> Result<RoundState> {
    let mut round_state = RoundState::new(round_idx);

    for info in tasks.iter() {
        // Resolve agent config (harness, model, prompt) from task's agent_type
        let config =
            agent::resolve_agent_config(info.task, &info.tag, default_harness, None, working_dir);

        // Warn if agent type was specified but definition not found
        if info.task.agent_type.is_some() && !config.from_agent_def {
            println!(
                "    {} Agent '{}' not found, using defaults",
                "!".yellow(),
                info.task.agent_type.as_deref().unwrap_or("unknown")
            );
        }

        match terminal::spawn_terminal_with_harness_and_model(
            &info.task.id,
            &config.prompt,
            working_dir,
            session_name,
            config.harness,
            config.model.as_deref(),
        ) {
            Ok(window_index) => {
                println!(
                    "    {} Spawned: {} | {} [{}] {}:{}",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed(),
                    config.display_info().dimmed(),
                    session_name.dimmed(),
                    window_index.dimmed()
                );
                round_state.task_ids.push(info.task.id.clone());
                round_state.tags.push(info.tag.clone());

                // Emit spawn event
                if let Some(writer) = event_writer {
                    let _ = writer.log_spawned(&info.task.id);
                }

                if let Ok(mut phase) = storage.load_group(&info.tag) {
                    if let Some(task) = phase.get_task_mut(&info.task.id) {
                        task.set_status(TaskStatus::InProgress);
                        let _ = storage.update_group(&info.tag, &phase);
                    }
                }
            }
            Err(e) => {
                println!("    {} Failed: {} - {}", "✗".red(), info.task.id.red(), e);
                round_state.failures.push(info.task.id.clone());
            }
        }

        thread::sleep(Duration::from_millis(500));
    }

    Ok(round_state)
}

/// Execute a round using extension-based subprocesses (no tmux)
fn execute_round_extensions(
    storage: &Storage,
    tasks: &[TaskInfo],
    working_dir: &std::path::Path,
    round_idx: usize,
    default_harness: Harness,
) -> Result<RoundState> {
    // Convert TaskInfo to WaveAgent format
    let wave_agents: Vec<session::WaveAgent> = tasks
        .iter()
        .map(|info| session::WaveAgent::new(info.task.clone(), &info.tag))
        .collect();

    // Mark tasks as in-progress before spawning
    for info in tasks {
        if let Ok(mut phase) = storage.load_group(&info.tag) {
            if let Some(task) = phase.get_task_mut(&info.task.id) {
                task.set_status(TaskStatus::InProgress);
                let _ = storage.update_group(&info.tag, &phase);
            }
        }
    }

    // Run async execution using tokio runtime
    let handle = tokio::runtime::Handle::current();
    let result = handle.block_on(async {
        session::execute_wave_async(&wave_agents, working_dir, round_idx, default_harness).await
    })?;

    // Print results
    for agent_result in &result.agent_results {
        if agent_result.success {
            println!(
                "    {} Completed: {} ({}ms)",
                "✓".green(),
                agent_result.task_id.cyan(),
                agent_result.duration_ms
            );
        } else {
            println!(
                "    {} Failed: {} (exit code: {:?})",
                "✗".red(),
                agent_result.task_id.red(),
                agent_result.exit_code
            );
        }
    }

    // Update task statuses based on results
    for agent_result in &result.agent_results {
        // Find the task's tag
        if let Some(info) = tasks.iter().find(|t| t.task.id == agent_result.task_id) {
            if let Ok(mut phase) = storage.load_group(&info.tag) {
                if let Some(task) = phase.get_task_mut(&agent_result.task_id) {
                    // The agent itself should update the status via scud set-status
                    // But if it failed to start, mark it as failed
                    if !agent_result.success && agent_result.exit_code.is_none() {
                        task.set_status(TaskStatus::Failed);
                        let _ = storage.update_group(&info.tag, &phase);
                    }
                }
            }
        }
    }

    Ok(result.round_state)
}

/// Execute a round using OpenCode Server mode
fn execute_round_server(
    storage: &Storage,
    tasks: &[TaskInfo],
    working_dir: &std::path::Path,
    round_idx: usize,
) -> Result<RoundState> {
    use crate::opencode::AgentOrchestrator;
    use tokio::sync::mpsc;

    // Mark tasks as in-progress before spawning
    for info in tasks {
        if let Ok(mut phase) = storage.load_group(&info.tag) {
            if let Some(task) = phase.get_task_mut(&info.task.id) {
                task.set_status(TaskStatus::InProgress);
                let _ = storage.update_group(&info.tag, &phase);
            }
        }
    }

    // Run async execution using tokio runtime
    let handle = tokio::runtime::Handle::current();
    let result = handle.block_on(async {
        // Create event channel
        let (event_tx, _event_rx) = mpsc::channel(1000);

        // Create orchestrator
        let mut orchestrator = AgentOrchestrator::new(event_tx.clone()).await?;

        // Spawn all agents
        for info in tasks {
            let prompt = generate_server_prompt(info.task, &info.tag, working_dir);

            // Use xAI/Grok as default for server mode
            let model = Some(("xai", "grok-3"));

            match orchestrator.spawn_agent(info.task, &info.tag, &prompt, model).await {
                Ok(_) => {
                    println!(
                        "    {} Spawned: {} | {} [server/grok-3]",
                        "✓".green(),
                        info.task.id.cyan(),
                        info.task.title.dimmed(),
                    );
                }
                Err(e) => {
                    println!(
                        "    {} Failed to spawn {}: {}",
                        "✗".red(),
                        info.task.id,
                        e
                    );
                }
            }
        }

        // Drop our sender so we can detect when orchestrator is done
        drop(event_tx);

        // Collect results
        let results = orchestrator.wait_all().await;

        // Cleanup
        orchestrator.cleanup().await;

        Ok::<_, anyhow::Error>(results)
    })?;

    // Build round state from results
    let mut round_state = RoundState::new(round_idx);

    for agent_result in &result {
        if agent_result.success {
            println!(
                "    {} Completed: {} ({}ms)",
                "✓".green(),
                agent_result.task_id.cyan(),
                agent_result.duration_ms
            );
            round_state.task_ids.push(agent_result.task_id.clone());
        } else {
            println!(
                "    {} Failed: {} (exit code: {:?})",
                "✗".red(),
                agent_result.task_id.red(),
                agent_result.exit_code
            );
            round_state.failures.push(agent_result.task_id.clone());
        }
    }

    // Add tags for successful tasks
    for task_id in &round_state.task_ids {
        if let Some(info) = tasks.iter().find(|t| t.task.id == *task_id) {
            round_state.tags.push(info.tag.clone());
        }
    }

    // Update task statuses based on results
    for agent_result in &result {
        if let Some(info) = tasks.iter().find(|t| t.task.id == agent_result.task_id) {
            if let Ok(mut phase) = storage.load_group(&info.tag) {
                if let Some(task) = phase.get_task_mut(&agent_result.task_id) {
                    // If failed without exit code, mark as failed
                    if !agent_result.success && agent_result.exit_code.is_none() {
                        task.set_status(TaskStatus::Failed);
                        let _ = storage.update_group(&info.tag, &phase);
                    }
                }
            }
        }
    }

    Ok(round_state)
}

/// Generate prompt for server mode (similar to extensions but optimized for OpenCode)
fn generate_server_prompt(task: &Task, tag: &str, working_dir: &std::path::Path) -> String {
    let details = task
        .details
        .as_ref()
        .map(|d| format!("\n\n## Details\n\n{}", d))
        .unwrap_or_default();

    let test_strategy = task
        .test_strategy
        .as_ref()
        .map(|t| format!("\n\n## Test Strategy\n\n{}", t))
        .unwrap_or_default();

    format!(
        r#"You are working on task [{id}] in phase "{tag}".

## Task: {title}

{description}{details}{test_strategy}

## Instructions

1. Implement the task requirements
2. Test your changes
3. When complete, run: `scud set-status {id} done --tag {tag}`

Working directory: {working_dir}
"#,
        id = task.id,
        tag = tag,
        title = task.title,
        description = task.description,
        details = details,
        test_strategy = test_strategy,
        working_dir = working_dir.display(),
    )
}

fn wait_for_round_completion(
    storage: &Storage,
    tasks: &[TaskInfo],
    session_name: &str,
    stale_timeout: Option<Duration>,
    event_writer: Option<&events::EventWriter>,
) -> Result<()> {
    use std::collections::HashSet;
    use std::io::Write;
    use std::time::Instant;

    let task_ids: Vec<String> = tasks.iter().map(|t| t.task.id.clone()).collect();
    let task_tags: HashMap<String, String> = tasks
        .iter()
        .map(|t| (t.task.id.clone(), t.tag.clone()))
        .collect();

    let round_start = Instant::now();
    let mut completed_tasks: HashSet<String> = HashSet::new();
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut spin_idx: usize = 0;
    let mut last_orphan_check = Instant::now();

    // Track per-task content hashes for heartbeat (1c)
    let mut last_content_hashes: HashMap<String, u64> = HashMap::new();
    let mut last_activity: HashMap<String, Instant> = HashMap::new();
    for task_id in &task_ids {
        last_activity.insert(task_id.clone(), Instant::now());
    }

    loop {
        let mut still_running: Vec<String> = Vec::new();

        for task_id in &task_ids {
            if completed_tasks.contains(task_id) {
                continue;
            }

            if let Some(tag) = task_tags.get(task_id) {
                if let Ok(phase) = storage.load_group(tag) {
                    if let Some(task) = phase.get_task(task_id) {
                        if task.status == TaskStatus::InProgress
                            || task.status == TaskStatus::Pending
                        {
                            still_running.push(task_id.clone());
                        } else {
                            // Task just completed
                            completed_tasks.insert(task_id.clone());
                            let elapsed = round_start.elapsed().as_secs();
                            let status_icon = if task.status == TaskStatus::Done {
                                "✓".green()
                            } else {
                                "✗".red()
                            };
                            // Clear the status line, then print completion
                            print!("\r{}\r", " ".repeat(80));
                            println!(
                                "    {} {} completed ({}s)",
                                status_icon,
                                task_id.cyan(),
                                elapsed
                            );
                            // Emit completion event (1b)
                            if let Some(writer) = event_writer {
                                let success = task.status == TaskStatus::Done;
                                let _ = writer.log_completed(
                                    task_id,
                                    success,
                                    round_start.elapsed().as_millis() as u64,
                                );
                            }
                        }
                    }
                }
            }
        }

        if still_running.is_empty() {
            // Clear status line
            print!("\r{}\r", " ".repeat(80));
            let _ = std::io::stdout().flush();
            break;
        }

        // Periodic orphan detection (1e): every 30s, check if tmux windows still exist
        if last_orphan_check.elapsed() >= Duration::from_secs(30) {
            last_orphan_check = Instant::now();
            for task_id in &still_running {
                if !tmux_window_exists_for_task(session_name, task_id) {
                    // Agent died - tmux window gone but task still InProgress
                    print!("\r{}\r", " ".repeat(80));
                    println!(
                        "    {} {} agent died (tmux window gone), marking failed",
                        "⚠".yellow(),
                        task_id.cyan()
                    );
                    // Mark as Failed
                    if let Some(tag) = task_tags.get(task_id) {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Failed);
                                let _ = storage.update_group(tag, &phase);
                            }
                        }
                    }
                    completed_tasks.insert(task_id.clone());
                    // Emit failed event
                    if let Some(writer) = event_writer {
                        let event = events::AgentEvent::new(
                            &writer.session_id(),
                            task_id,
                            events::EventKind::Failed {
                                reason: "agent window disappeared".to_string(),
                            },
                        );
                        let _ = writer.write(&event);
                    }
                }
            }
        }

        // Stale task timeout (1d): check if any task exceeded the threshold
        if let Some(timeout) = stale_timeout {
            if round_start.elapsed() >= timeout {
                for task_id in &still_running {
                    // Cross-reference with tmux window existence
                    if !tmux_window_exists_for_task(session_name, task_id) {
                        print!("\r{}\r", " ".repeat(80));
                        println!(
                            "    {} {} stale (timeout + no tmux window), resetting to pending",
                            "⚠".yellow(),
                            task_id.cyan()
                        );
                        if let Some(tag) = task_tags.get(task_id) {
                            if let Ok(mut phase) = storage.load_group(tag) {
                                if let Some(task) = phase.get_task_mut(task_id) {
                                    task.set_status(TaskStatus::Pending);
                                    let _ = storage.update_group(tag, &phase);
                                }
                            }
                        }
                        completed_tasks.insert(task_id.clone());
                    }
                }
            }
        }

        // Heartbeat check (1c): poll tmux panes for activity
        for task_id in &still_running {
            if completed_tasks.contains(task_id) {
                continue;
            }
            let window_name = format!("task-{}", task_id);
            let window_target = format!("{}:{}", session_name, window_name);
            if let Ok(output) = std::process::Command::new("tmux")
                .args(["capture-pane", "-t", &window_target, "-p", "-S", "-20"])
                .output()
            {
                if output.status.success() {
                    let content = String::from_utf8_lossy(&output.stdout);
                    let hash = {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        content.hash(&mut hasher);
                        hasher.finish()
                    };
                    let prev_hash = last_content_hashes.get(task_id).copied();
                    if prev_hash.is_none() || prev_hash != Some(hash) {
                        last_activity.insert(task_id.clone(), Instant::now());
                    }
                    last_content_hashes.insert(task_id.clone(), hash);
                }
            }
        }

        // Print status line (1a)
        let elapsed = round_start.elapsed().as_secs();
        let spinner = spinner_chars[spin_idx % spinner_chars.len()];
        spin_idx += 1;

        // Check for idle agents
        let idle_agents: Vec<&String> = still_running
            .iter()
            .filter(|id| {
                !completed_tasks.contains(*id)
                    && last_activity
                        .get(*id)
                        .map(|t| t.elapsed() > Duration::from_secs(60))
                        .unwrap_or(false)
            })
            .collect();

        let running_count = still_running
            .iter()
            .filter(|id| !completed_tasks.contains(*id))
            .count();

        let status = if running_count <= 2 {
            let names: Vec<&str> = still_running
                .iter()
                .filter(|id| !completed_tasks.contains(*id))
                .map(|s| s.as_str())
                .collect();
            format!("{} running: {}", running_count, names.join(", "))
        } else {
            format!("{} running", running_count)
        };

        let idle_note = if !idle_agents.is_empty() {
            format!(
                " ({} idle >60s)",
                idle_agents.len()
            )
        } else {
            String::new()
        };

        print!(
            "\r    Waiting... [{}] {} {}s{}",
            status, spinner, elapsed, idle_note
        );
        let _ = std::io::stdout().flush();

        thread::sleep(Duration::from_secs(5));
    }

    Ok(())
}

fn collect_changed_files(
    working_dir: &std::path::Path,
    start_commit: Option<&str>,
) -> Result<Vec<String>> {
    use std::process::Command;

    // Construct the commit range: start_commit..HEAD or fallback to HEAD~1..HEAD
    let range = match start_commit {
        Some(commit) => format!("{}..HEAD", commit),
        None => "HEAD~1..HEAD".to_string(),
    };

    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["diff", "--name-only", &range])
        .output()?;

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    Ok(files)
}

fn run_dry_run(
    project_root: Option<PathBuf>,
    phase_tag: &str,
    round_size: usize,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let all_phases = storage.load_tasks()?;

    let waves = compute_waves_from_tasks(&all_phases, phase_tag, all_tags)?;

    println!("{}", "Execution Plan (dry-run)".yellow().bold());
    println!("{}", "═".repeat(50).yellow());
    println!();

    let mut total_tasks = 0;
    let mut total_rounds = 0;

    for (wave_idx, wave) in waves.iter().enumerate() {
        let rounds = wave.len().div_ceil(round_size);
        total_tasks += wave.len();
        total_rounds += rounds;

        println!(
            "{} {} - {} task(s), {} round(s)",
            "Wave".blue().bold(),
            wave_idx + 1,
            wave.len(),
            rounds
        );

        for (round_idx, chunk) in wave.chunks(round_size).enumerate() {
            println!("  {} {}:", "Round".yellow(), round_idx + 1);
            for info in chunk {
                println!(
                    "    {} {} | {}",
                    "○".white(),
                    info.task.id.cyan(),
                    info.task.title
                );
            }
        }
        println!();
    }

    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(30).blue());
    println!("  Total waves:  {}", waves.len());
    println!("  Total tasks:  {}", total_tasks);
    println!("  Total rounds: {}", total_rounds);

    if total_rounds > 0 {
        let speedup = total_tasks as f64 / total_rounds as f64;
        println!("  Speedup:      {}", format!("{:.1}x", speedup).green());
    }

    println!();
    println!("{}", "No agents spawned (dry-run mode).".yellow());

    Ok(())
}

// ============================================================================
// Review Agent Support
// ============================================================================

/// Result of a review operation
#[derive(Debug)]
pub struct ReviewResult {
    /// Whether all reviewed tasks passed
    pub all_passed: bool,
    /// Task IDs that need improvement
    pub tasks_to_improve: Vec<String>,
}

/// Spawn a reviewer agent and wait for it to complete
#[allow(dead_code)]
pub fn spawn_reviewer(
    working_dir: &std::path::Path,
    session_name: &str,
    summary: &WaveSummary,
    wave_tasks: &[(String, String)], // (id, title)
    review_all: bool,
) -> Result<ReviewResult> {
    println!();
    println!("  {} Spawning reviewer agent...", "Review:".magenta());

    let prompt = agent::generate_review_prompt(summary, wave_tasks, review_all);

    // Load reviewer agent definition for harness/model
    let agent_def = AgentDef::try_load("reviewer", working_dir).unwrap_or_else(|| {
        // Fallback: claude/opus
        AgentDef {
            agent: crate::agents::AgentMeta {
                name: "reviewer".to_string(),
                description: "Code reviewer".to_string(),
            },
            model: crate::agents::ModelConfig {
                harness: "claude".to_string(),
                model: Some("opus".to_string()),
            },
            prompt: Default::default(),
        }
    });

    let harness = agent_def.harness()?;
    let model = agent_def.model();

    // Spawn reviewer
    terminal::spawn_terminal_with_harness_and_model(
        &format!("review-wave-{}", summary.wave_number),
        &prompt,
        working_dir,
        session_name,
        harness,
        model,
    )?;

    println!(
        "    {} Reviewer spawned, waiting for completion...",
        "✓".green()
    );

    // Wait for reviewer to complete by watching for output file
    wait_for_review_completion(working_dir, summary.wave_number)
}

/// Wait for the review to complete by polling for marker file
fn wait_for_review_completion(
    working_dir: &std::path::Path,
    wave_number: usize,
) -> Result<ReviewResult> {
    let marker_path = working_dir
        .join(".scud")
        .join(format!("review-complete-{}", wave_number));

    let timeout = Duration::from_secs(1800); // 30 minute timeout
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            println!("    {} Review timed out after 30 minutes", "!".yellow());
            return Ok(ReviewResult {
                all_passed: true, // Assume pass on timeout
                tasks_to_improve: vec![],
            });
        }

        if marker_path.exists() {
            let content = std::fs::read_to_string(&marker_path)?;
            std::fs::remove_file(&marker_path)?; // Clean up

            let all_passed = content.contains("ALL_PASS");
            let tasks_to_improve = if content.contains("IMPROVE_TASKS:") {
                content
                    .lines()
                    .find(|l| l.starts_with("IMPROVE_TASKS:"))
                    .map(|l| {
                        l.strip_prefix("IMPROVE_TASKS:")
                            .unwrap_or("")
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            };

            println!("    {} Review complete", "✓".green());
            if !all_passed {
                println!(
                    "    {} Tasks needing improvement: {}",
                    "!".yellow(),
                    tasks_to_improve.join(", ")
                );
            }

            return Ok(ReviewResult {
                all_passed,
                tasks_to_improve,
            });
        }

        thread::sleep(Duration::from_secs(5));
    }
}

// ============================================================================
// Repair Loop Support
// ============================================================================

/// Run repair loop for failed validation
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn run_repair_loop(
    storage: &Storage,
    working_dir: &std::path::Path,
    session_name: &str,
    bp_config: &BackpressureConfig,
    wave_state: &WaveState,
    validation_result: &ValidationResult,
    max_attempts: usize,
) -> Result<bool> {
    let wave_tasks = wave_state.all_task_ids();
    let task_tags = wave_state.task_tags();

    println!();
    println!("  {} Analyzing failure attribution...", "Repair:".magenta());

    // Get the first failed command for attribution
    let failed_cmd = validation_result.results.iter().find(|r| !r.passed);
    let failed_cmd = match failed_cmd {
        Some(cmd) => cmd,
        None => return Ok(true), // No failures? Shouldn't happen
    };

    // Attribute the failure
    let attribution = attribute_failure(
        working_dir,
        &failed_cmd.stderr,
        &failed_cmd.stdout,
        &wave_tasks,
        wave_state.start_commit.as_deref(),
    )?;

    match attribution.confidence {
        AttributionConfidence::High => {
            println!(
                "    {} High confidence: task {} responsible",
                "✓".green(),
                attribution.responsible_tasks.join(", ")
            );
        }
        AttributionConfidence::Medium => {
            println!(
                "    {} Medium confidence: tasks {} may be responsible",
                "~".yellow(),
                attribution.responsible_tasks.join(", ")
            );
        }
        AttributionConfidence::Low => {
            println!(
                "    {} Low confidence: cannot determine specific task",
                "!".red()
            );
        }
    }

    // Mark cleared tasks as done
    for task_id in &attribution.cleared_tasks {
        if let Some(tag) = task_tags
            .iter()
            .find(|(id, _)| id == task_id)
            .map(|(_, t)| t)
        {
            if let Ok(mut phase) = storage.load_group(tag) {
                if let Some(task) = phase.get_task_mut(task_id) {
                    task.set_status(TaskStatus::Done);
                    let _ = storage.update_group(tag, &phase);
                    println!("    {} Cleared: {} (not responsible)", "✓".green(), task_id);
                }
            }
        }
    }

    // Attempt repairs on responsible tasks
    for attempt in 1..=max_attempts {
        println!();
        println!(
            "  {} Repair attempt {}/{}",
            "Repair:".magenta(),
            attempt,
            max_attempts
        );

        let mut all_repaired = true;

        for task_id in &attribution.responsible_tasks {
            // Find task details
            let (task, _tag) = match find_task_with_tag(storage, task_id, &task_tags) {
                Some(t) => t,
                None => continue,
            };

            // Get files changed by this task
            let task_files = crate::attribution::get_task_changed_files(
                working_dir,
                task_id,
                wave_state.start_commit.as_deref(),
            )?;

            // Parse error files
            let error_files: Vec<String> =
                crate::attribution::parse_error_locations(&failed_cmd.stderr, &failed_cmd.stdout)
                    .into_iter()
                    .map(|(f, _)| f)
                    .collect();

            // Generate repair prompt
            let prompt = agent::generate_repair_prompt(
                task_id,
                &task.title,
                &failed_cmd.command,
                &format!("{}\n{}", failed_cmd.stderr, failed_cmd.stdout),
                &task_files.into_iter().collect::<Vec<_>>(),
                &error_files,
            );

            // Spawn repairer
            spawn_repairer(working_dir, session_name, task_id, &prompt)?;

            // Wait for repair completion
            if !wait_for_repair_completion_task(working_dir, task_id)? {
                all_repaired = false;
            }
        }

        if !all_repaired {
            println!("    {} Some repairs failed or blocked", "!".yellow());
            continue;
        }

        // Re-run validation
        println!();
        println!("  {} Re-running validation...", "Validate:".magenta());
        let new_result = crate::backpressure::run_validation(working_dir, bp_config)?;

        if new_result.all_passed {
            println!("    {} Validation passed after repair!", "✓".green());

            // Mark all responsible tasks as done
            for task_id in &attribution.responsible_tasks {
                if let Some(tag) = task_tags
                    .iter()
                    .find(|(id, _)| id == task_id)
                    .map(|(_, t)| t)
                {
                    if let Ok(mut phase) = storage.load_group(tag) {
                        if let Some(task) = phase.get_task_mut(task_id) {
                            task.set_status(TaskStatus::Done);
                            let _ = storage.update_group(tag, &phase);
                        }
                    }
                }
            }

            return Ok(true);
        }

        println!(
            "    {} Validation still failing, will retry...",
            "!".yellow()
        );
    }

    // Max attempts reached - mark responsible tasks as failed
    println!();
    println!("  {} Max repair attempts reached", "!".red());

    for task_id in &attribution.responsible_tasks {
        if let Some(tag) = task_tags
            .iter()
            .find(|(id, _)| id == task_id)
            .map(|(_, t)| t)
        {
            if let Ok(mut phase) = storage.load_group(tag) {
                if let Some(task) = phase.get_task_mut(task_id) {
                    task.set_status(TaskStatus::Failed);
                    let _ = storage.update_group(tag, &phase);
                    println!("    {} Marked failed: {}", "✗".red(), task_id);
                }
            }
        }
    }

    Ok(false)
}

/// Spawn a repairer agent for a specific task
fn spawn_repairer(
    working_dir: &std::path::Path,
    session_name: &str,
    task_id: &str,
    prompt: &str,
) -> Result<()> {
    // Load repairer agent definition
    let agent_def = AgentDef::try_load("repairer", working_dir).unwrap_or_else(|| AgentDef {
        agent: crate::agents::AgentMeta {
            name: "repairer".to_string(),
            description: "Repair agent".to_string(),
        },
        model: crate::agents::ModelConfig {
            harness: "claude".to_string(),
            model: Some("opus".to_string()),
        },
        prompt: Default::default(),
    });

    let harness = agent_def.harness()?;
    let model = agent_def.model();

    terminal::spawn_terminal_with_harness_and_model(
        &format!("repair-{}", task_id),
        prompt,
        working_dir,
        session_name,
        harness,
        model,
    )?;

    println!("    {} Spawned repairer for {}", "✓".green(), task_id);
    Ok(())
}

/// Wait for a repair to complete by polling for marker file
fn wait_for_repair_completion_task(working_dir: &std::path::Path, task_id: &str) -> Result<bool> {
    let marker_path = working_dir
        .join(".scud")
        .join(format!("repair-complete-{}", task_id));

    let timeout = Duration::from_secs(1800); // 30 minute timeout
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            println!("    {} Repair timed out for {}", "!".yellow(), task_id);
            return Ok(false);
        }

        if marker_path.exists() {
            let content = std::fs::read_to_string(&marker_path)?;
            std::fs::remove_file(&marker_path)?;

            let success = content.contains("SUCCESS");
            if success {
                println!("    {} Repair completed for {}", "✓".green(), task_id);
            } else {
                println!("    {} Repair blocked for {}", "!".yellow(), task_id);
            }

            return Ok(success);
        }

        thread::sleep(Duration::from_secs(5));
    }
}

/// Find a task by ID along with its tag
fn find_task_with_tag(
    storage: &Storage,
    task_id: &str,
    task_tags: &[(String, String)],
) -> Option<(Task, String)> {
    let tag = task_tags.iter().find(|(id, _)| id == task_id)?.1.clone();
    let phase = storage.load_group(&tag).ok()?;
    let task = phase.get_task(task_id)?.clone();
    Some((task, tag))
}
