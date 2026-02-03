# Batch Repair Agent and Tmux Health Check Implementation Plan

## Overview

This plan addresses two related issues in the SCUD swarm orchestration:

1. **Inefficient Repair Loop**: When backpressure validation fails, the current implementation spawns one repair agent per task sequentially, which is slow and doesn't leverage the agent's ability to analyze multiple issues at once.

2. **Stale Tmux Sessions**: When an agent process crashes and leaves tmux idle, tasks remain stuck as `InProgress` until manual intervention.

## Current State Analysis

### Back Pressure Repair Loop (`swarm/mod.rs:1937-2119`)

The current `run_repair_loop()` function:
- Attributes failures to specific tasks via `attribute_failure()`
- Iterates through `attribution.responsible_tasks` sequentially (line 2020)
- Spawns one repair agent per task via `spawn_repairer()` (line 2052)
- Waits for each repair to complete before spawning the next (line 2055)
- Re-runs validation after ALL repairs complete (line 2068)
- Retries up to `max_attempts` times

**Problem**: Even with good attribution, this spawns N agents for N responsible tasks. Each agent works in isolation without seeing what other agents are doing. The iteration is sequential, not parallel.

### Tmux Health Checking (`swarm/mod.rs:1571-1710`)

The current health checking:
- Orphan detection every 30s: marks tasks `Failed` if tmux window gone (line 1575)
- Content-based heartbeat: hashes pane content to detect activity (line 1634-1660)
- Idle agent display: shows "(N idle >60s)" in status (line 1695-1702)

**Problem**: Idle detection is informational only - it displays idle agents but takes no action. If an agent process crashes but tmux window remains open (e.g., shell prompt sitting idle), the task stays `InProgress` forever.

### Key Discoveries

1. **Review prompt pattern exists** (`spawn/agent.rs:240-305`): `generate_review_prompt()` already handles reviewing multiple tasks at once - we can model the batch repair prompt after this.

2. **Doctor command pattern** (`doctor.rs`): Has infrastructure for detecting stale `InProgress` tasks and can auto-fix them with `--fix` flag.

3. **Marker file pattern**: Repair agents signal completion via `.scud/repair-complete-{task_id}` files.

## Desired End State

### Issue 1: Batch Repair Agent

When backpressure validation fails:
1. System analyzes failure attribution once
2. Spawns a **single** "batch repair" agent with a prompt containing:
   - All validation errors
   - All responsible tasks and their context
   - Instructions to analyze and fix systematically
3. Agent reviews all issues in one pass, iterating internally if needed
4. Agent signals completion via single marker file
5. System re-validates and proceeds

### Issue 2: Tmux Health Checks

During wave execution:
1. Content-based heartbeat tracks last activity per agent
2. If an agent has been idle >N minutes (configurable):
   - Check if the tmux pane shows a shell prompt (not an active process)
   - If shell prompt detected + idle timeout exceeded → mark task `Failed`
3. Emit proper events and allow retry on next wave

**Verification**:
- Run `scud swarm` with a task that crashes the agent mid-execution
- Observe task gets auto-failed after idle timeout
- Run backpressure test that fails multiple tasks
- Observe single batch repair agent spawned instead of N agents

## What We're NOT Doing

- Not changing the attribution logic itself (still uses git blame)
- Not adding new CLI flags for idle timeout (use existing `--stale-timeout-minutes`)
- Not removing the per-task repair capability (keep as fallback)
- Not changing how tmux sessions are created
- Not modifying the existing review agent flow

## Implementation Approach

We'll make surgical changes to the repair loop and add idle-based failure detection:

1. Add `generate_batch_repair_prompt()` function
2. Replace sequential repair iteration with single batch agent spawn
3. Add idle timeout failure detection in `wait_for_round_completion()`
4. Add shell prompt detection helper

## Phase 1: Batch Repair Prompt Generation

### Overview
Create a new prompt generator that gives one agent context about all failing tasks.

### Changes Required:

#### 1.1 Add Batch Repair Prompt

**File**: `scud-cli/src/commands/spawn/agent.rs`
**Changes**: Add `generate_batch_repair_prompt()` function after `generate_repair_prompt()` (around line 363)

```rust
/// Generate a prompt for batch repair agent handling multiple tasks
pub fn generate_batch_repair_prompt(
    tasks: &[(String, String, Vec<String>)], // (task_id, title, files_changed)
    failed_command: &str,
    error_output: &str,
    error_locations: &[(String, usize)], // (file, line)
) -> String {
    let tasks_str = tasks
        .iter()
        .map(|(id, title, files)| {
            format!(
                "- {} | {}\n  Files: {}",
                id,
                title,
                files.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let error_locations_str = error_locations
        .iter()
        .take(20) // Limit to avoid prompt explosion
        .map(|(file, line)| format!("  {}:{}", file, line))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a batch repair agent fixing validation failures for multiple SCUD tasks.

## Validation Failure
The following validation command failed:
{failed_command}

Error output:
{error_output}

## Error Locations
{error_locations}

## Responsible Tasks
Based on git blame analysis, these tasks may be responsible:
{tasks}

## Your Mission
1. Analyze the error output to understand ALL the issues
2. Read the relevant files and understand what each task was trying to do
3. Fix issues systematically - some errors may be related
4. Run the validation command after each fix to check progress: {failed_command}

## Process
For each issue:
1. Identify which task introduced it
2. Read the task details: scud show <task_id>
3. Fix the issue while preserving intended functionality
4. Commit: scud commit -m "fix: <task_id> - <description>"
5. Log: scud log <task_id> "Fixed: <brief description>"

## Important
- Fix ALL issues before signaling completion
- Some issues may cascade - fix root causes first
- If you cannot fix an issue, document why
- Iterate until validation passes or you're truly blocked

## Completion
When ALL validation passes:
  echo "BATCH_REPAIR_COMPLETE: SUCCESS" > .scud/batch-repair-complete
  echo "FIXED_TASKS: <comma-separated task IDs that were fixed>" >> .scud/batch-repair-complete

If blocked on some tasks:
  echo "BATCH_REPAIR_COMPLETE: PARTIAL" > .scud/batch-repair-complete
  echo "FIXED_TASKS: <task IDs fixed>" >> .scud/batch-repair-complete
  echo "BLOCKED_TASKS: <task IDs blocked>" >> .scud/batch-repair-complete
  echo "BLOCK_REASON: <explanation>" >> .scud/batch-repair-complete

If completely blocked:
  echo "BATCH_REPAIR_COMPLETE: BLOCKED" > .scud/batch-repair-complete
  echo "REASON: <explanation>" >> .scud/batch-repair-complete
"#,
        failed_command = failed_command,
        error_output = error_output,
        error_locations = error_locations_str,
        tasks = tasks_str,
    )
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Build passes: `cargo build -p scud-cli`
- [ ] Linting passes: `cargo clippy -p scud-cli`
- [ ] Unit tests pass: `cargo test -p scud-cli`

#### Manual Verification:
- [ ] New function compiles without warnings

---

## Phase 2: Batch Repair Loop Implementation

### Overview
Replace the sequential per-task repair spawning with a single batch repair agent.

### Changes Required:

#### 2.1 Modify `run_repair_loop()` for Batch Mode

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Replace the inner loop (lines 2009-2096) with batch repair logic

Replace the section from line 2009 (`for attempt in 1..=max_attempts {`) through line 2096 (`}` closing the max_attempts loop) with:

```rust
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
    let error_locations: Vec<(String, usize)> =
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
```

#### 2.2 Add Batch Repair Helper Functions

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add after `spawn_repairer()` function (around line 2155)

```rust
/// Result of a batch repair attempt
enum BatchRepairResult {
    Success(Vec<String>),           // All fixed, list of task IDs
    Partial(Vec<String>, Vec<String>), // Some fixed, some blocked
    Blocked(String),                // Completely blocked with reason
    Timeout,                        // Timed out
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
```

#### 2.3 Keep Legacy Per-Task Repair as Fallback

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Rename existing functions and keep them available

- Rename `spawn_repairer` to `spawn_single_repairer`
- Rename `wait_for_repair_completion_task` to `wait_for_single_repair_completion`
- Keep them in place for potential fallback use or single-task scenarios

### Success Criteria:

#### Automated Verification:
- [ ] Build passes: `cargo build -p scud-cli`
- [ ] Linting passes: `cargo clippy -p scud-cli`
- [ ] Unit tests pass: `cargo test -p scud-cli`

#### Manual Verification:
- [ ] Run swarm with tasks that will cause a build failure
- [ ] Observe single "batch-repair" tmux window spawned instead of N repair windows
- [ ] Observe batch repair agent analyzing all issues at once
- [ ] Verify validation re-runs after batch repair completes

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Idle Agent Health Check

### Overview
Add automatic failure detection for agents that have been idle too long with no active process.

### Changes Required:

#### 3.1 Add Shell Prompt Detection Helper

**File**: `scud-cli/src/commands/spawn/terminal.rs`
**Changes**: Add helper function after `tmux_window_exists()` (around line 650)

```rust
/// Check if a tmux pane shows a shell prompt (indicating process completed/crashed)
pub fn tmux_pane_shows_prompt(session_name: &str, window_name: &str) -> bool {
    let window_target = format!("{}:{}", session_name, window_name);

    // Capture last line of pane content
    let output = std::process::Command::new("tmux")
        .args(["capture-pane", "-t", &window_target, "-p", "-S", "-1"])
        .output();

    let Ok(output) = output else {
        return false;
    };

    if !output.status.success() {
        return false;
    }

    let last_line = String::from_utf8_lossy(&output.stdout);
    let last_line = last_line.trim();

    // Common shell prompt patterns
    // These indicate the agent process has exited and we're back at shell
    let prompt_patterns = [
        "$ ",           // bash default
        "% ",           // zsh default
        "> ",           // fish, some custom prompts
        "# ",           // root shell
        "❯ ",           // starship, some modern prompts
        "→ ",           // some custom prompts
    ];

    // Check if line ends with a prompt pattern
    for pattern in prompt_patterns {
        if last_line.ends_with(pattern) || last_line.ends_with(pattern.trim()) {
            return true;
        }
    }

    // Also check for common prompt formats: user@host, (env), etc followed by prompt
    if last_line.contains('@') && (last_line.ends_with('$') || last_line.ends_with('%') || last_line.ends_with('>')) {
        return true;
    }

    false
}
```

#### 3.2 Add Idle Timeout Failure Detection

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: In `wait_for_round_completion()`, add idle failure logic after the heartbeat check (around line 1660)

Add this block after the heartbeat check section (after line 1660, before the print status line section):

```rust
        // Idle timeout failure detection: if agent has been idle AND shows shell prompt
        let idle_timeout = Duration::from_secs(300); // 5 minutes idle threshold
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
                        &writer.session_id(),
                        task_id,
                        events::EventKind::Failed {
                            reason: "agent idle with shell prompt (process crashed)".to_string(),
                        },
                    );
                    let _ = writer.write(&event);
                }
            }
        }
```

#### 3.3 Make Idle Timeout Configurable (Optional Enhancement)

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add `--idle-timeout-minutes` flag to SwarmArgs struct and pass through

In `SwarmArgs` struct, add:
```rust
    /// Minutes of inactivity before marking idle agent as failed (default: 5)
    #[arg(long, default_value = "5")]
    pub idle_timeout_minutes: u64,
```

Update the idle timeout calculation:
```rust
let idle_timeout = Duration::from_secs(idle_timeout_minutes * 60);
```

### Success Criteria:

#### Automated Verification:
- [ ] Build passes: `cargo build -p scud-cli`
- [ ] Linting passes: `cargo clippy -p scud-cli`
- [ ] Unit tests pass: `cargo test -p scud-cli`

#### Manual Verification:
- [ ] Run swarm with a task
- [ ] Kill the agent process (Ctrl+C in the tmux window)
- [ ] Observe shell prompt appears in tmux
- [ ] Wait 5+ minutes
- [ ] Observe task gets auto-marked as `Failed`
- [ ] Observe proper event emitted

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 4: Documentation and Testing

### Overview
Add documentation and integration tests for the new behavior.

### Changes Required:

#### 4.1 Update Orchestrator Documentation

**File**: `docs/orchestrator.md`
**Changes**: Add section about batch repair and health checking

```markdown
### Batch Repair

When backpressure validation fails after a wave, SCUD spawns a single "batch repair"
agent instead of one agent per failing task. This agent:

- Receives context about ALL responsible tasks at once
- Can analyze related failures together
- Iterates internally to fix issues systematically
- Signals completion via `.scud/batch-repair-complete` marker file

### Agent Health Checking

During wave execution, SCUD monitors agent health:

1. **Orphan Detection** (every 30s): Tasks whose tmux windows disappear are marked `Failed`
2. **Idle Detection** (configurable, default 5min): Tasks whose agents show shell prompts
   (indicating process exit) after being idle are marked `Failed`
3. **Stale Timeout** (optional `--stale-timeout-minutes`): Tasks exceeding the timeout
   with no tmux window are reset to `Pending`

This prevents tasks from being stuck as `InProgress` when agents crash.
```

### Success Criteria:

#### Automated Verification:
- [ ] Documentation builds/renders correctly
- [ ] No broken links

#### Manual Verification:
- [ ] Documentation accurately describes behavior
- [ ] Examples are clear and correct

---

## Testing Strategy

### Unit Tests:
- `generate_batch_repair_prompt()` produces expected format
- `parse_task_list()` correctly parses marker file content
- `tmux_pane_shows_prompt()` recognizes common prompt patterns

### Integration Tests:
- Batch repair flow: multiple tasks fail, single agent spawned
- Idle detection: crashed agent detected and task failed
- Mixed scenarios: some tasks complete, some crash

### Manual Testing Steps:
1. Create test tasks that will cause build failures
2. Run `scud swarm --tag test`
3. Verify single batch repair window spawns
4. In separate test: spawn task, kill agent process
5. Wait 5+ minutes, verify task marked failed
6. Verify events show correct failure reasons

## Performance Considerations

- Batch repair reduces tmux window spawning overhead
- Shell prompt detection adds one `tmux capture-pane` call per idle task per poll
- Idle check only runs for tasks already identified as idle (>60s), not all tasks

## Migration Notes

- Existing `.scud/repair-complete-{task_id}` marker files will still work
- New `.scud/batch-repair-complete` file for batch mode
- No breaking changes to CLI interface
- Legacy per-task repair code preserved as fallback

## References

- Current repair loop: `scud-cli/src/commands/swarm/mod.rs:1937-2119`
- Review prompt pattern: `scud-cli/src/commands/spawn/agent.rs:240-305`
- Health check loop: `scud-cli/src/commands/swarm/mod.rs:1487-1714`
- Doctor diagnostics: `scud-cli/src/commands/doctor.rs`
