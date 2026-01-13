//! Ralph Wiggum mode - Wave-based execution with smart model review
//!
//! This module implements the "Ralph Wiggum" method for AI-driven development:
//! 1. Tasks are executed in waves based on the dependency DAG
//! 2. Within each wave, tasks are split into rounds (configurable size, default 5)
//! 3. Fast models execute the tasks in parallel
//! 4. After all tasks in a wave complete, a smart model reviews:
//!    - The task prompt
//!    - Diff of files changed
//!    - Summary from the task agent
//! 5. Smart model fixes any issues and generates a "handoff" prompt
//! 6. Handoff context carries into the next wave
//!
//! Usage:
//!   scud ralph --tag <tag>              # Normal mode (no review)
//!   scud ralph --tag <tag> --review     # With smart model review after each wave

pub mod handoff;
pub mod review;
pub mod session;

use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::commands::spawn::hooks;
use crate::commands::spawn::terminal::{self, parse_terminal, Terminal};
use crate::commands::spawn::agent;
use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

use self::handoff::Handoff;
use self::review::ReviewResult;
use self::session::{RalphSession, RoundState, WaveState};

/// Main entry point for the ralph command
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    round_size: usize,
    review: bool,
    all_tags: bool,
    terminal_arg: &str,
    dry_run: bool,
    session_name: Option<String>,
    model: Option<&str>,
) -> Result<()> {
    // Validate round_size
    if round_size == 0 {
        anyhow::bail!("--round-size must be at least 1");
    }

    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Determine phase tag
    let phase_tag = if all_tags {
        "all".to_string()
    } else {
        resolve_group_tag(&storage, tag, true)?
    };

    // Detect terminal
    let terminal = parse_terminal(terminal_arg)?;
    terminal::check_terminal_available(&terminal)?;

    // Generate session name
    let session_name = session_name.unwrap_or_else(|| format!("ralph-{}", phase_tag));

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Display header
    println!("{}", "SCUD Ralph Wiggum Mode".cyan().bold());
    println!("{}", "═".repeat(50));
    println!(
        "{:<20} {}",
        "Tag:".dimmed(),
        phase_tag.green()
    );
    println!(
        "{:<20} {}",
        "Round size:".dimmed(),
        round_size.to_string().cyan()
    );
    println!(
        "{:<20} {}",
        "Smart review:".dimmed(),
        if review { "enabled".green() } else { "disabled".yellow() }
    );
    println!(
        "{:<20} {}",
        "Terminal:".dimmed(),
        terminal.name().cyan()
    );
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

    // Initialize ralph session
    let mut ralph_session = RalphSession::new(
        &session_name,
        &phase_tag,
        terminal.name(),
        &working_dir.to_string_lossy(),
        round_size,
        review,
    );

    // Load any existing handoff from previous session
    let mut handoff = Handoff::load(project_root.as_ref(), &phase_tag).unwrap_or_default();

    // Main loop: execute waves until all tasks done
    let mut wave_number = 1;
    loop {
        // Load fresh task state
        let all_phases = storage.load_tasks()?;
        let all_tasks_flat = flatten_all_tasks(&all_phases);

        // Compute waves from current state
        let waves = compute_waves_from_tasks(&all_phases, &all_tasks_flat, &phase_tag, all_tags)?;

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

            // Check if there are in-progress tasks we're waiting for
            let in_progress_count = count_in_progress(&all_phases, &phase_tag, all_tags);
            if in_progress_count > 0 {
                println!(
                    "Waiting for {} in-progress task(s) to complete...",
                    in_progress_count.to_string().cyan()
                );
                thread::sleep(Duration::from_secs(10));
                continue;
            } else {
                // Might be blocked tasks
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

        // Split wave into rounds and execute
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

            // Spawn agents for this round
            let round_state = execute_round(
                project_root.as_ref(),
                &storage,
                round_tasks,
                &terminal,
                &working_dir,
                &session_name,
                &handoff,
                round_idx,
            )?;

            wave_state.rounds.push(round_state);

            // Wait for round completion
            println!("    Waiting for round completion...");
            wait_for_round_completion(&storage, round_tasks, &phase_tag)?;

            println!("    {} Round {} complete", "✓".green(), round_idx + 1);
        }

        // After wave completion, optionally run smart review
        if review {
            println!();
            println!(
                "{} Running smart model review...",
                "Review:".magenta().bold()
            );

            let review_result = review::review_wave(
                project_root.clone(),
                &wave_state,
                &all_phases,
                &phase_tag,
                model,
            )
            .await_sync()?;

            // Display review summary
            display_review_result(&review_result);

            // Update handoff with review insights
            handoff = handoff::generate_handoff(
                &review_result,
                &wave_state,
                wave_number,
            );

            // Save handoff for next wave / session
            handoff.save(project_root.as_ref(), &phase_tag)?;

            ralph_session.reviews.push(review_result);
        }

        // Save session state
        ralph_session.waves.push(wave_state);
        session::save_session(project_root.as_ref(), &ralph_session)?;

        wave_number += 1;
    }

    // Final summary
    println!();
    println!("{}", "Ralph Session Summary".blue().bold());
    println!("{}", "═".repeat(40).blue());
    println!(
        "  Waves completed: {}",
        ralph_session.waves.len().to_string().green()
    );

    let total_tasks: usize = ralph_session.waves.iter()
        .flat_map(|w| &w.rounds)
        .map(|r| r.task_ids.len())
        .sum();
    println!("  Tasks executed: {}", total_tasks.to_string().green());

    if review {
        let fixes: usize = ralph_session.reviews.iter()
            .map(|r| r.fixes_applied.len())
            .sum();
        println!("  Fixes applied: {}", fixes.to_string().cyan());
    }

    Ok(())
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
    _all_tasks_flat: &[&Task],
    phase_tag: &str,
    all_tags: bool,
) -> Result<Vec<Vec<TaskInfo<'a>>>> {
    use std::collections::{HashSet};

    // Collect actionable tasks
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

    // Build dependency graph and compute waves using Kahn's algorithm
    let task_ids: HashSet<String> = actionable.iter().map(|t| t.task.id.clone()).collect();

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for info in &actionable {
        in_degree.entry(info.task.id.clone()).or_insert(0);

        for dep in &info.task.dependencies {
            if task_ids.contains(dep) {
                *in_degree.entry(info.task.id.clone()).or_insert(0) += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(info.task.id.clone());
            }
        }
    }

    // Compute waves
    let mut waves: Vec<Vec<TaskInfo<'a>>> = Vec::new();
    let mut remaining = in_degree.clone();

    while !remaining.is_empty() {
        let ready: Vec<String> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        if ready.is_empty() {
            // Circular dependency - break to avoid infinite loop
            break;
        }

        // Collect ready tasks
        let wave: Vec<TaskInfo<'a>> = actionable
            .iter()
            .filter(|t| ready.contains(&t.task.id))
            .cloned()
            .collect();

        // Remove from remaining and update dependents
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

/// Check if a task is actionable (pending, not expanded, dependencies met)
fn is_task_actionable(task: &Task, phase: &Phase) -> bool {
    // Must be pending
    if task.status != TaskStatus::Pending {
        return false;
    }

    // Must not be expanded
    if task.is_expanded() {
        return false;
    }

    // If subtask, parent must be expanded
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

/// Count in-progress tasks
fn count_in_progress(
    all_phases: &HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> usize {
    let mut count = 0;

    let tags: Vec<&String> = if all_tags {
        all_phases.keys().collect()
    } else {
        all_phases.keys().filter(|t| t.as_str() == phase_tag).collect()
    };

    for tag in tags {
        if let Some(phase) = all_phases.get(tag) {
            count += phase.tasks.iter()
                .filter(|t| t.status == TaskStatus::InProgress)
                .count();
        }
    }

    count
}

/// Execute a single round of tasks
fn execute_round(
    _project_root: Option<&PathBuf>,
    storage: &Storage,
    tasks: &[TaskInfo],
    terminal: &Terminal,
    working_dir: &std::path::Path,
    session_name: &str,
    handoff: &Handoff,
    round_idx: usize,
) -> Result<RoundState> {
    let mut round_state = RoundState::new(round_idx);

    for info in tasks.iter() {
        // Generate prompt with handoff context
        let mut prompt = agent::generate_prompt(info.task, &info.tag);

        // Inject handoff context if present
        if !handoff.context.is_empty() {
            prompt = format!(
                "{}\n\n## Context from Previous Wave\n{}\n",
                prompt, handoff.context
            );
        }

        // Spawn terminal
        match terminal::spawn_terminal(terminal, &info.task.id, &prompt, working_dir, session_name) {
            Ok(()) => {
                println!(
                    "    {} Spawned: {} | {}",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed()
                );
                round_state.task_ids.push(info.task.id.clone());
                round_state.tags.push(info.tag.clone());

                // Mark as in-progress
                if let Ok(mut phase) = storage.load_group(&info.tag) {
                    if let Some(task) = phase.get_task_mut(&info.task.id) {
                        task.set_status(TaskStatus::InProgress);
                        let _ = storage.update_group(&info.tag, &phase);
                    }
                }
            }
            Err(e) => {
                println!(
                    "    {} Failed: {} - {}",
                    "✗".red(),
                    info.task.id.red(),
                    e
                );
                round_state.failures.push(info.task.id.clone());
            }
        }

        // Small delay between spawns
        thread::sleep(Duration::from_millis(500));
    }

    Ok(round_state)
}

/// Wait for all tasks in a round to complete
fn wait_for_round_completion(
    storage: &Storage,
    tasks: &[TaskInfo],
    _phase_tag: &str,
) -> Result<()> {
    let task_ids: Vec<String> = tasks.iter().map(|t| t.task.id.clone()).collect();
    let task_tags: HashMap<String, String> = tasks
        .iter()
        .map(|t| (t.task.id.clone(), t.tag.clone()))
        .collect();

    loop {
        let mut all_done = true;

        for task_id in &task_ids {
            if let Some(tag) = task_tags.get(task_id) {
                if let Ok(phase) = storage.load_group(tag) {
                    if let Some(task) = phase.get_task(task_id) {
                        if task.status == TaskStatus::InProgress || task.status == TaskStatus::Pending {
                            all_done = false;
                            break;
                        }
                    }
                }
            }
        }

        if all_done {
            break;
        }

        thread::sleep(Duration::from_secs(5));
    }

    Ok(())
}

/// Display review result summary
fn display_review_result(result: &ReviewResult) {
    println!();
    println!("  {} Tasks reviewed: {}", "│".dimmed(), result.tasks_reviewed);

    if !result.issues_found.is_empty() {
        println!("  {} Issues found:", "│".dimmed());
        for issue in &result.issues_found {
            println!("  {}   - {}", "│".dimmed(), issue.yellow());
        }
    }

    if !result.fixes_applied.is_empty() {
        println!("  {} Fixes applied:", "│".dimmed());
        for fix in &result.fixes_applied {
            println!("  {}   - {}", "│".dimmed(), fix.green());
        }
    }

    if !result.recommendations.is_empty() {
        println!("  {} Recommendations:", "│".dimmed());
        for rec in &result.recommendations {
            println!("  {}   - {}", "│".dimmed(), rec.cyan());
        }
    }
}

/// Run dry-run mode showing execution plan
fn run_dry_run(
    project_root: Option<PathBuf>,
    phase_tag: &str,
    round_size: usize,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);
    let all_phases = storage.load_tasks()?;
    let all_tasks_flat = flatten_all_tasks(&all_phases);

    let waves = compute_waves_from_tasks(&all_phases, &all_tasks_flat, phase_tag, all_tags)?;

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

/// Helper trait to run async functions synchronously
trait AwaitSync {
    type Output;
    fn await_sync(self) -> Self::Output;
}

impl<F: std::future::Future> AwaitSync for F {
    type Output = F::Output;
    fn await_sync(self) -> Self::Output {
        tokio::runtime::Handle::current().block_on(self)
    }
}
