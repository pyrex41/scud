# PR #6 Swarm Mode Fixes Implementation Plan

## Overview

This plan addresses five issues identified during code review of PR #6 (Swarm mode - wave-based parallel execution). The fixes improve reliability and correctness without changing the feature's core functionality.

## Current State Analysis

PR #6 adds `scud swarm` command for wave-based parallel task execution with backpressure validation. The code is well-structured but has several issues:

1. **Timeout not implemented** - `backpressure.rs:211` accepts timeout but ignores it
2. **Naive command parsing** - `backpressure.rs:213` splits on whitespace, breaks quoted args
3. **Unused parameter** - `mod.rs:293` has dead `_all_tasks_flat` parameter
4. **Incomplete git diff** - `mod.rs:506` only gets last commit, misses multi-commit waves
5. **No session locking** - Multiple swarm instances can run concurrently

### Key Discoveries:
- Project uses `edition = "2021"` and Rust 1.73+ features (`div_ceil`)
- Already has `fs2` crate for file locking (used elsewhere)
- Uses `std::process::Command` for subprocess execution

## Desired End State

After implementation:
- Commands respect configured timeout (default 5 minutes)
- Shell commands are executed properly via `sh -c` for correct parsing
- Dead code is removed
- Changed files correctly captured across all wave commits
- Only one swarm session can run per tag at a time

## What We're NOT Doing

- Adding shell parsing library (overkill - `sh -c` is sufficient)
- Session cleanup/garbage collection (separate concern)
- Retry logic for failed spawns (separate feature)

## Implementation Approach

All fixes are isolated to 2 files (`backpressure.rs` and `mod.rs`), making this a low-risk change.

---

## Phase 1: Implement Command Timeout

### Overview
Add proper timeout handling to prevent hung commands from blocking swarm execution indefinitely.

### Changes Required:

#### 1.1 Add wait_timeout support

**File**: `scud-cli/src/commands/swarm/backpressure.rs`
**Changes**: Implement timeout using `std::process::Child::wait` with timeout loop

```rust
use std::time::{Duration, Instant};

/// Run a single command with timeout
fn run_command(working_dir: &Path, cmd_str: &str, timeout_secs: u64) -> Result<(i32, String, String)> {
    use std::io::Read;
    use std::process::Stdio;

    // Use shell to handle complex commands (quoted args, pipes, etc.)
    let mut child = Command::new("sh")
        .args(["-c", cmd_str])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    // Poll for completion with timeout
    loop {
        match child.try_wait()? {
            Some(status) => {
                // Process completed
                let mut stdout = String::new();
                let mut stderr = String::new();

                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut stderr);
                }

                let exit_code = status.code().unwrap_or(-1);
                return Ok((exit_code, stdout, stderr));
            }
            None => {
                // Still running
                if start.elapsed() > timeout {
                    // Kill the process
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!("Command timed out after {} seconds", timeout_secs);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` compiles without warnings
- [ ] `cargo test -p scud-cli backpressure` passes
- [ ] `cargo clippy -p scud-cli` has no new warnings

#### Manual Verification:
- [ ] Test with a slow command to verify timeout works

---

## Phase 2: Fix Shell Command Parsing

### Overview
The current implementation splits on whitespace which breaks commands like `npm run "build:dev"` or commands with special characters. Using `sh -c` handles all shell syntax correctly.

### Changes Required:

Already addressed in Phase 1 - the `sh -c` approach handles:
- Quoted arguments: `npm run "build:dev"`
- Pipes: `cargo build 2>&1 | head`
- Environment variables: `NODE_ENV=production npm test`
- Complex commands: `cargo test -- --nocapture`

### Additional Test:

**File**: `scud-cli/src/commands/swarm/backpressure.rs`
**Changes**: Add test for complex command parsing

```rust
#[test]
fn test_run_command_with_quotes() {
    let tmp = TempDir::new().unwrap();
    // Create a test script that checks arguments
    let script = tmp.path().join("test.sh");
    std::fs::write(&script, "#!/bin/sh\necho \"$1\"").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let result = run_command(tmp.path(), "echo 'hello world'", 10);
    assert!(result.is_ok());
    let (code, stdout, _) = result.unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("hello world"));
}

#[test]
fn test_run_command_timeout() {
    let tmp = TempDir::new().unwrap();
    let result = run_command(tmp.path(), "sleep 10", 1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timed out"));
}
```

### Success Criteria:

#### Automated Verification:
- [ ] New tests pass: `cargo test -p scud-cli backpressure`
- [ ] Complex commands work in practice

---

## Phase 3: Remove Dead Code

### Overview
Remove unused `_all_tasks_flat` parameter from `compute_waves_from_tasks`.

### Changes Required:

#### 3.1 Update function signature

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Remove unused parameter

Before (line 291-296):
```rust
fn compute_waves_from_tasks<'a>(
    all_phases: &'a HashMap<String, Phase>,
    _all_tasks_flat: &[&Task],
    phase_tag: &str,
    all_tags: bool,
) -> Result<Vec<Vec<TaskInfo<'a>>>> {
```

After:
```rust
fn compute_waves_from_tasks<'a>(
    all_phases: &'a HashMap<String, Phase>,
    phase_tag: &str,
    all_tags: bool,
) -> Result<Vec<Vec<TaskInfo<'a>>>> {
```

#### 3.2 Update call sites

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Remove the argument at both call sites

Line 134:
```rust
// Before
let waves = compute_waves_from_tasks(&all_phases, &all_tasks_flat, &phase_tag, all_tags)?;
// After
let waves = compute_waves_from_tasks(&all_phases, &phase_tag, all_tags)?;
```

Line 527:
```rust
// Before
let waves = compute_waves_from_tasks(&all_phases, &all_tasks_flat, phase_tag, all_tags)?;
// After
let waves = compute_waves_from_tasks(&all_phases, phase_tag, all_tags)?;
```

#### 3.3 Remove unused variable

Line 131 can be removed if `all_tasks_flat` is no longer needed:
```rust
// Remove this line if not used elsewhere
let all_tasks_flat = flatten_all_tasks(&all_phases);
```

Check if it's used elsewhere in the function - if not, remove the binding entirely.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` compiles without unused variable warnings
- [ ] `cargo clippy -p scud-cli` passes

---

## Phase 4: Fix Git Diff Range for Changed Files

### Overview
The current implementation uses `HEAD~1..HEAD` which only captures the last commit. If a wave results in multiple commits, earlier files are missed.

### Changes Required:

#### 4.1 Track wave start commit

**File**: `scud-cli/src/commands/swarm/session.rs`
**Changes**: Add start_commit field to WaveState

```rust
/// State of a single wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveState {
    /// Wave number (1-indexed)
    pub wave_number: usize,
    /// Git commit SHA at wave start
    pub start_commit: Option<String>,
    /// Rounds executed in this wave
    pub rounds: Vec<RoundState>,
    // ... rest unchanged
}

impl WaveState {
    pub fn new(wave_number: usize) -> Self {
        Self {
            wave_number,
            start_commit: get_current_commit(),
            rounds: Vec::new(),
            // ... rest
        }
    }
}

fn get_current_commit() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}
```

#### 4.2 Update collect_changed_files to use commit range

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Accept start commit parameter

```rust
fn collect_changed_files(working_dir: &std::path::Path, start_commit: Option<&str>) -> Result<Vec<String>> {
    use std::process::Command;

    let args = match start_commit {
        Some(commit) => vec!["diff", "--name-only", &format!("{}..HEAD", commit)],
        None => vec!["diff", "--name-only", "HEAD~1..HEAD"],
    };

    let output = Command::new("git")
        .current_dir(working_dir)
        .args(&args)
        .output()?;

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    Ok(files)
}
```

#### 4.3 Update call site

**File**: `scud-cli/src/commands/swarm/mod.rs`
Line ~254:
```rust
// Before
files_changed: collect_changed_files(&working_dir).unwrap_or_default(),
// After
files_changed: collect_changed_files(&working_dir, wave_state.start_commit.as_deref()).unwrap_or_default(),
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` compiles
- [ ] Existing tests pass

#### Manual Verification:
- [ ] Run swarm with tasks that create multiple commits and verify all changed files are captured

---

## Phase 5: Add Session Locking

### Overview
Prevent multiple swarm instances from running on the same tag simultaneously, which could cause race conditions on task status updates.

### Changes Required:

#### 5.1 Add lock file handling

**File**: `scud-cli/src/commands/swarm/session.rs`
**Changes**: Add lock file functions

```rust
use fs2::FileExt;
use std::fs::File;

/// Get the path to a session's lock file
pub fn lock_file_path(project_root: Option<&PathBuf>, session_name: &str) -> PathBuf {
    swarm_dir(project_root).join(format!("{}.lock", session_name))
}

/// Acquire an exclusive lock for a swarm session
/// Returns a file handle that holds the lock (dropped when session ends)
pub fn acquire_session_lock(project_root: Option<&PathBuf>, session_name: &str) -> Result<File> {
    let dir = swarm_dir(project_root);
    std::fs::create_dir_all(&dir)?;

    let lock_path = lock_file_path(project_root, session_name);
    let file = File::create(&lock_path)?;

    // Try to acquire exclusive lock (non-blocking)
    match file.try_lock_exclusive() {
        Ok(()) => Ok(file),
        Err(_) => anyhow::bail!(
            "Another swarm session '{}' is already running. \
             Use a different --session name or wait for it to complete.",
            session_name
        ),
    }
}

/// Release session lock (called automatically when File is dropped)
pub fn release_session_lock(lock_file: File) {
    let _ = lock_file.unlock();
    // File is dropped here, releasing the lock
}
```

#### 5.2 Use lock in swarm run

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Acquire lock at start, release at end

Near the top of `run()` function, after session_name is determined:
```rust
// Acquire session lock to prevent concurrent execution
let _session_lock = session::acquire_session_lock(project_root.as_ref(), &session_name)?;
// Lock is automatically released when _session_lock goes out of scope
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-cli` compiles
- [ ] Test that second instance fails to start with clear error message

#### Manual Verification:
- [ ] Start swarm in one terminal, try to start same session in another - should fail with helpful message

---

## Testing Strategy

### Unit Tests:
- Timeout test: command that sleeps longer than timeout
- Shell parsing test: command with quoted arguments
- Lock test: try to acquire same lock twice

### Integration Tests:
- Run swarm dry-run to verify wave computation still works
- Verify backpressure commands execute correctly

### Manual Testing Steps:
1. Run `scud swarm --tag test --dry-run` to verify no regressions
2. Test timeout with `[swarm.backpressure] commands = ["sleep 10"]` and 5 second timeout
3. Test shell parsing with command containing quotes
4. Test locking by running two swarm instances simultaneously

## References

- PR #6: `claude/ralph-wiggum-scud-integration-IAtQG` branch
- `fs2` crate docs: https://docs.rs/fs2
- Backpressure file: `scud-cli/src/commands/swarm/backpressure.rs`
- Main swarm module: `scud-cli/src/commands/swarm/mod.rs`
- Session module: `scud-cli/src/commands/swarm/session.rs`
