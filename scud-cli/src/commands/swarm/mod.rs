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
pub mod publisher;
pub mod session;
pub mod transcript;
pub mod zmq_client;

/// Re-export backpressure module for backward compatibility.
///
/// The canonical location is now [`crate::backpressure`], but this re-export
/// maintains the old path `scud::commands::swarm::backpressure` for existing code.
pub use crate::backpressure;

use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::commands::helpers::resolve_group_tag;
use crate::commands::spawn::agent;
use crate::commands::spawn::headless::{self, store::SessionStatus, StreamStore};
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
pub async fn run(
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
    idle_timeout_minutes: u64,
    no_publish_events: bool,
    pause_flag: Option<Arc<AtomicBool>>,
    stop_flag: Option<Arc<AtomicBool>>,
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
    let (working_dir, is_salvo_worktree, main_project_root) = if !no_worktree && !all_tags {
        if let Some(tag_name) = tag {
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
            SwarmMode::Headless => "headless (waves)".green(),
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
        SwarmMode::Headless => "headless",
    };
    let mut swarm_session = SwarmSession::new(
        &session_name,
        &phase_tag,
        terminal_mode,
        &working_dir.to_string_lossy(),
        round_size,
    );

    // EventWriter will be created later in the function

    // Compute stale timeout duration
    let stale_timeout = stale_timeout_minutes.map(|m| Duration::from_secs(m * 60));

    // Create EventWriter for SQLite event logging (Phase 1b)
    // Include ZMQ publisher if not disabled
    let event_writer = events::EventWriter::new_with_zmq(
        &working_dir,
        &session_name,
        !no_publish_events,
    ).ok();

    // Create status tracking for control commands
    let status_state = Arc::new(std::sync::Mutex::new(crate::commands::swarm::publisher::SwarmStatus {
        state: "running".to_string(),
        current_wave: 0,
        total_waves: 0,
        tasks_completed: 0,
        tasks_total: 0,
    }));

    // Start heartbeat background task for connection liveness detection
    let heartbeat_handle = if event_writer.is_some() {
        let working_dir = working_dir.clone();
        let session_name = session_name.clone();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            let writer = match events::EventWriter::new(&working_dir, &session_name) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to create heartbeat EventWriter: {}", e);
                    return;
                }
            };

            while !stop_flag_clone.load(Ordering::Relaxed) {
                if let Err(e) = writer.log_heartbeat() {
                    eprintln!("Heartbeat logging error: {}", e);
                }
                thread::sleep(Duration::from_secs(5));
            }
        });

        Some((handle, stop_flag))
    } else {
        None
    };



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

         // Stop heartbeat background task
         if let Some((handle, stop_flag)) = heartbeat_handle {
             stop_flag.store(true, Ordering::Relaxed);
             let _ = handle.join();
         }

         return Ok(());
     }

    // === WAVE MODE: Batch execution ===
    // Main loop: execute waves until all tasks done
    let mut wave_number = 1;
    loop {
        // Check pause/stop flags from control commands
        if let Some(ref pause_flag) = pause_flag {
            while pause_flag.load(Ordering::SeqCst) {
                // Handle any pending control requests while paused
                #[cfg(feature = "zmq")]
                if let Some(ref writer) = &event_writer {
                    if let Some(zmq_publisher) = writer.zmq_publisher() {
                        let _ = zmq_publisher.handle_control_request(
                            pause_flag,
                            stop_flag.as_ref().unwrap_or(&Arc::new(AtomicBool::new(false))),
                            &|| status_state.lock().unwrap().clone(),
                        );
                    }
                }

                std::thread::sleep(Duration::from_millis(100));
                if let Some(ref stop_flag) = stop_flag {
                    if stop_flag.load(Ordering::SeqCst) {
                        println!();
                        println!("{}", "Swarm stopped by control command".yellow());
                        break;
                    }
                }
            }
        }

        if let Some(ref stop_flag) = stop_flag {
            if stop_flag.load(Ordering::SeqCst) {
                println!();
                println!("{}", "Swarm stopped by control command".yellow());

                // Update status
                {
                    let mut status = status_state.lock().unwrap();
                    status.state = "stopped".to_string();
                }

                break;
            }
        }

        // Load fresh task state (must reload each iteration to see completed tasks)
        let all_phases = storage.load_tasks()?;

        // Compute waves from current state
        let waves = compute_waves_from_tasks(&all_phases, &phase_tag, all_tags)?;

        // Update status for control commands
        {
            let mut status = status_state.lock().unwrap();
            status.current_wave = wave_number;
            status.total_waves = waves.len();
            status.tasks_total = waves.iter().map(|w| w.len()).sum();
            status.tasks_completed = all_phases.values()
                .flat_map(|phase| &phase.tasks)
                .filter(|task| matches!(task.status, TaskStatus::Done))
                .count();
            status.state = if pause_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::SeqCst))
            {
                "paused".to_string()
            } else {
                "running".to_string()
            };
        }

        if waves.is_empty() {
            println!();
            println!("{}", "All tasks complete!".green().bold());

            // Update status
            {
                let mut status = status_state.lock().unwrap();
                status.state = "completed".to_string();
            }

            // Publish swarm completed event
            if let Some(ref writer) = &event_writer {
                let _ = writer.publish_event(&publisher::ZmqEvent::SwarmCompleted {
                    success: true,
                });
                let _ = writer.log_swarm_completed(true);
            }

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
                        idle_timeout_minutes,
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
                    )
                    .await?
                }
                SwarmMode::Server => {
                    // Server mode: use OpenCode Server for agent orchestration
                    execute_round_server(
                        &storage,
                        round_tasks,
                        &working_dir,
                        round_idx,
                    )
                    .await?
                }
                SwarmMode::Headless => {
                    // Headless mode: use spawn's headless infrastructure for streaming JSON
                    execute_round_headless(
                        &storage,
                        round_tasks,
                        &working_dir,
                        round_idx,
                        harness,
                        event_writer.as_ref(),
                    )
                    .await?
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

    // Publish swarm completed event (successful completion)
    if let Some(ref writer) = &event_writer {
        let _ = writer.publish_event(&publisher::ZmqEvent::SwarmCompleted {
            success: true,
        });
        let _ = writer.log_swarm_completed(true);
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

    // Stop heartbeat background task
    if let Some((handle, stop_flag)) = heartbeat_handle {
        stop_flag.store(true, Ordering::Relaxed);
        if let Err(e) = handle.join() {
            eprintln!("Heartbeat thread join error: {:?}", e);
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

fn find_task_title_tag(
    phases: &HashMap<String, crate::models::phase::Phase>,
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
async fn execute_round_extensions<'a>(
    storage: &Storage,
    tasks: &[TaskInfo<'a>],
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

    let result = session::execute_wave_async(&wave_agents, working_dir, round_idx, default_harness).await?;

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
async fn execute_round_server<'a>(
    storage: &Storage,
    tasks: &[TaskInfo<'a>],
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
        // Create event channel
        let (event_tx, _event_rx) = mpsc::channel(1000);

        // Create orchestrator
        let mut orchestrator = AgentOrchestrator::new(event_tx.clone()).await?;

        // Resolve model from config
        let config_path = working_dir.join(".scud").join("config.toml");
        let config = crate::config::Config::load(&config_path).unwrap_or_default();
        let model_str = config.swarm_model().to_string();
        let provider_str = config.swarm.direct_api_provider.clone();

        // Spawn all agents
        for info in tasks {
            let prompt = generate_server_prompt(info.task, &info.tag, working_dir);

            let model = Some((provider_str.as_str(), model_str.as_str()));

            match orchestrator.spawn_agent(info.task, &info.tag, &prompt, model).await {
                Ok(_) => {
                    println!(
                        "    {} Spawned: {} | {} [server/{}/{}]",
                        "✓".green(),
                        info.task.id.cyan(),
                        info.task.title.dimmed(),
                        provider_str,
                        model_str,
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

    let result = results;

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

/// Execute a round using spawn's headless streaming infrastructure (no tmux)
///
/// Uses the headless runner from `spawn::headless` to capture streaming JSON
/// events from Claude Code or OpenCode agents without requiring tmux.
async fn execute_round_headless(
    storage: &Storage,
    tasks: &[TaskInfo<'_>],
    working_dir: &std::path::Path,
    round_idx: usize,
    default_harness: Harness,
    event_writer: Option<&events::EventWriter>,
) -> Result<RoundState> {
    use crate::commands::attach::{save_session_metadata, SessionMetadata};

    let mut round_state = RoundState::new(round_idx);

    // Create stream store for this round
    let store = StreamStore::new();

    // Mark tasks as in-progress and spawn agents
    for info in tasks {
        // Resolve agent config (harness, model, prompt) from task's agent_type
        let config =
            agent::resolve_agent_config(info.task, &info.tag, default_harness, None, working_dir);

        // Create session in store
        store.create_session(&info.task.id, &info.tag);

        // Create a runner for this task's specific harness
        let runner = match headless::create_runner(config.harness) {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "    {} Failed to create runner for {}: {}",
                    "✗".red(),
                    info.task.id.red(),
                    e
                );
                round_state.failures.push(info.task.id.clone());
                continue;
            }
        };

        // Spawn headless session
        let spawn_result = runner
            .start(
                &info.task.id,
                &config.prompt,
                working_dir,
                config.model.as_deref(),
            )
            .await;

        match spawn_result {
            Ok(mut session_handle) => {
                // Set PID in store
                if let Some(pid) = session_handle.pid() {
                    store.set_pid(&info.task.id, pid);
                }

                println!(
                    "    {} Spawned (headless): {} | {} [{}]",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed(),
                    config.display_info().dimmed(),
                );

                round_state.task_ids.push(info.task.id.clone());
                round_state.tags.push(info.tag.clone());

                // Emit spawn event
                if let Some(writer) = event_writer {
                    let _ = writer.log_spawned(&info.task.id);
                }

                // Mark task as in-progress
                if let Ok(mut phase) = storage.load_group(&info.tag) {
                    if let Some(task) = phase.get_task_mut(&info.task.id) {
                        task.set_status(TaskStatus::InProgress);
                        let _ = storage.update_group(&info.tag, &phase);
                    }
                }

                // Spawn background task to collect events
                let store_clone = store.clone();
                let task_id = info.task.id.clone();
                let tag = info.tag.clone();
                let working_dir_clone = working_dir.to_path_buf();
                let harness_name = default_harness.name().to_string();

                tokio::spawn(async move {
                    while let Some(event) = session_handle.events.recv().await {
                        // Check for session ID assignment
                        if let headless::StreamEventKind::SessionAssigned { ref session_id } =
                            event.kind
                        {
                            store_clone.set_session_id(&task_id, session_id);

                            // Save session metadata for `scud attach` continuation
                            let metadata =
                                SessionMetadata::new(&task_id, session_id, &tag, &harness_name);
                            let _ = save_session_metadata(&working_dir_clone, &metadata);
                        }

                        store_clone.push_event(&task_id, event);
                    }

                    // Wait for process to complete
                    let _ = session_handle.wait().await;
                });
            }
            Err(e) => {
                println!(
                    "    {} Failed (headless): {} - {}",
                    "✗".red(),
                    info.task.id.red(),
                    e
                );
                round_state.failures.push(info.task.id.clone());

                // Record error in store
                store.push_event(&info.task.id, headless::StreamEvent::error(e.to_string()));
            }
        }

        // Small delay between spawns to avoid overwhelming the system
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Wait for all tasks to complete by polling the store
    let max_wait = Duration::from_secs(3600); // 1 hour max
    let start = std::time::Instant::now();
    let total_tasks = round_state.task_ids.len();
    let mut poll_count = 0u32;
    // Track how many display lines we printed last time (for ANSI overwrite)
    let mut prev_display_lines = 0usize;

    loop {
        // Fast initial polling (2s for first 5 polls), then slow (5s)
        let poll_interval = if poll_count < 5 {
            Duration::from_secs(2)
        } else {
            Duration::from_secs(5)
        };
        poll_count += 1;

        // Small initial delay to let agents start producing output
        if poll_count == 1 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        let active_tasks = store.active_tasks();
        let active_count = active_tasks.len();
        if active_count == 0 {
            break;
        }

        if start.elapsed() > max_wait {
            println!(
                "    {} Timeout waiting for {} tasks",
                "!".yellow(),
                active_count
            );
            break;
        }

        // Move cursor up to overwrite previous display (if any)
        if prev_display_lines > 0 {
            // Move up N lines and clear each one
            for _ in 0..prev_display_lines {
                print!("\x1b[A\x1b[2K");
            }
        }

        // Build display lines, then print them all
        let mut display = Vec::new();
        let completed = total_tasks - active_count;
        let elapsed = start.elapsed().as_secs();

        display.push(format!(
            "\n    ─── {} {}/{} done ({} active) · {}s ───",
            "▶".blue(),
            completed,
            total_tasks,
            active_count,
            format_duration(elapsed),
        ));

        // Show ALL tasks in this round with their current status
        for task_id in &round_state.task_ids {
            let status = store.get_status(task_id);
            let elapsed_task = store.get_elapsed_secs(task_id).unwrap_or(0);
            let stats = store
                .session_stats(task_id)
                .map(|(events, _)| format!("{}ev", events))
                .unwrap_or_default();

            let (icon, status_str) = match &status {
                Some(SessionStatus::Completed) => ("✓".green(), "done".green()),
                Some(SessionStatus::Failed) => ("✗".red(), "fail".red()),
                Some(SessionStatus::Running) => ("⟳".blue(), "run".blue()),
                Some(SessionStatus::Starting) => ("…".yellow(), "init".yellow()),
                None => ("?".dimmed(), "?".dimmed()),
            };

            // For active tasks, show last tool activity; for done tasks, just show stats
            let detail = match &status {
                Some(SessionStatus::Running) | Some(SessionStatus::Starting) => {
                    // Prefer tool line, fall back to last output line
                    let tool_line = store.get_last_tool_line(task_id);
                    let activity = tool_line
                        .or_else(|| {
                            store.get_output(task_id, 1).into_iter().next()
                        })
                        .unwrap_or_default();
                    let trimmed = if activity.len() > 60 {
                        format!("{}…", &activity[..59])
                    } else {
                        activity
                    };
                    if trimmed.is_empty() {
                        format!("{}s {}", format_duration(elapsed_task), stats)
                    } else {
                        format!("{}s {} {}", format_duration(elapsed_task), stats, trimmed)
                    }
                }
                _ => {
                    format!("{}s {}", format_duration(elapsed_task), stats)
                }
            };

            display.push(format!(
                "      {} {:>5} [{}] {}",
                icon,
                task_id.cyan(),
                status_str,
                detail.dimmed(),
            ));
        }

        // Print and track line count for next overwrite
        prev_display_lines = display.len();
        for line in &display {
            println!("{}", line);
        }

        tokio::time::sleep(poll_interval).await;
    }

    // Final summary (printed once, not overwritten)
    let elapsed = start.elapsed().as_secs();
    let successes = round_state.task_ids.iter()
        .filter(|id| matches!(store.get_status(id), Some(SessionStatus::Completed)))
        .count();
    let failures = round_state.task_ids.iter()
        .filter(|id| matches!(store.get_status(id), Some(SessionStatus::Failed)))
        .count();
    println!(
        "\n    ─── Round complete: {} ok, {} failed, {} total in {}s ───",
        format!("{}", successes).green(),
        format!("{}", failures).red(),
        total_tasks,
        format_duration(elapsed),
    );
    // Show details for failed tasks only
    for task_id in &round_state.task_ids {
        if matches!(store.get_status(task_id), Some(SessionStatus::Failed)) {
            println!("      {} {} — last output:", "✗".red(), task_id.red());
            let output = store.get_all_output(task_id);
            for line in output.iter().rev().take(5).rev() {
                let trimmed = if line.len() > 80 {
                    format!("{}…", &line[..79])
                } else {
                    line.clone()
                };
                if !trimmed.is_empty() {
                    println!("        {}", trimmed.dimmed());
                }
            }
        }
    }

    // Emit completion events
    for task_id in &round_state.task_ids {
        if let Some(writer) = event_writer {
            let _ = writer.log_completed(task_id, true, start.elapsed().as_millis() as u64);
        }
    }

    Ok(round_state)
}

/// Format seconds into compact human-readable duration (e.g., "45s", "2m30s", "1h05m")
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}", secs)
    } else if secs < 3600 {
        format!("{}m{:02}", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
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
    idle_timeout_minutes: u64,
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
                            writer.session_id(),
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

        // Idle timeout failure detection: if agent has been idle AND shows shell prompt
        let idle_timeout = Duration::from_secs(idle_timeout_minutes * 60);
        for task_id in &still_running {
            if completed_tasks.contains(task_id) {
                continue;
            }

            // Check if this task has been idle long enough
            let is_idle_timeout = last_activity
                .get(task_id)
                .map(|t| t.elapsed() > idle_timeout)
                .unwrap_or(false);

            if !is_idle_timeout {
                continue;
            }

            // Check if the pane shows a shell prompt (process exited)
            let window_name = format!("task-{}", task_id);
            if terminal::tmux_pane_shows_prompt(session_name, &window_name) {
                print!("\r{}\r", " ".repeat(80));
                println!(
                    "    {} {} agent idle with shell prompt, marking failed",
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
                        writer.session_id(),
                        task_id,
                        events::EventKind::Failed {
                            reason: "agent idle with shell prompt (process crashed)".to_string(),
                        },
                    );
                    let _ = writer.write(&event);
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

    // Collect task info for batch repair
    let mut task_infos: Vec<(String, String, Vec<String>)> = Vec::new();
    for task_id in &attribution.responsible_tasks {
        let (task, _tag) = match find_task_with_tag(storage, task_id, &task_tags) {
            Some(t) => t,
            None => continue,
        };

        let task_files = crate::attribution::get_task_changed_files(
            working_dir,
            task_id,
            wave_state.start_commit.as_deref(),
        )
        .unwrap_or_default()
        .into_iter()
        .collect();

        task_infos.push((task_id.clone(), task.title.clone(), task_files));
    }

    // Parse error locations for the prompt
    let error_locations: Vec<(String, Option<u32>)> =
        crate::attribution::parse_error_locations(&failed_cmd.stderr, &failed_cmd.stdout);

    // Attempt batch repairs
    for attempt in 1..=max_attempts {
        println!();
        println!(
            "  {} Batch repair attempt {}/{}",
            "Repair:".magenta(),
            attempt,
            max_attempts
        );

        // Generate batch repair prompt
        let prompt = agent::generate_batch_repair_prompt(
            &task_infos,
            &failed_cmd.command,
            &format!("{}\n{}", failed_cmd.stderr, failed_cmd.stdout),
            &error_locations,
        );

        // Spawn single batch repairer
        spawn_batch_repairer(working_dir, session_name, &prompt)?;

        // Wait for batch repair completion
        let repair_result = wait_for_batch_repair_completion(working_dir)?;

        match repair_result {
            BatchRepairResult::Success(fixed_tasks) => {
                // Re-run validation
                println!();
                println!("  {} Re-running validation...", "Validate:".magenta());
                let new_result = crate::backpressure::run_validation(working_dir, bp_config)?;

                if new_result.all_passed {
                    println!("    {} Validation passed after batch repair!", "✓".green());

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
                    "    {} Validation still failing (fixed: {}), will retry...",
                    "!".yellow(),
                    fixed_tasks.join(", ")
                );
            }
            BatchRepairResult::Partial(fixed, blocked) => {
                // Mark fixed tasks as done, blocked as blocked
                for task_id in &fixed {
                    if let Some(tag) = task_tags
                        .iter()
                        .find(|(id, _)| id == task_id)
                        .map(|(_, t)| t)
                    {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Done);
                                let _ = storage.update_group(tag, &phase);
                                println!("    {} Fixed: {}", "✓".green(), task_id);
                            }
                        }
                    }
                }
                for task_id in &blocked {
                    if let Some(tag) = task_tags
                        .iter()
                        .find(|(id, _)| id == task_id)
                        .map(|(_, t)| t)
                    {
                        if let Ok(mut phase) = storage.load_group(tag) {
                            if let Some(task) = phase.get_task_mut(task_id) {
                                task.set_status(TaskStatus::Blocked);
                                let _ = storage.update_group(tag, &phase);
                                println!("    {} Blocked: {}", "!".yellow(), task_id);
                            }
                        }
                    }
                }

                // Re-run validation
                let new_result = crate::backpressure::run_validation(working_dir, bp_config)?;
                if new_result.all_passed {
                    println!("    {} Validation passed!", "✓".green());
                    return Ok(true);
                }
            }
            BatchRepairResult::Blocked(reason) => {
                println!("    {} Batch repair blocked: {}", "!".red(), reason);
            }
            BatchRepairResult::Timeout => {
                println!("    {} Batch repair timed out", "!".yellow());
            }
        }
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

/// Spawn a repairer agent for a specific task (kept as fallback for single-task scenarios)
#[allow(dead_code)]
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

/// Wait for a repair to complete by polling for marker file (kept as fallback for single-task scenarios)
#[allow(dead_code)]
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

/// Result of a batch repair attempt
enum BatchRepairResult {
    Success(Vec<String>),              // All fixed, list of task IDs
    Partial(Vec<String>, Vec<String>), // Some fixed, some blocked
    Blocked(String),                   // Completely blocked with reason
    Timeout,                           // Timed out
}

/// Spawn a batch repairer agent
fn spawn_batch_repairer(
    working_dir: &std::path::Path,
    session_name: &str,
    prompt: &str,
) -> Result<()> {
    // Load repairer agent definition
    let agent_def = AgentDef::try_load("repairer", working_dir).unwrap_or_else(|| AgentDef {
        agent: crate::agents::AgentMeta {
            name: "batch-repairer".to_string(),
            description: "Batch repair agent".to_string(),
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
        "batch-repair",
        prompt,
        working_dir,
        session_name,
        harness,
        model,
    )?;

    println!("    {} Spawned batch repairer", "✓".green());
    Ok(())
}

/// Wait for batch repair to complete by polling for marker file
fn wait_for_batch_repair_completion(working_dir: &std::path::Path) -> Result<BatchRepairResult> {
    let marker_path = working_dir.join(".scud").join("batch-repair-complete");

    let timeout = Duration::from_secs(2700); // 45 minute timeout for batch
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            return Ok(BatchRepairResult::Timeout);
        }

        if marker_path.exists() {
            let content = std::fs::read_to_string(&marker_path)?;
            let _ = std::fs::remove_file(&marker_path); // Clean up

            // Parse the marker file
            if content.contains("SUCCESS") {
                let fixed = parse_task_list(&content, "FIXED_TASKS:");
                return Ok(BatchRepairResult::Success(fixed));
            } else if content.contains("PARTIAL") {
                let fixed = parse_task_list(&content, "FIXED_TASKS:");
                let blocked = parse_task_list(&content, "BLOCKED_TASKS:");
                return Ok(BatchRepairResult::Partial(fixed, blocked));
            } else if content.contains("BLOCKED") {
                let reason = content
                    .lines()
                    .find(|l| l.starts_with("REASON:"))
                    .map(|l| l.trim_start_matches("REASON:").trim().to_string())
                    .unwrap_or_else(|| "Unknown reason".to_string());
                return Ok(BatchRepairResult::Blocked(reason));
            }
        }

        thread::sleep(Duration::from_secs(5));
    }
}

/// Parse comma-separated task list from marker file line
fn parse_task_list(content: &str, prefix: &str) -> Vec<String> {
    content
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(|l| {
            l.trim_start_matches(prefix)
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
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
