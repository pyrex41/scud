# `scud ralph` Command Implementation Plan

## Overview

Implement a `scud ralph` command that runs an autonomous coding loop with fresh context per iteration. Unlike `scud swarm` which processes tasks in parallel waves, Ralph processes one task at a time, completing it fully (with backpressure validation) before moving to the next.

This implements the "Ralph" methodology: fresh context each iteration, one task per loop, backpressure for self-correction.

## Current State Analysis

**Existing infrastructure we can reuse:**
- `src/commands/spawn/terminal.rs` - tmux spawning, harness abstraction (Claude/OpenCode)
- `src/commands/spawn/agent.rs` - Agent config resolution, prompt generation
- `src/backpressure.rs` - Validation commands execution
- `src/storage/mod.rs` - Task storage and status management
- `src/commands/swarm/session.rs` - Session state persistence patterns

**Key difference from swarm:**
- Swarm: parallel waves with multiple tasks → validate → repair → next wave
- Ralph: single task → validate → next task (fresh agent context each iteration)

## Desired End State

After implementation:
1. `scud ralph` runs an autonomous loop that:
   - Picks the next available task (respecting DAG dependencies)
   - Spawns a fresh agent (new tmux window) to complete it
   - Waits for agent completion
   - Runs backpressure validation
   - If validation fails, spawns repair agent (fresh context)
   - Moves to next task when validation passes
   - Continues until all tasks done or `--max-iterations` reached

2. Supports both SCUD tasks and PRD mode (IMPLEMENTATION_PLAN.md parsing)
3. Session state persists for resume capability
4. Works with existing `scud monitor` TUI

## What We're NOT Doing

- **No new TUI** - Ralph uses existing `scud monitor`
- **No AGENTS.md integration** (Phase 4 of original plan) - defer to later
- **No PRD generation** (Phase 6 of original plan) - defer to later
- **No hybrid mode** - defer spec file loading to later
- **No custom prompts from `.scud/ralph/`** - defer to later, use default prompts only

## Implementation Approach

Minimal viable implementation: single file `src/commands/ralph.rs` with ~400-500 lines, reusing existing infrastructure heavily.

---

## Phase 1: Core Command Structure

### Overview
Add the `scud ralph` command with basic CLI parsing and loop skeleton.

### Changes Required:

#### 1.1 Add command module

**File**: `src/commands/mod.rs`
**Changes**: Add `pub mod ralph;`

```rust
// After line 40 (pub mod swarm;)
// Ralph mode (sequential iteration with fresh context)
pub mod ralph;
```

#### 1.2 Add CLI command definition

**File**: `src/main.rs`
**Changes**: Add `Ralph` variant to `Commands` enum (after `Swarm` at ~line 731)

```rust
    /// Run Ralph mode - sequential iteration loop with fresh context per task
    Ralph {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Task source: scud (SCUD tasks) or prd (IMPLEMENTATION_PLAN.md)
        #[arg(long, default_value = "auto")]
        source: String,

        /// Maximum iterations (0 = unlimited)
        #[arg(short = 'n', long, default_value = "0")]
        max_iterations: usize,

        /// Skip backpressure validation
        #[arg(long)]
        no_validate: bool,

        /// Disable automatic repair on validation failure
        #[arg(long)]
        no_repair: bool,

        /// Maximum repair attempts per task (default: 3)
        #[arg(long, default_value = "3")]
        max_repair_attempts: usize,

        /// AI harness: claude, opencode
        #[arg(short = 'H', long, default_value = "claude")]
        harness: String,

        /// Model to use with harness
        #[arg(short = 'M', long)]
        model: Option<String>,

        /// Session name (default: ralph-<tag>)
        #[arg(long)]
        session: Option<String>,

        /// Show plan without executing
        #[arg(long)]
        dry_run: bool,

        /// Resume from previous session
        #[arg(long)]
        resume: bool,

        /// Push to git after each successful iteration
        #[arg(long)]
        push: bool,
    },
```

#### 1.3 Add command handler in main.rs

**File**: `src/main.rs`
**Changes**: Add match arm after `Commands::Swarm` handler (~line 1097)

```rust
        Commands::Ralph {
            tag,
            source,
            max_iterations,
            no_validate,
            no_repair,
            max_repair_attempts,
            harness,
            model,
            session,
            dry_run,
            resume,
            push,
        } => commands::ralph::run(
            cli.project,
            tag.as_deref(),
            &source,
            max_iterations,
            no_validate,
            no_repair,
            max_repair_attempts,
            &harness,
            model.as_deref(),
            session,
            dry_run,
            resume,
            push,
        ),
```

#### 1.4 Create ralph command module

**File**: `src/commands/ralph.rs`
**Changes**: New file with core loop structure

```rust
//! Ralph mode - Sequential iteration loop with fresh context per task
//!
//! Implements the Ralph methodology:
//! 1. Fresh context each iteration - spawns new agent each time
//! 2. One task per loop - focus, complete, validate
//! 3. Backpressure - tests/lint force self-correction
//! 4. File-based state - session persists for resume

use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::backpressure::{BackpressureConfig, run_validation};
use crate::commands::helpers::resolve_group_tag;
use crate::commands::spawn::agent;
use crate::commands::spawn::terminal::{self, Harness};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Task source mode
#[derive(Debug, Clone, PartialEq)]
pub enum TaskSource {
    /// Use SCUD task storage
    Scud,
    /// Use IMPLEMENTATION_PLAN.md
    Prd,
    /// Auto-detect based on project structure
    Auto,
}

impl TaskSource {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "scud" => Ok(TaskSource::Scud),
            "prd" | "plan" => Ok(TaskSource::Prd),
            "auto" => Ok(TaskSource::Auto),
            other => anyhow::bail!("Unknown source: '{}'. Use: scud, prd, auto", other),
        }
    }
}

/// Ralph session state (persists between runs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RalphSession {
    pub name: String,
    pub tag: String,
    pub source: String,
    pub iteration: u32,
    pub current_task_id: Option<String>,
    pub completed_tasks: Vec<String>,
    pub failed_tasks: Vec<String>,
}

impl RalphSession {
    pub fn new(name: &str, tag: &str, source: &str) -> Self {
        Self {
            name: name.to_string(),
            tag: tag.to_string(),
            source: source.to_string(),
            iteration: 0,
            current_task_id: None,
            completed_tasks: Vec::new(),
            failed_tasks: Vec::new(),
        }
    }

    pub fn save(&self, project_root: Option<&PathBuf>) -> Result<()> {
        let root = project_root
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let path = root.join(".scud").join("ralph-session.json");
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load(project_root: Option<&PathBuf>) -> Result<Option<Self>> {
        let root = project_root
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let path = root.join(".scud").join("ralph-session.json");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&content)?))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    source: &str,
    max_iterations: usize,
    no_validate: bool,
    no_repair: bool,
    max_repair_attempts: usize,
    harness_arg: &str,
    model: Option<&str>,
    session_name: Option<String>,
    dry_run: bool,
    resume: bool,
    push: bool,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Check tmux is available
    terminal::check_tmux_available()?;

    // Resolve task source
    let task_source = TaskSource::parse(source)?;
    let effective_source = match task_source {
        TaskSource::Auto => detect_task_source(&project_root),
        other => other,
    };

    // Resolve tag (only relevant for SCUD source)
    let effective_tag = resolve_group_tag(&storage, tag, true)?;

    // Parse harness
    let harness = Harness::parse(harness_arg)?;
    terminal::find_harness_binary(harness)?;

    // Generate session name
    let session_name = session_name.unwrap_or_else(|| format!("ralph-{}", effective_tag));

    // Load or create session
    let mut session = if resume {
        RalphSession::load(&project_root)?
            .ok_or_else(|| anyhow::anyhow!("No Ralph session to resume"))?
    } else {
        RalphSession::new(&session_name, &effective_tag, source)
    };

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
    println!("{:<20} {}", "Source:".dimmed(), format!("{:?}", effective_source).cyan());
    println!("{:<20} {}", "Terminal:".dimmed(), "tmux".cyan());
    println!("{:<20} {}", "Harness:".dimmed(), harness.name().cyan());
    if let Some(m) = model {
        println!("{:<20} {}", "Model:".dimmed(), m.cyan());
    }
    println!(
        "{:<20} {}",
        "Validation:".dimmed(),
        if no_validate { "skip".yellow() } else { "enabled".green() }
    );
    println!(
        "{:<20} {}",
        "Repair:".dimmed(),
        if no_repair { "disabled".yellow() } else { format!("up to {} attempts", max_repair_attempts).green() }
    );
    if max_iterations > 0 {
        println!("{:<20} {}", "Max iterations:".dimmed(), max_iterations.to_string().cyan());
    }
    println!();

    if dry_run {
        return run_dry_run(&storage, &effective_tag, &effective_source);
    }

    // Main Ralph loop
    run_ralph_loop(
        &storage,
        &mut session,
        &effective_tag,
        &effective_source,
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

fn detect_task_source(project_root: &Option<PathBuf>) -> TaskSource {
    let root = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Check for SCUD initialization
    if root.join(".scud").join("tasks").exists() {
        return TaskSource::Scud;
    }

    // Check for IMPLEMENTATION_PLAN.md
    if root.join("IMPLEMENTATION_PLAN.md").exists() {
        return TaskSource::Prd;
    }

    // Default to SCUD
    TaskSource::Scud
}

fn run_dry_run(storage: &Storage, tag: &str, source: &TaskSource) -> Result<()> {
    println!("{}", "Dry run - showing execution plan:".yellow());
    println!();

    match source {
        TaskSource::Scud => {
            let phases = storage.load_tasks()?;
            let phase = phases.iter().find(|p| p.tag == tag);

            if let Some(phase) = phase {
                let pending: Vec<_> = phase.tasks.iter()
                    .filter(|t| t.status == TaskStatus::Pending)
                    .collect();

                println!("Tasks to process ({}):", pending.len());
                for (i, task) in pending.iter().enumerate() {
                    println!("  {}. {} - {}", i + 1, task.id.cyan(), task.title);
                }
            } else {
                println!("No tasks found for tag: {}", tag);
            }
        }
        TaskSource::Prd => {
            println!("PRD mode: would parse IMPLEMENTATION_PLAN.md");
            // TODO: Implement PRD parsing
        }
        TaskSource::Auto => unreachable!(),
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_ralph_loop(
    storage: &Storage,
    session: &mut RalphSession,
    tag: &str,
    source: &TaskSource,
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
    loop {
        // Check iteration limit
        if max_iterations > 0 && session.iteration >= max_iterations as u32 {
            println!("{}", format!("Reached max iterations: {}", max_iterations).yellow());
            break;
        }

        session.iteration += 1;
        println!();
        println!("{}", format!("═══════════════ ITERATION {} ═══════════════", session.iteration).cyan().bold());

        // Get next task
        let task = match source {
            TaskSource::Scud => get_next_scud_task(storage, tag)?,
            TaskSource::Prd => get_next_prd_task(project_root)?,
            TaskSource::Auto => unreachable!(),
        };

        let Some(task_info) = task else {
            println!("{}", "No more tasks available. Ralph complete!".green().bold());
            break;
        };

        println!("Task: {} - {}", task_info.id.cyan(), task_info.title);
        session.current_task_id = Some(task_info.id.clone());
        session.save(project_root)?;

        // Mark task in-progress (SCUD mode only)
        if *source == TaskSource::Scud {
            mark_task_status(storage, tag, &task_info.id, TaskStatus::InProgress)?;
        }

        // Spawn agent with fresh context
        let window_name = format!("task-{}", task_info.id);
        spawn_ralph_agent(
            &task_info,
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
                    if *source == TaskSource::Scud {
                        mark_task_status(storage, tag, &task_info.id, TaskStatus::Failed)?;
                    }
                    session.failed_tasks.push(task_info.id.clone());
                    session.save(project_root)?;
                    continue;
                }

                // Attempt repairs
                let repaired = run_repair_loop(
                    &task_info,
                    max_repair_attempts,
                    harness,
                    model,
                    session_name,
                    working_dir,
                    bp_config,
                    &validation,
                )?;

                if !repaired {
                    println!("  {} Repair failed after {} attempts", "✗".red(), max_repair_attempts);
                    if *source == TaskSource::Scud {
                        mark_task_status(storage, tag, &task_info.id, TaskStatus::Failed)?;
                    }
                    session.failed_tasks.push(task_info.id.clone());
                    session.save(project_root)?;
                    continue;
                }
            }
            println!("  {} Validation passed", "✓".green());
        }

        // Mark task complete
        if *source == TaskSource::Scud {
            mark_task_status(storage, tag, &task_info.id, TaskStatus::Done)?;
        } else if *source == TaskSource::Prd {
            mark_prd_task_complete(project_root, &task_info.id)?;
        }
        session.completed_tasks.push(task_info.id.clone());
        session.current_task_id = None;
        session.save(project_root)?;

        // Git push if enabled
        if push {
            println!("  {} Pushing to remote...", "→".dimmed());
            if let Err(e) = git_push(working_dir) {
                println!("  {} Push failed: {}", "!".yellow(), e);
            } else {
                println!("  {} Pushed", "✓".green());
            }
        }

        println!("  {} Task {} complete", "✓".green().bold(), task_info.id);
    }

    // Final summary
    println!();
    println!("{}", "═══════════════ SUMMARY ═══════════════".cyan().bold());
    println!("  Iterations: {}", session.iteration);
    println!("  Completed:  {} tasks", session.completed_tasks.len());
    println!("  Failed:     {} tasks", session.failed_tasks.len());

    Ok(())
}

/// Task info from any source
struct TaskInfo {
    id: String,
    title: String,
    description: String,
}

fn get_next_scud_task(storage: &Storage, tag: &str) -> Result<Option<TaskInfo>> {
    let phases = storage.load_tasks()?;
    let phase = phases.iter().find(|p| p.tag == tag);

    let Some(phase) = phase else {
        return Ok(None);
    };

    // Find first pending task with no blocking dependencies
    for task in &phase.tasks {
        if task.status != TaskStatus::Pending {
            continue;
        }

        // Check if all dependencies are done
        let deps_satisfied = task.depends_on.iter().all(|dep_id| {
            phase.tasks.iter()
                .find(|t| t.id == *dep_id)
                .map(|t| t.status == TaskStatus::Done)
                .unwrap_or(true) // Treat missing deps as satisfied
        });

        if deps_satisfied {
            return Ok(Some(TaskInfo {
                id: task.id.clone(),
                title: task.title.clone(),
                description: task.description.clone().unwrap_or_default(),
            }));
        }
    }

    Ok(None)
}

fn get_next_prd_task(project_root: &Option<PathBuf>) -> Result<Option<TaskInfo>> {
    let root = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let plan_path = root.join("IMPLEMENTATION_PLAN.md");
    if !plan_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&plan_path)?;

    // Parse markdown to find unchecked items: - [ ] Task description
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [ ]") {
            let description = trimmed.strip_prefix("- [ ]").unwrap_or("").trim();
            return Ok(Some(TaskInfo {
                id: format!("line-{}", line_num + 1),
                title: description.to_string(),
                description: description.to_string(),
            }));
        }
    }

    Ok(None)
}

fn mark_task_status(storage: &Storage, tag: &str, task_id: &str, status: TaskStatus) -> Result<()> {
    storage.update_task_status(tag, task_id, status)
}

fn mark_prd_task_complete(project_root: &Option<PathBuf>, task_id: &str) -> Result<()> {
    let root = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let plan_path = root.join("IMPLEMENTATION_PLAN.md");
    let content = std::fs::read_to_string(&plan_path)?;

    // Parse line number from task_id (format: "line-N")
    let line_num: usize = task_id
        .strip_prefix("line-")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid PRD task id: {}", task_id))?;

    // Update the specific line
    let mut lines: Vec<&str> = content.lines().collect();
    if line_num > 0 && line_num <= lines.len() {
        let line = lines[line_num - 1];
        if line.trim().starts_with("- [ ]") {
            let updated = line.replace("- [ ]", "- [x]");
            lines[line_num - 1] = Box::leak(updated.into_boxed_str()); // Safe for this use case
        }
    }

    std::fs::write(&plan_path, lines.join("\n"))?;
    Ok(())
}

fn spawn_ralph_agent(
    task: &TaskInfo,
    harness: Harness,
    model: Option<&str>,
    session_name: &str,
    window_name: &str,
    working_dir: &PathBuf,
) -> Result<()> {
    // Generate prompt
    let prompt = generate_ralph_prompt(task);

    // Write prompt to temp file
    let prompt_file = std::env::temp_dir().join(format!("ralph-prompt-{}.txt", task.id));
    std::fs::write(&prompt_file, &prompt)?;

    // Find harness binary
    let binary_path = terminal::find_harness_binary(harness)?;

    // Generate command
    let command = harness.command(binary_path, &prompt_file, model);

    // Spawn in tmux
    terminal::spawn_in_tmux(session_name, window_name, &command, working_dir)?;

    Ok(())
}

fn generate_ralph_prompt(task: &TaskInfo) -> String {
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
        task.id, task.title, task.description, task.title
    )
}

fn wait_for_agent_completion(session_name: &str, window_name: &str) -> Result<()> {
    use std::time::Duration;
    use std::thread;

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

fn run_repair_loop(
    task: &TaskInfo,
    max_attempts: usize,
    harness: Harness,
    model: Option<&str>,
    session_name: &str,
    working_dir: &PathBuf,
    bp_config: &BackpressureConfig,
    initial_failure: &crate::backpressure::ValidationResult,
) -> Result<bool> {
    use crate::backpressure::run_validation;

    let mut last_failure = initial_failure.clone();

    for attempt in 1..=max_attempts {
        println!("  {} Repair attempt {}/{}...", "→".dimmed(), attempt, max_attempts);

        // Generate repair prompt
        let repair_prompt = generate_repair_prompt(task, &last_failure);

        // Write prompt to temp file
        let prompt_file = std::env::temp_dir().join(format!("ralph-repair-{}-{}.txt", task.id, attempt));
        std::fs::write(&prompt_file, &repair_prompt)?;

        // Find harness binary
        let binary_path = terminal::find_harness_binary(harness)?;

        // Generate command
        let command = harness.command(binary_path, &prompt_file, model);

        // Spawn repair agent
        let window_name = format!("repair-{}-{}", task.id, attempt);
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

fn generate_repair_prompt(task: &TaskInfo, failure: &crate::backpressure::ValidationResult) -> String {
    let failures: Vec<String> = failure.results.iter()
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
        task.id, task.title,
        failures.join("\n\n"),
        task.id
    )
}

fn git_push(working_dir: &PathBuf) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["push"])
        .current_dir(working_dir)
        .output()?;

    if !output.status.success() {
        anyhow::bail!("git push failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud-cli`
- [x] Tests pass: `cargo test -p scud-cli`
- [x] Linting passes: `cargo clippy -p scud-cli` (warnings only, no errors)

#### Manual Verification:
- [ ] `scud ralph --help` shows command options
- [ ] `scud ralph --dry-run` shows execution plan

**Implementation Note**: After completing this phase and all automated verification passes, pause for manual testing before proceeding.

---

## Phase 2: tmux Helper Function

### Overview
Add the `spawn_in_tmux` helper function that the ralph command uses.

### Changes Required:

#### 2.1 Add spawn_in_tmux function

**File**: `src/commands/spawn/terminal.rs`
**Changes**: Add a new public function for simple tmux spawning

```rust
/// Spawn a command in a tmux window (simpler than spawn_tmux which does more setup)
pub fn spawn_in_tmux(
    session_name: &str,
    window_name: &str,
    command: &str,
    working_dir: &Path,
) -> Result<()> {
    // Check if session exists, create if not
    let session_exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !session_exists {
        // Create new session with a control window
        Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s", session_name,
                "-n", "ctrl",
                "-c", &working_dir.to_string_lossy(),
            ])
            .output()
            .context("Failed to create tmux session")?;
    }

    // Create new window for this task
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-t", session_name,
            "-n", window_name,
            "-c", &working_dir.to_string_lossy(),
            "-P", "-F", "#{window_index}",
        ])
        .output()
        .context("Failed to create tmux window")?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to create tmux window: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let window_index = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Send the command to the window
    let send_result = Command::new("tmux")
        .args([
            "send-keys",
            "-t", &format!("{}:{}", session_name, window_index),
            command,
            "Enter",
        ])
        .output()
        .context("Failed to send command to tmux window")?;

    if !send_result.status.success() {
        anyhow::bail!(
            "Failed to send command: {}",
            String::from_utf8_lossy(&send_result.stderr)
        );
    }

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud-cli`
- [x] Tests pass: `cargo test -p scud-cli`

#### Manual Verification:
- [ ] `scud ralph` successfully spawns agents in tmux

---

## Phase 3: Storage Helper Method

### Overview
Add the `update_task_status` helper method to Storage if it doesn't exist.

### Changes Required:

#### 3.1 Check and add update_task_status

**File**: `src/storage/mod.rs`
**Changes**: Add method if not present

```rust
/// Update a single task's status
pub fn update_task_status(&self, tag: &str, task_id: &str, status: TaskStatus) -> Result<()> {
    let mut phases = self.load_tasks()?;

    let phase = phases.iter_mut()
        .find(|p| p.tag == tag)
        .ok_or_else(|| anyhow::anyhow!("Phase not found: {}", tag))?;

    let task = phase.tasks.iter_mut()
        .find(|t| t.id == task_id)
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

    task.status = status;

    self.save_phase(phase)?;
    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [x] Code compiles: `cargo build -p scud-cli`

---

## Testing Strategy

### Unit Tests:
- Session serialization/deserialization
- PRD parsing (unchecked/checked items)
- Task source detection

### Integration Tests:
- Full loop with mock agent (using `echo` instead of real harness)
- Validation pass/fail handling
- Session resume

### Manual Testing Steps:
1. Initialize a test project with a few tasks: `scud init && scud ai parse-prd test.md`
2. Run `scud ralph --dry-run` to verify task listing
3. Run `scud ralph --max-iterations 1` to execute one iteration
4. Verify task status changes to `in-progress` then `done`
5. Test `scud ralph --resume` after interrupting
6. Test with `--source prd` on a project with IMPLEMENTATION_PLAN.md

## References

- Ralph methodology: https://ghuntley.com/ralph/
- SCUD swarm implementation: `src/commands/swarm/mod.rs`
- Backpressure system: `src/backpressure.rs`
- Spawn/terminal: `src/commands/spawn/terminal.rs`
- Original implementation plan: `../ralph/IMPLEMENTATION_PLAN.md`
