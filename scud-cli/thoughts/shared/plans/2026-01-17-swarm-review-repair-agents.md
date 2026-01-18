# Swarm Review & Repair Agents Implementation Plan

## Overview

Add smart review and repair agents to the swarm workflow that:
1. Optionally review code quality after each wave (configurable)
2. Attribute validation failures to specific tasks using git blame
3. Spawn smart repair agents to fix issues and iterate until all tasks pass

## Current State Analysis

### What Exists
- Backpressure validation runs after each wave (`src/commands/swarm/mod.rs:258-291`)
- On failure, ALL wave tasks marked as `Failed` (no attribution)
- `WaveState` tracks: `start_commit`, `files_changed`, `validation` results
- Commits prefixed with `[TASK-ID]` but not parsed for attribution
- `AgentDef` system loads agent definitions from `.scud/agents/<name>.toml`
- `spawn_terminal_with_harness_and_model()` can spawn with specific model

### Key Files
- `src/commands/swarm/mod.rs:258-291` - Validation and failure handling
- `src/commands/swarm/session.rs` - Wave/round state tracking
- `src/commands/spawn/terminal.rs:307-326` - Agent spawning
- `src/agents/mod.rs` - Agent definition loading
- `src/backpressure.rs` - Validation execution

## Desired End State

After implementation:
1. `scud swarm --review` spawns a reviewer after each wave to check quality
2. `scud swarm --review-all` reviews ALL tasks (expensive but thorough)
3. On validation failure, git blame attributes errors to specific tasks
4. Only responsible tasks marked failed; others proceed
5. Smart repair agent spawns, fixes issues, re-runs validation
6. Iterates until validation passes or max attempts reached
7. New `reviewer` and `repairer` spawn agents use claude/opus

### Verification
- `scud swarm --review --dry-run` shows review plan
- Validation failures show attributed task IDs in output
- Session JSON includes attribution data and repair attempts
- `scud config spawn-agents list` shows reviewer/repairer agents

## What We're NOT Doing

- Real-time streaming of agent output to swarm monitor
- Automatic task complexity adjustment based on failure rate
- Cross-wave learning (each wave is independent)
- Integration with external code review tools (GitHub PR reviews)
- Custom reviewer prompts per project (use agent definition system)

## Implementation Approach

The implementation follows a bottom-up approach:
1. Add git blame infrastructure for failure attribution
2. Create reviewer and repairer spawn agent definitions
3. Extend swarm CLI with review flags
4. Implement review agent spawning after waves
5. Implement repair loop on validation failure
6. Wire everything together in the swarm main loop

---

## Phase 1: Git Blame Infrastructure for Failure Attribution

### Overview
Add functions to attribute validation failures to specific tasks by parsing error output and using git blame to map changed lines to commits with task IDs.

### Changes Required:

#### 1.1 New Attribution Module

**File**: `src/attribution.rs` (new file)
**Changes**: Create module for failure attribution logic

```rust
//! Failure attribution using git blame
//!
//! Maps validation errors to specific tasks by:
//! 1. Parsing error output for file:line references
//! 2. Using git blame to find which commit changed each line
//! 3. Extracting task IDs from commit messages ([TASK-ID] prefix)

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use regex::Regex;

/// Result of attributing a failure to tasks
#[derive(Debug, Clone)]
pub struct Attribution {
    /// Task IDs that likely caused the failure
    pub responsible_tasks: Vec<String>,
    /// Task IDs that are probably not responsible
    pub cleared_tasks: Vec<String>,
    /// Whether attribution was definitive or uncertain
    pub confidence: AttributionConfidence,
    /// Raw evidence used for attribution
    pub evidence: Vec<AttributionEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributionConfidence {
    /// Clear single task responsible
    High,
    /// Multiple tasks may be responsible
    Medium,
    /// Could not determine - all tasks suspect
    Low,
}

#[derive(Debug, Clone)]
pub struct AttributionEvidence {
    pub file: String,
    pub line: Option<u32>,
    pub task_id: Option<String>,
    pub commit_sha: Option<String>,
    pub error_snippet: String,
}

/// Parse error output for file:line references
pub fn parse_error_locations(stderr: &str, stdout: &str) -> Vec<(String, Option<u32>)> {
    let mut locations = Vec::new();
    let combined = format!("{}\n{}", stderr, stdout);

    // Common patterns:
    // Rust: --> src/main.rs:42:5
    // TypeScript: src/index.ts(10,5): error
    // Go: ./main.go:15:3:
    // Python: File "script.py", line 10
    // Generic: filename:line or filename:line:col

    let patterns = [
        r"(?:-->|error\[.*?\]:)\s+([^:\s]+):(\d+)",  // Rust
        r"([^\s(]+)\((\d+),\d+\):",                   // TypeScript
        r"([^\s:]+):(\d+):\d+:",                      // Go/generic
        r#"File "([^"]+)", line (\d+)"#,              // Python
        r"([^\s:]+):(\d+)",                           // Generic fallback
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            for cap in re.captures_iter(&combined) {
                if let (Some(file), Some(line)) = (cap.get(1), cap.get(2)) {
                    let file_str = file.as_str().to_string();
                    let line_num = line.as_str().parse::<u32>().ok();
                    if !locations.iter().any(|(f, _)| f == &file_str) {
                        locations.push((file_str, line_num));
                    }
                }
            }
        }
    }

    locations
}

/// Get task ID from a commit message (looks for [TASK-ID] prefix)
pub fn extract_task_id_from_commit(message: &str) -> Option<String> {
    let re = Regex::new(r"\[([^\]]+)\]").ok()?;
    re.captures(message)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Use git blame to find which task changed a specific line
pub fn blame_line(working_dir: &Path, file: &str, line: u32) -> Result<Option<String>> {
    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["blame", "-L", &format!("{},{}", line, line), "--porcelain", file])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let blame_output = String::from_utf8_lossy(&output.stdout);

    // Look for "summary" line in porcelain output
    for line in blame_output.lines() {
        if line.starts_with("summary ") {
            let message = line.strip_prefix("summary ").unwrap_or("");
            return Ok(extract_task_id_from_commit(message));
        }
    }

    Ok(None)
}

/// Get all commits in range that match task ID pattern
pub fn get_task_commits(
    working_dir: &Path,
    start_commit: Option<&str>,
) -> Result<HashMap<String, Vec<String>>> {
    let range = match start_commit {
        Some(commit) => format!("{}..HEAD", commit),
        None => "HEAD~10..HEAD".to_string(),
    };

    let output = Command::new("git")
        .current_dir(working_dir)
        .args(["log", "--format=%H %s", &range])
        .output()?;

    let mut task_commits: HashMap<String, Vec<String>> = HashMap::new();

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let sha = parts[0].to_string();
            let message = parts[1];
            if let Some(task_id) = extract_task_id_from_commit(message) {
                task_commits.entry(task_id).or_default().push(sha);
            }
        }
    }

    Ok(task_commits)
}

/// Get files changed by a specific task (via its commits)
pub fn get_task_changed_files(
    working_dir: &Path,
    task_id: &str,
    start_commit: Option<&str>,
) -> Result<HashSet<String>> {
    let task_commits = get_task_commits(working_dir, start_commit)?;
    let mut files = HashSet::new();

    if let Some(commits) = task_commits.get(task_id) {
        for sha in commits {
            let output = Command::new("git")
                .current_dir(working_dir)
                .args(["diff-tree", "--no-commit-id", "--name-only", "-r", sha])
                .output()?;

            for file in String::from_utf8_lossy(&output.stdout).lines() {
                files.insert(file.to_string());
            }
        }
    }

    Ok(files)
}

/// Main attribution function - attributes validation failure to tasks
pub fn attribute_failure(
    working_dir: &Path,
    stderr: &str,
    stdout: &str,
    wave_tasks: &[String],
    start_commit: Option<&str>,
) -> Result<Attribution> {
    let mut evidence = Vec::new();
    let mut responsible: HashSet<String> = HashSet::new();

    // Parse error locations
    let locations = parse_error_locations(stderr, stdout);

    // Try to blame each location
    for (file, line_opt) in &locations {
        let mut ev = AttributionEvidence {
            file: file.clone(),
            line: *line_opt,
            task_id: None,
            commit_sha: None,
            error_snippet: String::new(),
        };

        if let Some(line) = line_opt {
            if let Ok(Some(task_id)) = blame_line(working_dir, file, *line) {
                if wave_tasks.contains(&task_id) {
                    responsible.insert(task_id.clone());
                    ev.task_id = Some(task_id);
                }
            }
        }

        evidence.push(ev);
    }

    // If no direct attribution, check which tasks touched error files
    if responsible.is_empty() && !locations.is_empty() {
        let error_files: HashSet<String> = locations.iter().map(|(f, _)| f.clone()).collect();

        for task_id in wave_tasks {
            if let Ok(task_files) = get_task_changed_files(working_dir, task_id, start_commit) {
                if !task_files.is_disjoint(&error_files) {
                    responsible.insert(task_id.clone());
                }
            }
        }
    }

    let confidence = if responsible.len() == 1 {
        AttributionConfidence::High
    } else if !responsible.is_empty() {
        AttributionConfidence::Medium
    } else {
        // Could not attribute - all tasks suspect
        responsible.extend(wave_tasks.iter().cloned());
        AttributionConfidence::Low
    };

    let cleared: Vec<String> = wave_tasks
        .iter()
        .filter(|t| !responsible.contains(*t))
        .cloned()
        .collect();

    Ok(Attribution {
        responsible_tasks: responsible.into_iter().collect(),
        cleared_tasks: cleared,
        confidence,
        evidence,
    })
}
```

#### 1.2 Add Module to lib.rs

**File**: `src/lib.rs`
**Changes**: Add `pub mod attribution;`

```rust
pub mod attribution;
```

#### 1.3 Add regex Dependency

**File**: `Cargo.toml`
**Changes**: Add regex crate (likely already present, verify)

```toml
[dependencies]
regex = "1"
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] Unit tests for attribution module pass: `cargo test attribution`
- [x] Clippy passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [x] Create a test scenario with intentional error, verify blame output (added integration tests)

---

## Phase 2: Reviewer and Repairer Spawn Agent Definitions

### Overview
Create spawn agent definitions for the reviewer (quality check) and repairer (fix issues) agents.

### Changes Required:

#### 2.1 Reviewer Agent Definition

**File**: `src/assets/spawn-agents/reviewer.toml`
**Changes**: Update existing reviewer to include wave context template

```toml
# Wave reviewer agent - reviews code quality after each wave
[agent]
name = "reviewer"
description = "Reviews wave output for code quality and implementation correctness"

[model]
harness = "claude"
model = "opus"

[prompt]
template = """You are reviewing the output of SCUD wave {wave_number}.

## Wave Summary
Tasks completed: {tasks_completed}
Files changed: {files_changed}

## Your Role
Review the code changes for:
1. Code correctness - does the implementation match requirements?
2. Code quality - clean, readable, follows project conventions?
3. Test coverage - are changes adequately tested?
4. Potential issues - bugs, edge cases, security concerns?

## Review Process
1. Read each changed file with: cat <file>
2. Check task requirements with: scud show <task_id>
3. Look for obvious issues, style problems, missing error handling
4. If improvements needed, note the task ID and issue

## Output
For each task reviewed, either:
- PASS: Task {task_id} - implementation looks good
- IMPROVE: Task {task_id} - {specific issue to fix}

When done reviewing, if all tasks pass:
  echo "REVIEW_COMPLETE: ALL_PASS"

If improvements needed:
  echo "REVIEW_COMPLETE: IMPROVEMENTS_NEEDED"
  echo "IMPROVE_TASKS: {comma-separated task IDs}"
"""
```

#### 2.2 Repairer Agent Definition

**File**: `src/assets/spawn-agents/repairer.toml` (new file)
**Changes**: Create repairer agent for fixing validation failures

```toml
# Repairer agent - fixes validation failures
[agent]
name = "repairer"
description = "Smart agent that fixes validation failures attributed to specific tasks"

[model]
harness = "claude"
model = "opus"

[prompt]
template = """You are a repair agent fixing validation failures for SCUD task {task.id}: {task.title}

## Validation Failure
The following validation command failed:
{failed_command}

Error output:
{error_output}

## Attribution
This failure has been attributed to task {task.id} based on git blame analysis.
Files changed by this task: {task_files}

## Your Mission
1. Analyze the error output to understand what went wrong
2. Read the relevant files: {error_files}
3. Fix the issue while preserving the task's intended functionality
4. Run the validation command to verify the fix: {failed_command}

## Important
- Focus on fixing the specific error, don't refactor unrelated code
- If the fix requires changes to other tasks' code, note it but don't modify
- After fixing, commit with: scud commit -m "fix: {task.id} - <description>"

When the validation passes:
  scud set-status {task.id} done
  echo "REPAIR_COMPLETE: SUCCESS"

If you cannot fix it:
  scud set-status {task.id} blocked
  echo "REPAIR_COMPLETE: BLOCKED"
  echo "REASON: <explanation>"
"""
```

#### 2.3 Add Repairer to Embedded Agents

**File**: `src/commands/config.rs`
**Changes**: Add repairer to EMBEDDED_SPAWN_AGENTS constant

```rust
const EMBEDDED_SPAWN_AGENTS: &[(&str, &str)] = &[
    // ... existing agents ...
    (
        "repairer",
        include_str!("../assets/spawn-agents/repairer.toml"),
    ),
];
```

#### 2.4 Update CLI Help

**File**: `src/main.rs`
**Changes**: Add repairer to spawn-agents add help text

```rust
/// Agent name (builder, reviewer, planner, researcher, analyzer, fast-builder, outside-generalist, repairer)
name: Option<String>,
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] `scud config spawn-agents list` shows repairer agent

#### Manual Verification:
- [x] `scud config spawn-agents add repairer` installs correctly

---

## Phase 3: Extend Swarm CLI with Review Flags

### Overview
Add CLI flags to control review behavior in swarm mode.

### Changes Required:

#### 3.1 Add Review Flags to Swarm Command

**File**: `src/main.rs`
**Changes**: Add review-related flags to Swarm command

```rust
/// Run swarm mode - wave-based parallel execution with backpressure
Swarm {
    // ... existing fields ...

    /// Run review agent after each wave (sample tasks)
    #[arg(long)]
    review: bool,

    /// Run review agent on ALL tasks (more thorough, more expensive)
    #[arg(long, conflicts_with = "review")]
    review_all: bool,

    /// Maximum repair attempts per wave before giving up
    #[arg(long, default_value = "3")]
    max_repair_attempts: usize,

    /// Skip repair attempts on validation failure (just mark tasks failed)
    #[arg(long)]
    no_repair: bool,
},
```

#### 3.2 Update Swarm Run Function Signature

**File**: `src/commands/swarm/mod.rs`
**Changes**: Add parameters for review/repair behavior

```rust
#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    round_size: usize,
    all_tags: bool,
    terminal_arg: &str,
    harness_arg: &str,
    dry_run: bool,
    session_name: Option<String>,
    no_research: bool,
    no_validate: bool,
    // New parameters:
    review: bool,
    review_all: bool,
    max_repair_attempts: usize,
    no_repair: bool,
) -> Result<()> {
```

#### 3.3 Wire Up CLI to Function

**File**: `src/main.rs`
**Changes**: Pass new flags to swarm::run()

```rust
Commands::Swarm {
    tag,
    round_size,
    all_tags,
    terminal,
    harness,
    dry_run,
    session,
    no_research,
    no_validate,
    review,
    review_all,
    max_repair_attempts,
    no_repair,
} => commands::swarm::run(
    cli.project,
    tag.as_deref(),
    round_size,
    all_tags,
    &terminal,
    &harness,
    dry_run,
    session,
    no_research,
    no_validate,
    review,
    review_all,
    max_repair_attempts,
    no_repair,
),
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] `scud swarm --help` shows new flags

#### Manual Verification:
- [x] Verify flags are correctly parsed with `--dry-run`

---

## Phase 4: Implement Review Agent Spawning

### Overview
Add logic to spawn a reviewer agent after each wave completes (before validation, or after if validation passes).

### Changes Required:

#### 4.1 Add Review Prompt Generation

**File**: `src/commands/spawn/agent.rs`
**Changes**: Add function to generate review prompts

```rust
use crate::commands::swarm::session::WaveSummary;

/// Generate a prompt for wave review
pub fn generate_review_prompt(
    summary: &WaveSummary,
    tasks: &[(String, String)], // (task_id, title)
    review_all: bool,
) -> String {
    let tasks_str = if review_all {
        tasks
            .iter()
            .map(|(id, title)| format!("- {} | {}", id, title))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Sample: first task, last task, and one random middle task
        let sample: Vec<_> = if tasks.len() <= 3 {
            tasks.iter().collect()
        } else {
            vec![&tasks[0], &tasks[tasks.len() / 2], &tasks[tasks.len() - 1]]
        };
        sample
            .iter()
            .map(|(id, title)| format!("- {} | {}", id, title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let files_str = if summary.files_changed.len() <= 10 {
        summary.files_changed.join("\n")
    } else {
        let mut s = summary.files_changed[..10].join("\n");
        s.push_str(&format!("\n... and {} more files", summary.files_changed.len() - 10));
        s
    };

    format!(
        r#"You are reviewing SCUD wave {wave_number}.

## Tasks to Review
{tasks}

## Files Changed
{files}

## Review Process
1. For each task, run: scud show <task_id>
2. Read the changed files relevant to each task
3. Check implementation quality and correctness

## Output Format
For each task:
  PASS: <task_id> - looks good
  IMPROVE: <task_id> - <specific issue>

When complete:
  echo "REVIEW_COMPLETE: ALL_PASS" or
  echo "REVIEW_COMPLETE: IMPROVEMENTS_NEEDED"
  echo "IMPROVE_TASKS: <comma-separated task IDs>"
"#,
        wave_number = summary.wave_number,
        tasks = tasks_str,
        files = files_str,
    )
}
```

#### 4.2 Add Review Spawning Function

**File**: `src/commands/swarm/mod.rs`
**Changes**: Add function to spawn reviewer and wait for completion

```rust
use crate::agents::AgentDef;

/// Spawn a reviewer agent and wait for it to complete
fn spawn_reviewer(
    storage: &Storage,
    working_dir: &Path,
    session_name: &str,
    terminal: &Terminal,
    summary: &WaveSummary,
    wave_tasks: &[(String, String)], // (id, title)
    review_all: bool,
) -> Result<ReviewResult> {
    println!();
    println!("  {} Spawning reviewer agent...", "Review:".magenta());

    let prompt = agent::generate_review_prompt(summary, wave_tasks, review_all);

    // Load reviewer agent definition for harness/model
    let agent_def = AgentDef::try_load("reviewer", working_dir)
        .unwrap_or_else(|| {
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

    // Write prompt to temp file for tracking
    let prompt_file = std::env::temp_dir().join(format!("scud-review-wave-{}.txt", summary.wave_number));
    std::fs::write(&prompt_file, &prompt)?;

    // Spawn reviewer
    terminal::spawn_terminal_with_harness_and_model(
        terminal,
        &format!("review-wave-{}", summary.wave_number),
        &prompt,
        working_dir,
        session_name,
        harness,
        model,
    )?;

    println!("    {} Reviewer spawned, waiting for completion...", "✓".green());

    // Wait for reviewer to complete by watching for output file
    // The reviewer writes "REVIEW_COMPLETE: ..." when done
    wait_for_review_completion(working_dir, summary.wave_number)
}

#[derive(Debug)]
struct ReviewResult {
    all_passed: bool,
    tasks_to_improve: Vec<String>,
}

fn wait_for_review_completion(working_dir: &Path, wave_number: usize) -> Result<ReviewResult> {
    // For now, we wait for the reviewer terminal to exit
    // In the future, could watch for a signal file

    // Simple approach: poll for a marker file that reviewer creates
    let marker_path = working_dir.join(".scud").join(format!("review-complete-{}", wave_number));

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

            return Ok(ReviewResult {
                all_passed,
                tasks_to_improve,
            });
        }

        thread::sleep(Duration::from_secs(5));
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] Unit tests pass: `cargo test`

#### Manual Verification:
- [ ] Run `scud swarm --review --dry-run` shows review plan
- [ ] Test reviewer spawning with a simple wave

---

## Phase 5: Implement Repair Loop on Validation Failure

### Overview
When validation fails, attribute the failure, spawn repair agents for responsible tasks, and iterate until validation passes.

### Changes Required:

#### 5.1 Add Repair Prompt Generation

**File**: `src/commands/spawn/agent.rs`
**Changes**: Add function to generate repair prompts

```rust
use crate::backpressure::CommandResult;

/// Generate a prompt for repairing a validation failure
pub fn generate_repair_prompt(
    task: &Task,
    tag: &str,
    failed_command: &CommandResult,
    task_files: &[String],
    error_files: &[String],
) -> String {
    format!(
        r#"You are a repair agent fixing validation failures for SCUD task {id}: {title}

## Validation Failure
Command: {command}
Exit code: {exit_code}

Error output:
```
{stderr}
```

Standard output:
```
{stdout}
```

## Task Context
Tag: {tag}
Description: {description}

## Files Changed by This Task
{task_files}

## Files Mentioned in Errors
{error_files}

## Your Mission
1. Analyze the error output to understand what went wrong
2. Read the relevant files
3. Fix the issue while preserving the task's intended functionality
4. Run the validation command to verify: {command}
5. Commit the fix: scud commit -m "fix: {id} - <description>"

When the validation passes:
  scud set-status {id} done
  echo "REPAIR_COMPLETE: SUCCESS" > .scud/repair-complete-{id}

If you cannot fix it:
  scud set-status {id} blocked
  echo "REPAIR_COMPLETE: BLOCKED" > .scud/repair-complete-{id}
  echo "REASON: <explanation>" >> .scud/repair-complete-{id}
"#,
        id = task.id,
        title = task.title,
        tag = tag,
        description = task.description,
        command = failed_command.command,
        exit_code = failed_command.exit_code.map(|c| c.to_string()).unwrap_or("N/A".to_string()),
        stderr = &failed_command.stderr,
        stdout = &failed_command.stdout,
        task_files = task_files.join("\n"),
        error_files = error_files.join("\n"),
    )
}
```

#### 5.2 Add Repair Loop Function

**File**: `src/commands/swarm/mod.rs`
**Changes**: Add function to run repair loop

```rust
use crate::attribution::{attribute_failure, Attribution, AttributionConfidence};

/// Run repair loop for failed validation
fn run_repair_loop(
    storage: &Storage,
    working_dir: &Path,
    session_name: &str,
    terminal: &Terminal,
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
            println!("    {} High confidence: task {} responsible",
                "✓".green(),
                attribution.responsible_tasks.join(", ")
            );
        }
        AttributionConfidence::Medium => {
            println!("    {} Medium confidence: tasks {} may be responsible",
                "~".yellow(),
                attribution.responsible_tasks.join(", ")
            );
        }
        AttributionConfidence::Low => {
            println!("    {} Low confidence: cannot determine specific task",
                "!".red()
            );
        }
    }

    // Mark cleared tasks as done
    for task_id in &attribution.cleared_tasks {
        if let Some(tag) = task_tags.iter().find(|(id, _)| id == task_id).map(|(_, t)| t) {
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
        println!("  {} Repair attempt {}/{}", "Repair:".magenta(), attempt, max_attempts);

        let mut all_repaired = true;

        for task_id in &attribution.responsible_tasks {
            // Find task details
            let (task, tag) = match find_task_with_tag(storage, task_id, &task_tags) {
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
            let error_files: Vec<String> = crate::attribution::parse_error_locations(
                &failed_cmd.stderr,
                &failed_cmd.stdout,
            )
            .into_iter()
            .map(|(f, _)| f)
            .collect();

            // Generate repair prompt
            let prompt = agent::generate_repair_prompt(
                &task,
                &tag,
                failed_cmd,
                &task_files.into_iter().collect::<Vec<_>>(),
                &error_files,
            );

            // Spawn repairer
            spawn_repairer(
                working_dir,
                session_name,
                terminal,
                task_id,
                &prompt,
            )?;

            // Wait for repair completion
            if !wait_for_repair_completion(working_dir, task_id)? {
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
        let new_result = backpressure::run_validation(working_dir, bp_config)?;

        if new_result.all_passed {
            println!("    {} Validation passed after repair!", "✓".green());

            // Mark all responsible tasks as done
            for task_id in &attribution.responsible_tasks {
                if let Some(tag) = task_tags.iter().find(|(id, _)| id == task_id).map(|(_, t)| t) {
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

        println!("    {} Validation still failing, will retry...", "!".yellow());
    }

    // Max attempts reached - mark responsible tasks as failed
    println!();
    println!("  {} Max repair attempts reached", "!".red());

    for task_id in &attribution.responsible_tasks {
        if let Some(tag) = task_tags.iter().find(|(id, _)| id == task_id).map(|(_, t)| t) {
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

fn spawn_repairer(
    working_dir: &Path,
    session_name: &str,
    terminal: &Terminal,
    task_id: &str,
    prompt: &str,
) -> Result<()> {
    // Load repairer agent definition
    let agent_def = AgentDef::try_load("repairer", working_dir)
        .unwrap_or_else(|| {
            AgentDef {
                agent: crate::agents::AgentMeta {
                    name: "repairer".to_string(),
                    description: "Repair agent".to_string(),
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

    terminal::spawn_terminal_with_harness_and_model(
        terminal,
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

fn wait_for_repair_completion(working_dir: &Path, task_id: &str) -> Result<bool> {
    let marker_path = working_dir.join(".scud").join(format!("repair-complete-{}", task_id));

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

            return Ok(content.contains("SUCCESS"));
        }

        thread::sleep(Duration::from_secs(5));
    }
}

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
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] Attribution tests pass: `cargo test attribution`

#### Manual Verification:
- [ ] Create intentional failure, verify attribution works
- [ ] Verify repair agent spawns with correct context

---

## Phase 6: Wire Everything Together in Swarm Main Loop

### Overview
Integrate the review and repair functionality into the main swarm execution loop.

### Changes Required:

#### 6.1 Update Main Swarm Loop

**File**: `src/commands/swarm/mod.rs`
**Changes**: Add review and repair calls to main loop

Replace the validation section (lines ~258-291) with:

```rust
        // === PHASE 3: VALIDATE (optional) ===
        if !no_validate && !bp_config.commands.is_empty() {
            println!();
            println!("  {} Running backpressure checks...", "Validate:".magenta());

            let validation_result = backpressure::run_validation(&working_dir, &bp_config)?;

            if validation_result.all_passed {
                println!("    {} All checks passed", "✓".green());

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
                        &terminal,
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

        // === PHASE 4: REVIEW (optional) ===
        if (review || review_all) && !dry_run {
            // Build task list for review
            let wave_tasks: Vec<(String, String)> = wave_state
                .task_tags()
                .iter()
                .filter_map(|(id, tag)| {
                    storage.load_group(tag).ok().and_then(|phase| {
                        phase.get_task(id).map(|t| (id.clone(), t.title.clone()))
                    })
                })
                .collect();

            if !wave_tasks.is_empty() {
                let review_result = spawn_reviewer(
                    &storage,
                    &working_dir,
                    &session_name,
                    &terminal,
                    wave_state.summary.as_ref().unwrap_or(&WaveSummary {
                        wave_number,
                        tasks_completed: vec![],
                        files_changed: vec![],
                    }),
                    &wave_tasks,
                    review_all,
                )?;

                if !review_result.all_passed && !review_result.tasks_to_improve.is_empty() {
                    println!("    {} Reviewer found issues in: {}",
                        "!".yellow(),
                        review_result.tasks_to_improve.join(", ")
                    );

                    // Spawn improvement agents for flagged tasks
                    for task_id in &review_result.tasks_to_improve {
                        // Find task and spawn builder to improve
                        if let Some((task, tag)) = find_task_with_tag(&storage, task_id, &wave_state.task_tags()) {
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
                                    &terminal,
                                    &format!("improve-{}", task_id),
                                    &prompt,
                                    &working_dir,
                                    &session_name,
                                    harness,
                                    model,
                                )?;

                                println!("    {} Spawned improvement agent for {}", "✓".green(), task_id);
                            }
                        }
                    }
                } else {
                    println!("    {} Review complete, all tasks approved", "✓".green());
                }
            }
        }
```

#### 6.2 Update Swarm Header Display

**File**: `src/commands/swarm/mod.rs`
**Changes**: Add review/repair info to header

```rust
    println!(
        "{:<20} {}",
        "Review:".dimmed(),
        if review_all {
            "all tasks".green()
        } else if review {
            "sample".green()
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
```

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] All tests pass: `cargo test`
- [x] Clippy passes: `cargo clippy -- -D warnings`

#### Manual Verification:
- [ ] `scud swarm --help` shows all new options
- [ ] `scud swarm --review --dry-run` shows review in plan
- [ ] Test with intentional failure to verify repair loop

---

## Phase 7: Session State Updates and Persistence

### Overview
Update session state structures to track review and repair attempts.

### Changes Required:

#### 7.1 Extend WaveState

**File**: `src/commands/swarm/session.rs`
**Changes**: Add review and repair tracking fields

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveState {
    // ... existing fields ...

    /// Review result (if review was run)
    #[serde(default)]
    pub review: Option<ReviewState>,

    /// Repair attempts (if validation failed)
    #[serde(default)]
    pub repairs: Vec<RepairAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewState {
    pub reviewed_tasks: Vec<String>,
    pub all_passed: bool,
    pub tasks_needing_improvement: Vec<String>,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAttempt {
    pub attempt_number: usize,
    pub attributed_tasks: Vec<String>,
    pub cleared_tasks: Vec<String>,
    pub attribution_confidence: String, // "high", "medium", "low"
    pub validation_passed: bool,
    pub completed_at: String,
}
```

#### 7.2 Update Repair Loop to Track State

Update `run_repair_loop()` to populate `RepairAttempt` structs and add to wave state.

### Success Criteria:

#### Automated Verification:
- [x] Build passes: `cargo build`
- [x] Session JSON includes new fields

#### Manual Verification:
- [ ] Review session JSON after swarm run with review/repair

---

## Testing Strategy

### Unit Tests:
- Attribution parsing (error location extraction)
- Git blame output parsing
- Task ID extraction from commit messages
- Prompt generation for review/repair agents

### Integration Tests:
- End-to-end swarm with intentional failure
- Attribution accuracy with known failure source
- Repair loop iteration count
- Review agent spawning

### Manual Testing Steps:
1. Create simple tasks that will pass validation
2. Run `scud swarm --review` and verify reviewer spawns
3. Create task that will fail validation (syntax error)
4. Run `scud swarm` and verify repair loop engages
5. Verify attribution correctly identifies failing task
6. Verify cleared tasks are marked done, not failed

## Performance Considerations

- Reviewer agent uses opus (expensive) - offer sampling mode by default
- Repair loop has max attempts to prevent infinite loops
- Attribution parsing uses regex - should be fast
- Git blame calls are per-error-line - could batch for many errors

## Migration Notes

- New CLI flags are additive, backward compatible
- Default behavior unchanged (no review, no repair)
- Session JSON gains new optional fields (backward compatible)

## References

- Current swarm implementation: `src/commands/swarm/mod.rs`
- Backpressure system: `src/backpressure.rs`
- Agent definitions: `src/agents/mod.rs`
- Spawn system: `src/commands/spawn/terminal.rs`
