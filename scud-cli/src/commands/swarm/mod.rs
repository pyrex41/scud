//! Swarm mode - Wave-based parallel execution with backpressure
//!
//! Executes tasks in dependency-order waves using parallel agents.
//! After each wave, runs backpressure validation (build, lint, test).
//!
//! Flow:
//! 1. [Optional] Research phase: Smart model analyzes tasks, may expand complex ones
//! 2. Build phase: Fast models execute tasks in parallel rounds
//! 3. Validate phase: Runs backpressure tests (compile, lint, test), smart model fixes issues
//! 4. Repeat for next wave
//!
//! Usage:
//!   scud swarm --tag <tag>                 # Full mode with research + validation
//!   scud swarm --tag <tag> --no-research   # Skip research, use tasks as-is
//!   scud swarm --tag <tag> --no-validate   # Skip backpressure validation

pub mod backpressure;
pub mod session;

use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::commands::helpers::{flatten_all_tasks, resolve_group_tag};
use crate::commands::spawn::agent;
use crate::commands::spawn::hooks;
use crate::commands::spawn::terminal::{self, parse_terminal, Terminal};
use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

use self::backpressure::BackpressureConfig;
use self::session::{RoundState, SwarmSession, WaveState, WaveSummary};

/// Main entry point for the swarm command
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    round_size: usize,
    all_tags: bool,
    terminal_arg: &str,
    dry_run: bool,
    session_name: Option<String>,
    no_research: bool,
    no_validate: bool,
) -> Result<()> {
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
    let session_name = session_name.unwrap_or_else(|| format!("swarm-{}", phase_tag));

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Load backpressure configuration
    let bp_config = BackpressureConfig::load(project_root.as_ref())?;

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
    println!("{:<20} {}", "Terminal:".dimmed(), terminal.name().cyan());

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
    let mut swarm_session = SwarmSession::new(
        &session_name,
        &phase_tag,
        terminal.name(),
        &working_dir.to_string_lossy(),
        round_size,
    );

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

            let in_progress_count = count_in_progress(&all_phases, &phase_tag, all_tags);
            if in_progress_count > 0 {
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

            // Spawn agents for this round
            // Note: Agents self-orient using scud CLI commands (scud list, scud show, etc.)
            let round_state = execute_round(
                &storage,
                round_tasks,
                &terminal,
                &working_dir,
                &session_name,
                round_idx,
            )?;

            wave_state.rounds.push(round_state);

            // Wait for round completion
            println!("    Waiting for round completion...");
            wait_for_round_completion(&storage, round_tasks)?;
            println!("    {} Round {} complete", "✓".green(), round_idx + 1);
        }

        // === PHASE 3: VALIDATE (optional) ===
        if !no_validate && !bp_config.commands.is_empty() {
            println!();
            println!("  {} Running backpressure checks...", "Validate:".magenta());

            let validation_result = backpressure::run_validation(&working_dir, &bp_config)?;

            if validation_result.all_passed {
                println!("    {} All checks passed", "✓".green());
            } else {
                println!("    {} Some checks failed:", "!".yellow());
                for failure in &validation_result.failures {
                    println!("      - {}", failure.red());
                }

                // Mark all tasks from this wave as Failed
                // Next wave can see them via: scud list --status failed
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
            }

            wave_state.validation = Some(validation_result);
        }

        // Generate wave summary (just what was done - not context accumulation)
        let summary = WaveSummary {
            wave_number,
            tasks_completed: wave_state.all_task_ids(),
            files_changed: collect_changed_files(&working_dir).unwrap_or_default(),
        };
        wave_state.summary = Some(summary);

        // Save session state
        swarm_session.waves.push(wave_state);
        session::save_session(project_root.as_ref(), &swarm_session)?;

        wave_number += 1;
    }

    // Final summary
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

fn execute_round(
    storage: &Storage,
    tasks: &[TaskInfo],
    terminal: &Terminal,
    working_dir: &std::path::Path,
    session_name: &str,
    round_idx: usize,
) -> Result<RoundState> {
    let mut round_state = RoundState::new(round_idx);

    for info in tasks.iter() {
        // Generate prompt - agents self-orient using scud CLI commands
        let prompt = agent::generate_prompt(info.task, &info.tag);

        match terminal::spawn_terminal(terminal, &info.task.id, &prompt, working_dir, session_name)
        {
            Ok(()) => {
                println!(
                    "    {} Spawned: {} | {}",
                    "✓".green(),
                    info.task.id.cyan(),
                    info.task.title.dimmed()
                );
                round_state.task_ids.push(info.task.id.clone());
                round_state.tags.push(info.tag.clone());

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

fn wait_for_round_completion(storage: &Storage, tasks: &[TaskInfo]) -> Result<()> {
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
                        if task.status == TaskStatus::InProgress
                            || task.status == TaskStatus::Pending
                        {
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

fn collect_changed_files(working_dir: &std::path::Path) -> Result<Vec<String>> {
    use std::process::Command;

    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["diff", "--name-only", "HEAD~1..HEAD"])
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
