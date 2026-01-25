# SCUD Eval Crate Implementation Plan

## Overview

Create a new `scud-eval` crate to measure and compare the effectiveness of different AI task execution modes: Swarm (parallel), Ralph (sequential), OpenCode variants, and direct Claude Code (single session baseline). The system will track metrics like wall-clock time, token usage, success rates, and code quality across standardized evaluation task sets.

## Current State Analysis

**Existing Infrastructure:**
- Swarm sessions already track: `started_at`, `completed_at` timestamps at session/wave/round level
- Validation results include `duration_secs` per command
- Spawn sessions track agent `started_at` and status
- No token/cost tracking (agents are external tmux processes)
- No git diff stats captured per task

**Execution Modes to Compare:**
| Mode | Description | Parallelism |
|------|-------------|-------------|
| `swarm-N` | Wave-based parallel execution | N agents per round |
| `ralph` | Sequential with fresh context per task | 1 agent at a time |
| `opencode-ralph` | Ralph using OpenCode harness | 1 agent at a time |
| `claude-direct` | Single session, auto-compact context | 1 continuous session |

## Desired End State

After implementation:
1. `scud eval run --mode swarm-4 --tasks eval-moderate` executes an eval run
2. Results stored in `~/.scud-eval/runs/<run-id>/` with metrics JSON
3. `scud eval compare <run-id-1> <run-id-2>` produces comparison report
4. Both synthetic (controlled) and real (external validity) task sets available
5. Token usage estimated via output parsing when available

### Verification:
- `scud eval list` shows available task sets and past runs
- `scud eval report <run-id>` generates markdown summary
- Metrics include: time, success rate, repair attempts, git diff stats

## What We're NOT Doing

- Real-time cost estimation during execution (post-hoc only)
- API proxy for exact token counting (too complex, output parsing sufficient)
- Automated quality scoring of code (manual review for now)
- CI/CD integration (future enhancement)
- Web dashboard (CLI reports only)

## Implementation Approach

The eval crate will:
1. Define eval task sets as SCUD-compatible task graphs (reusable `.scg` files)
2. Clone task sets to temporary tags for isolated runs
3. Invoke existing scud commands (swarm, ralph) with metrics wrappers
4. Collect results from session JSONs + git stats + parsed output
5. Store normalized results for comparison

## Phase 1: Core Eval Framework

### Overview
Create the `scud-eval` crate structure, define the metrics schema, and implement basic result storage.

### Changes Required:

#### 1.1 Create Crate Structure

**File**: `scud-eval/Cargo.toml`
```toml
[package]
name = "scud-eval"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Evaluation framework for SCUD execution modes"

[lib]
name = "scud_eval"
path = "src/lib.rs"

[[bin]]
name = "scud-eval"
path = "src/main.rs"

[dependencies]
scud-core = { path = "../scud-core" }
serde.workspace = true
serde_json = "1.0"
tokio.workspace = true
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4.5", features = ["derive"] }
uuid = { version = "1", features = ["v4"] }
dirs = "5.0"
tabled = "0.15"  # For CLI tables
```

**File**: `scud-eval/src/lib.rs`
```rust
pub mod config;
pub mod metrics;
pub mod runner;
pub mod storage;
pub mod tasksets;
pub mod comparison;
```

#### 1.2 Define Metrics Schema

**File**: `scud-eval/src/metrics.rs`
```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Metrics for a single task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub task_id: String,
    pub task_title: String,
    pub complexity: u32,

    // Timing
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<f64>,

    // Outcome
    pub success: bool,
    pub first_pass_success: bool,
    pub repair_attempts: u32,

    // Git stats (if available)
    pub lines_added: Option<u32>,
    pub lines_removed: Option<u32>,
    pub files_changed: Option<u32>,

    // Token estimates (parsed from output, may be None)
    pub tokens_input: Option<u64>,
    pub tokens_output: Option<u64>,
    pub estimated_cost_usd: Option<f64>,
}

/// Metrics for an entire eval run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRunMetrics {
    pub run_id: String,
    pub mode: ExecutionMode,
    pub taskset_name: String,
    pub harness: String,
    pub model: Option<String>,

    // Timing
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_duration_secs: Option<f64>,

    // Aggregate outcomes
    pub total_tasks: u32,
    pub tasks_succeeded: u32,
    pub tasks_failed: u32,
    pub first_pass_success_rate: f64,
    pub total_repair_attempts: u32,

    // Aggregate git stats
    pub total_lines_added: u32,
    pub total_lines_removed: u32,
    pub total_files_changed: u32,

    // Aggregate tokens (sum of non-None values)
    pub total_tokens_input: Option<u64>,
    pub total_tokens_output: Option<u64>,
    pub total_estimated_cost_usd: Option<f64>,

    // Per-task breakdown
    pub task_metrics: Vec<TaskMetrics>,

    // Validation results
    pub validation_commands: Vec<ValidationMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetrics {
    pub command: String,
    pub passed: bool,
    pub duration_secs: f64,
    pub run_count: u32,  // How many times this command was run
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    Swarm { round_size: usize },
    Ralph,
    ClaudeDirect,  // Single session baseline
}

impl ExecutionMode {
    pub fn name(&self) -> String {
        match self {
            Self::Swarm { round_size } => format!("swarm-{}", round_size),
            Self::Ralph => "ralph".to_string(),
            Self::ClaudeDirect => "claude-direct".to_string(),
        }
    }
}
```

#### 1.3 Implement Result Storage

**File**: `scud-eval/src/storage.rs`
```rust
use anyhow::Result;
use std::path::PathBuf;

/// Storage location: ~/.scud-eval/
pub fn eval_home() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".scud-eval")
}

pub fn runs_dir() -> PathBuf {
    eval_home().join("runs")
}

pub fn tasksets_dir() -> PathBuf {
    eval_home().join("tasksets")
}

pub fn run_dir(run_id: &str) -> PathBuf {
    runs_dir().join(run_id)
}

/// Save eval run results
pub fn save_run(metrics: &EvalRunMetrics) -> Result<PathBuf> {
    let dir = run_dir(&metrics.run_id);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("metrics.json");
    let json = serde_json::to_string_pretty(metrics)?;
    std::fs::write(&path, json)?;

    Ok(path)
}

/// Load eval run results
pub fn load_run(run_id: &str) -> Result<EvalRunMetrics> {
    let path = run_dir(run_id).join("metrics.json");
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

/// List all run IDs
pub fn list_runs() -> Result<Vec<String>> {
    let dir = runs_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut runs = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                runs.push(name.to_string());
            }
        }
    }
    runs.sort();
    Ok(runs)
}
```

#### 1.4 Add to Workspace

**File**: `Cargo.toml` (workspace root)
```toml
[workspace]
members = ["scud-cli", "scud-core", "descartes", "descartes-cli", "scud-eval"]
```

### Success Criteria:

#### Automated Verification:
- [ ] Crate compiles: `cargo build -p scud-eval`
- [ ] Tests pass: `cargo test -p scud-eval`
- [ ] Can serialize/deserialize metrics: unit test

#### Manual Verification:
- [ ] `~/.scud-eval/` directory structure is sensible
- [ ] JSON output is human-readable

---

## Phase 2: Eval Task Sets

### Overview
Create both synthetic (controlled) and real (external validity) task sets as portable `.scg` files.

### Changes Required:

#### 2.1 Task Set Definition

**File**: `scud-eval/src/tasksets.rs`
```rust
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSet {
    pub name: String,
    pub description: String,
    pub task_type: TaskSetType,
    pub scg_content: String,  // The actual SCG file content
    pub expected_files: Vec<String>,  // Files that should be created/modified
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskSetType {
    Synthetic,  // Controlled, designed for comparison
    Real,       // From actual project, external validity
}

/// Built-in task sets
pub fn builtin_tasksets() -> Vec<TaskSet> {
    vec![
        trivial_taskset(),
        moderate_taskset(),
        complex_taskset(),
        real_scud_taskset(),
    ]
}

fn trivial_taskset() -> TaskSet {
    TaskSet {
        name: "eval-trivial".to_string(),
        description: "5 independent complexity-1 tasks".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-trivial

@meta {
  name eval-trivial
  id_format sequential
}

@nodes
1 | Create hello.py with print statement | P | 1 | M
2 | Create goodbye.py with print statement | P | 1 | M
3 | Create utils.py with add function | P | 1 | M
4 | Create constants.py with PI constant | P | 1 | M
5 | Create README.md with project title | P | 1 | M

@edges

@details
1 | description |
  Create a file hello.py that prints "Hello, World!"
2 | description |
  Create a file goodbye.py that prints "Goodbye, World!"
3 | description |
  Create a file utils.py with a function add(a, b) that returns a + b
4 | description |
  Create a file constants.py that defines PI = 3.14159
5 | description |
  Create a README.md with a single H1 header "Eval Project"
"#.to_string(),
        expected_files: vec![
            "hello.py".to_string(),
            "goodbye.py".to_string(),
            "utils.py".to_string(),
            "constants.py".to_string(),
            "README.md".to_string(),
        ],
    }
}

fn moderate_taskset() -> TaskSet {
    TaskSet {
        name: "eval-moderate".to_string(),
        description: "5 tasks with dependencies, complexity 3-5".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-moderate

@meta {
  name eval-moderate
  id_format sequential
}

@nodes
1 | Create data models module | P | 3 | H
2 | Implement CRUD operations | P | 5 | H
3 | Add validation logic | P | 3 | M
4 | Create CLI interface | P | 5 | M
5 | Write unit tests | P | 3 | M

@edges
2 -> 1
3 -> 1
4 -> 2
5 -> 2
5 -> 3

@details
1 | description |
  Create models.py with User and Item dataclasses.
  User: id (int), name (str), email (str)
  Item: id (int), name (str), price (float), owner_id (int)
1 | test_strategy |
  Verify classes can be instantiated with valid data
2 | description |
  Create crud.py with in-memory storage and functions:
  - create_user(user: User) -> User
  - get_user(id: int) -> Optional[User]
  - create_item(item: Item) -> Item
  - get_items_by_owner(owner_id: int) -> List[Item]
2 | test_strategy |
  Test each CRUD operation with valid and invalid inputs
3 | description |
  Create validators.py with:
  - validate_email(email: str) -> bool (basic @ check)
  - validate_price(price: float) -> bool (must be positive)
  Add validation to crud.py create functions
3 | test_strategy |
  Test validators with valid and invalid inputs
4 | description |
  Create cli.py with argparse interface:
  - add-user --name NAME --email EMAIL
  - add-item --name NAME --price PRICE --owner OWNER_ID
  - list-items --owner OWNER_ID
4 | test_strategy |
  Test CLI commands work from command line
5 | description |
  Create test_all.py with pytest tests for:
  - Model instantiation
  - CRUD operations
  - Validators
  Run with: pytest test_all.py
5 | test_strategy |
  All tests should pass with pytest
"#.to_string(),
        expected_files: vec![
            "models.py".to_string(),
            "crud.py".to_string(),
            "validators.py".to_string(),
            "cli.py".to_string(),
            "test_all.py".to_string(),
        ],
    }
}

fn complex_taskset() -> TaskSet {
    TaskSet {
        name: "eval-complex".to_string(),
        description: "8 tasks with deep dependencies, complexity 5-13".to_string(),
        task_type: TaskSetType::Synthetic,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-complex

@meta {
  name eval-complex
  id_format sequential
}

@nodes
1 | Design database schema | P | 5 | C
2 | Implement ORM models | P | 8 | H
3 | Create repository layer | P | 8 | H
4 | Build service layer | P | 8 | H
5 | Implement REST API | P | 8 | H
6 | Add authentication | P | 13 | H
7 | Create integration tests | P | 8 | M
8 | Write API documentation | P | 5 | L

@edges
2 -> 1
3 -> 2
4 -> 3
5 -> 4
6 -> 5
7 -> 5
7 -> 6
8 -> 5

@details
1 | description |
  Create schema.sql with tables:
  - users (id, username, email, password_hash, created_at)
  - posts (id, user_id, title, content, created_at, updated_at)
  - comments (id, post_id, user_id, content, created_at)
  Include foreign keys and indexes
2 | description |
  Create models/ directory with SQLAlchemy models:
  - models/user.py - User model
  - models/post.py - Post model
  - models/comment.py - Comment model
  - models/__init__.py - exports all models
3 | description |
  Create repositories/ directory:
  - repositories/base.py - BaseRepository with CRUD
  - repositories/user.py - UserRepository
  - repositories/post.py - PostRepository
  - repositories/comment.py - CommentRepository
4 | description |
  Create services/ directory:
  - services/user.py - user registration, profile
  - services/post.py - create/edit/delete posts
  - services/comment.py - add/remove comments
5 | description |
  Create api/ directory with FastAPI routes:
  - api/users.py - /users endpoints
  - api/posts.py - /posts endpoints
  - api/comments.py - /comments endpoints
  - api/main.py - FastAPI app with routers
6 | description |
  Add authentication:
  - api/auth.py - /login, /register endpoints
  - auth/jwt.py - JWT token creation/validation
  - auth/dependencies.py - FastAPI auth dependencies
  Protect POST/PUT/DELETE endpoints
7 | description |
  Create tests/ directory:
  - tests/conftest.py - fixtures, test database
  - tests/test_users.py - user API tests
  - tests/test_posts.py - post API tests
  - tests/test_auth.py - authentication tests
8 | description |
  Create docs/ directory:
  - docs/api.md - API endpoint documentation
  - docs/setup.md - Installation and setup guide
"#.to_string(),
        expected_files: vec![
            "schema.sql".to_string(),
            "models/__init__.py".to_string(),
            "repositories/base.py".to_string(),
            "services/user.py".to_string(),
            "api/main.py".to_string(),
            "auth/jwt.py".to_string(),
            "tests/conftest.py".to_string(),
            "docs/api.md".to_string(),
        ],
    }
}

fn real_scud_taskset() -> TaskSet {
    // A real task set based on actual SCUD improvements
    TaskSet {
        name: "eval-real-scud".to_string(),
        description: "Real tasks: add summary stats to SCUD".to_string(),
        task_type: TaskSetType::Real,
        scg_content: r#"# SCUD Graph v1
# Phase: eval-real-scud

@meta {
  name eval-real-scud
  id_format sequential
}

@nodes
1 | Add task duration tracking | P | 5 | H
2 | Store completion timestamps | P | 3 | H
3 | Calculate average task duration | P | 3 | M
4 | Add stats subcommand | P | 5 | M
5 | Display duration in task list | P | 3 | L

@edges
2 -> 1
3 -> 2
4 -> 3
5 -> 2

@details
1 | description |
  Add started_at and completed_at fields to Task struct in scud-core/src/models/task.rs.
  Update SCG format to persist these timestamps.
2 | description |
  Update set-status command to record timestamps:
  - When status changes to in-progress, set started_at
  - When status changes to done, set completed_at
3 | description |
  Add duration calculation to PhaseStats:
  - average_duration_secs: Option<f64>
  - total_duration_secs: Option<f64>
  Calculate from tasks that have both timestamps
4 | description |
  Add 'scud stats' subcommand that displays:
  - Total tasks by status
  - Average completion time
  - Tasks completed today/this week
5 | description |
  Update 'scud list' to optionally show duration:
  - Add --show-duration flag
  - Display elapsed time for in-progress tasks
  - Display total time for completed tasks
"#.to_string(),
        expected_files: vec![
            "scud-core/src/models/task.rs".to_string(),
            "scud-cli/src/commands/set_status.rs".to_string(),
            "scud-cli/src/commands/stats.rs".to_string(),
            "scud-cli/src/commands/list.rs".to_string(),
        ],
    }
}

/// Install a taskset to ~/.scud-eval/tasksets/
pub fn install_taskset(taskset: &TaskSet) -> Result<PathBuf> {
    let dir = tasksets_dir().join(&taskset.name);
    std::fs::create_dir_all(&dir)?;

    // Write SCG file
    let scg_path = dir.join("tasks.scg");
    std::fs::write(&scg_path, &taskset.scg_content)?;

    // Write metadata
    let meta_path = dir.join("taskset.json");
    let meta = serde_json::to_string_pretty(taskset)?;
    std::fs::write(&meta_path, meta)?;

    Ok(dir)
}
```

#### 2.2 Task Set Cloning for Isolated Runs

**File**: `scud-eval/src/runner.rs` (partial - cloning logic)
```rust
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// Clone a taskset to a temporary project directory for isolated execution
pub fn setup_eval_workspace(taskset: &TaskSet, run_id: &str) -> Result<EvalWorkspace> {
    // Create temp directory for the eval run
    let workspace_dir = eval_home()
        .join("runs")
        .join(run_id)
        .join("workspace");
    std::fs::create_dir_all(&workspace_dir)?;

    // Initialize as git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&workspace_dir)
        .output()?;

    // Create .scud directory structure
    let scud_dir = workspace_dir.join(".scud");
    std::fs::create_dir_all(scud_dir.join("tasks"))?;

    // Write taskset SCG
    std::fs::write(
        scud_dir.join("tasks/tasks.scg"),
        &taskset.scg_content,
    )?;

    // Set active tag
    std::fs::write(
        scud_dir.join("active-tag"),
        &taskset.name,
    )?;

    // Initial commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&workspace_dir)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial eval workspace"])
        .current_dir(&workspace_dir)
        .output()?;

    Ok(EvalWorkspace {
        path: workspace_dir,
        taskset_name: taskset.name.clone(),
        run_id: run_id.to_string(),
    })
}

pub struct EvalWorkspace {
    pub path: PathBuf,
    pub taskset_name: String,
    pub run_id: String,
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo test -p scud-eval tasksets` - taskset generation works
- [ ] SCG content parses correctly with scud-core parser
- [ ] Workspace setup creates valid git repo

#### Manual Verification:
- [ ] Task sets represent meaningful evaluation scenarios
- [ ] Dependencies form sensible DAGs
- [ ] Complexity ratings are calibrated correctly

---

## Phase 3: Metrics Collection

### Overview
Add instrumentation to collect timing, git stats, and token estimates from execution.

### Changes Required:

#### 3.1 Session JSON Parser

**File**: `scud-eval/src/metrics/collector.rs`
```rust
use anyhow::Result;
use std::path::Path;
use chrono::{DateTime, Utc};

/// Collect metrics from a completed swarm session
pub fn collect_swarm_metrics(
    session_path: &Path,
    workspace: &Path,
) -> Result<Vec<TaskMetrics>> {
    let session: SwarmSessionJson = serde_json::from_str(
        &std::fs::read_to_string(session_path)?
    )?;

    let mut task_metrics = vec![];

    for wave in &session.waves {
        for round in &wave.rounds {
            for task_id in &round.task_ids {
                let git_stats = collect_git_stats_for_task(workspace, task_id)?;

                let started = parse_rfc3339(&round.started_at)?;
                let completed = round.completed_at.as_ref()
                    .map(|s| parse_rfc3339(s))
                    .transpose()?;

                let repair_count = wave.repairs
                    .iter()
                    .filter(|r| r.attributed_tasks.contains(task_id))
                    .count() as u32;

                let first_pass = wave.validation
                    .as_ref()
                    .map(|v| v.all_passed)
                    .unwrap_or(true);

                task_metrics.push(TaskMetrics {
                    task_id: task_id.clone(),
                    task_title: String::new(), // Fill from task storage
                    complexity: 0, // Fill from task storage
                    started_at: started,
                    completed_at: completed,
                    duration_secs: completed.map(|c| (c - started).num_seconds() as f64),
                    success: true, // Determined by final status
                    first_pass_success: first_pass && repair_count == 0,
                    repair_attempts: repair_count,
                    lines_added: git_stats.map(|g| g.additions),
                    lines_removed: git_stats.map(|g| g.deletions),
                    files_changed: git_stats.map(|g| g.files),
                    tokens_input: None,
                    tokens_output: None,
                    estimated_cost_usd: None,
                });
            }
        }
    }

    Ok(task_metrics)
}

/// Get git diff stats for changes attributed to a task
fn collect_git_stats_for_task(workspace: &Path, task_id: &str) -> Result<Option<GitStats>> {
    // Find commits with [task_id] prefix
    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "--all", &format!("--grep=[{}]", task_id)])
        .current_dir(workspace)
        .output()?;

    let commits: Vec<&str> = std::str::from_utf8(&output.stdout)?
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();

    if commits.is_empty() {
        return Ok(None);
    }

    let mut total = GitStats::default();
    for commit in commits {
        let stats = get_commit_stats(workspace, commit)?;
        total.additions += stats.additions;
        total.deletions += stats.deletions;
        total.files += stats.files;
    }

    Ok(Some(total))
}

#[derive(Default)]
struct GitStats {
    additions: u32,
    deletions: u32,
    files: u32,
}

fn get_commit_stats(workspace: &Path, commit: &str) -> Result<GitStats> {
    let output = std::process::Command::new("git")
        .args(["show", "--stat", "--format=", commit])
        .current_dir(workspace)
        .output()?;

    // Parse "X files changed, Y insertions(+), Z deletions(-)"
    let text = std::str::from_utf8(&output.stdout)?;
    // ... parsing logic ...
    Ok(GitStats::default())
}
```

#### 3.2 Token Estimation via Output Parsing

**File**: `scud-eval/src/metrics/tokens.rs`
```rust
use regex::Regex;

/// Attempt to parse token usage from captured agent output
///
/// Different CLIs report usage differently:
/// - Claude Code: May print "Tokens: X input, Y output"
/// - OpenCode: May print cost/token summaries
pub fn estimate_tokens_from_output(output: &str) -> Option<TokenEstimate> {
    // Claude Code pattern (hypothetical - check actual output)
    let claude_re = Regex::new(r"(?i)tokens?:\s*(\d+)\s*input.*?(\d+)\s*output").ok()?;
    if let Some(caps) = claude_re.captures(output) {
        return Some(TokenEstimate {
            input: caps.get(1)?.as_str().parse().ok()?,
            output: caps.get(2)?.as_str().parse().ok()?,
        });
    }

    // OpenCode pattern (hypothetical)
    let opencode_re = Regex::new(r"(?i)usage:\s*(\d+)\s*/\s*(\d+)").ok()?;
    if let Some(caps) = opencode_re.captures(output) {
        return Some(TokenEstimate {
            input: caps.get(1)?.as_str().parse().ok()?,
            output: caps.get(2)?.as_str().parse().ok()?,
        });
    }

    None
}

pub struct TokenEstimate {
    pub input: u64,
    pub output: u64,
}

/// Estimate cost based on model and token counts
pub fn estimate_cost(model: &str, tokens: &TokenEstimate) -> f64 {
    // Approximate pricing (update as needed)
    let (input_rate, output_rate) = match model {
        m if m.contains("opus") => (15.0 / 1_000_000.0, 75.0 / 1_000_000.0),
        m if m.contains("sonnet") => (3.0 / 1_000_000.0, 15.0 / 1_000_000.0),
        m if m.contains("haiku") => (0.25 / 1_000_000.0, 1.25 / 1_000_000.0),
        m if m.contains("grok") => (2.0 / 1_000_000.0, 10.0 / 1_000_000.0),
        _ => (5.0 / 1_000_000.0, 15.0 / 1_000_000.0), // Default estimate
    };

    (tokens.input as f64 * input_rate) + (tokens.output as f64 * output_rate)
}
```

#### 3.3 Tmux Output Capture Hook

To capture agent output for token parsing, we can add a post-completion hook that captures the full tmux pane history.

**File**: `scud-eval/src/metrics/capture.rs`
```rust
use anyhow::Result;
use std::path::Path;

/// Capture full tmux pane output for a task window
pub fn capture_agent_output(session_name: &str, task_id: &str) -> Result<String> {
    let window_name = format!("task-{}", task_id);

    let output = std::process::Command::new("tmux")
        .args([
            "capture-pane",
            "-t", &format!("{}:{}", session_name, window_name),
            "-p",
            "-S", "-",  // From start of history
        ])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Save captured output to run directory
pub fn save_agent_output(run_dir: &Path, task_id: &str, output: &str) -> Result<()> {
    let logs_dir = run_dir.join("agent_logs");
    std::fs::create_dir_all(&logs_dir)?;
    std::fs::write(logs_dir.join(format!("{}.log", task_id)), output)?;
    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Session JSON parsing extracts correct timing data
- [ ] Git stats calculation matches manual inspection
- [ ] Token regex patterns match sample outputs (if available)

#### Manual Verification:
- [ ] Metrics align with observed execution
- [ ] Git attribution correctly links commits to tasks

---

## Phase 4: Eval Runner

### Overview
Implement the CLI runner that orchestrates eval execution and collects results.

### Changes Required:

#### 4.1 Runner Implementation

**File**: `scud-eval/src/runner.rs` (complete)
```rust
use anyhow::Result;
use std::process::Command;

pub async fn run_eval(config: EvalConfig) -> Result<EvalRunMetrics> {
    let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let taskset = load_taskset(&config.taskset_name)?;

    // Setup isolated workspace
    let workspace = setup_eval_workspace(&taskset, &run_id)?;

    let started_at = chrono::Utc::now();

    // Execute based on mode
    match &config.mode {
        ExecutionMode::Swarm { round_size } => {
            run_swarm(&workspace.path, &taskset.name, *round_size, &config)?;
        }
        ExecutionMode::Ralph => {
            run_ralph(&workspace.path, &taskset.name, &config)?;
        }
        ExecutionMode::ClaudeDirect => {
            run_claude_direct(&workspace.path, &taskset, &config)?;
        }
    }

    let completed_at = chrono::Utc::now();

    // Collect metrics from session files
    let task_metrics = collect_metrics(&workspace, &config)?;

    // Aggregate into run metrics
    let metrics = aggregate_metrics(
        run_id,
        config,
        started_at,
        completed_at,
        task_metrics,
    );

    // Save results
    storage::save_run(&metrics)?;

    Ok(metrics)
}

fn run_swarm(workspace: &Path, tag: &str, round_size: usize, config: &EvalConfig) -> Result<()> {
    let mut cmd = Command::new("scud");
    cmd.current_dir(workspace)
        .args(["swarm", "--tag", tag])
        .args(["--round-size", &round_size.to_string()]);

    if let Some(harness) = &config.harness {
        cmd.args(["--harness", harness]);
    }
    if let Some(model) = &config.model {
        cmd.args(["--model", model]);
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Swarm execution failed");
    }
    Ok(())
}

fn run_ralph(workspace: &Path, tag: &str, config: &EvalConfig) -> Result<()> {
    let mut cmd = Command::new("scud");
    cmd.current_dir(workspace)
        .args(["ralph", "--tag", tag]);

    if let Some(harness) = &config.harness {
        cmd.args(["--harness", harness]);
    }
    if let Some(model) = &config.model {
        cmd.args(["--model", model]);
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Ralph execution failed");
    }
    Ok(())
}

fn run_claude_direct(workspace: &Path, taskset: &TaskSet, config: &EvalConfig) -> Result<()> {
    // Generate a prompt that asks Claude to complete all tasks sequentially
    // in a single session with auto-compaction enabled
    let prompt = generate_direct_prompt(taskset);

    // Write prompt to temp file
    let prompt_file = workspace.join("eval-prompt.txt");
    std::fs::write(&prompt_file, &prompt)?;

    // Execute Claude Code directly
    let mut cmd = Command::new("claude");
    cmd.current_dir(workspace)
        .arg(&format!("$(cat '{}')", prompt_file.display()))
        .arg("--dangerously-skip-permissions");

    if let Some(model) = &config.model {
        cmd.args(["--model", model]);
    }

    // This runs in a single session until all tasks complete
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Claude direct execution failed");
    }
    Ok(())
}

fn generate_direct_prompt(taskset: &TaskSet) -> String {
    format!(r#"You are completing a series of coding tasks for an evaluation benchmark.

TASKSET: {}
DESCRIPTION: {}

TASKS TO COMPLETE (in dependency order):
{}

INSTRUCTIONS:
1. Complete each task in order, respecting dependencies
2. After completing each task, commit your changes with message: [TASK-ID] description
3. Mark each task done by running: scud set-status <id> done
4. Continue until all tasks are marked done
5. Your context will auto-compact as needed - this is normal

Begin now. Start with the first task that has no dependencies.
"#,
        taskset.name,
        taskset.description,
        taskset.scg_content,
    )
}
```

#### 4.2 CLI Interface

**File**: `scud-eval/src/main.rs`
```rust
use clap::{Parser, Subcommand};
use scud_eval::{runner, storage, comparison, tasksets};

#[derive(Parser)]
#[command(name = "scud-eval")]
#[command(about = "Evaluate SCUD execution modes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an evaluation
    Run {
        /// Execution mode: swarm-N, ralph, claude-direct
        #[arg(short, long)]
        mode: String,

        /// Task set name
        #[arg(short, long)]
        tasks: String,

        /// AI harness: claude or opencode
        #[arg(long, default_value = "claude")]
        harness: String,

        /// Model to use
        #[arg(long)]
        model: Option<String>,
    },

    /// List available task sets and past runs
    List {
        /// Show task sets instead of runs
        #[arg(long)]
        tasksets: bool,
    },

    /// Compare two or more eval runs
    Compare {
        /// Run IDs to compare
        run_ids: Vec<String>,
    },

    /// Generate report for a run
    Report {
        /// Run ID
        run_id: String,

        /// Output format: text, json, markdown
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Install built-in task sets
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { mode, tasks, harness, model } => {
            let config = parse_mode_config(&mode, &tasks, &harness, model)?;
            let metrics = runner::run_eval(config).await?;
            println!("Eval run complete: {}", metrics.run_id);
            println!("Results: ~/.scud-eval/runs/{}/", metrics.run_id);
        }

        Commands::List { tasksets: show_tasksets } => {
            if show_tasksets {
                for ts in tasksets::builtin_tasksets() {
                    println!("{}: {} ({})", ts.name, ts.description,
                        match ts.task_type {
                            tasksets::TaskSetType::Synthetic => "synthetic",
                            tasksets::TaskSetType::Real => "real",
                        });
                }
            } else {
                for run_id in storage::list_runs()? {
                    let run = storage::load_run(&run_id)?;
                    println!("{}: {} on {} ({} tasks, {:.1}s)",
                        run_id,
                        run.mode.name(),
                        run.taskset_name,
                        run.total_tasks,
                        run.total_duration_secs.unwrap_or(0.0),
                    );
                }
            }
        }

        Commands::Compare { run_ids } => {
            let runs: Vec<_> = run_ids.iter()
                .map(|id| storage::load_run(id))
                .collect::<Result<_, _>>()?;
            comparison::print_comparison(&runs)?;
        }

        Commands::Report { run_id, format } => {
            let run = storage::load_run(&run_id)?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&run)?),
                "markdown" => println!("{}", comparison::to_markdown(&run)?),
                _ => comparison::print_report(&run)?,
            }
        }

        Commands::Init => {
            for ts in tasksets::builtin_tasksets() {
                let path = tasksets::install_taskset(&ts)?;
                println!("Installed: {} -> {}", ts.name, path.display());
            }
        }
    }

    Ok(())
}
```

#### 4.3 Comparison Module

**File**: `scud-eval/src/comparison.rs`
```rust
use crate::metrics::EvalRunMetrics;
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct ComparisonRow {
    metric: String,
    #[tabled(rename = "Run 1")]
    run1: String,
    #[tabled(rename = "Run 2")]
    run2: String,
    delta: String,
}

pub fn print_comparison(runs: &[EvalRunMetrics]) -> anyhow::Result<()> {
    if runs.len() < 2 {
        anyhow::bail!("Need at least 2 runs to compare");
    }

    let r1 = &runs[0];
    let r2 = &runs[1];

    let rows = vec![
        ComparisonRow {
            metric: "Mode".to_string(),
            run1: r1.mode.name(),
            run2: r2.mode.name(),
            delta: "-".to_string(),
        },
        ComparisonRow {
            metric: "Task Set".to_string(),
            run1: r1.taskset_name.clone(),
            run2: r2.taskset_name.clone(),
            delta: "-".to_string(),
        },
        ComparisonRow {
            metric: "Total Time (s)".to_string(),
            run1: format!("{:.1}", r1.total_duration_secs.unwrap_or(0.0)),
            run2: format!("{:.1}", r2.total_duration_secs.unwrap_or(0.0)),
            delta: format_delta(r1.total_duration_secs, r2.total_duration_secs),
        },
        ComparisonRow {
            metric: "Success Rate".to_string(),
            run1: format!("{:.1}%", r1.tasks_succeeded as f64 / r1.total_tasks as f64 * 100.0),
            run2: format!("{:.1}%", r2.tasks_succeeded as f64 / r2.total_tasks as f64 * 100.0),
            delta: "-".to_string(),
        },
        ComparisonRow {
            metric: "First-Pass Rate".to_string(),
            run1: format!("{:.1}%", r1.first_pass_success_rate * 100.0),
            run2: format!("{:.1}%", r2.first_pass_success_rate * 100.0),
            delta: "-".to_string(),
        },
        ComparisonRow {
            metric: "Repair Attempts".to_string(),
            run1: r1.total_repair_attempts.to_string(),
            run2: r2.total_repair_attempts.to_string(),
            delta: format!("{:+}", r2.total_repair_attempts as i32 - r1.total_repair_attempts as i32),
        },
        ComparisonRow {
            metric: "Lines Changed".to_string(),
            run1: format!("+{} -{}", r1.total_lines_added, r1.total_lines_removed),
            run2: format!("+{} -{}", r2.total_lines_added, r2.total_lines_removed),
            delta: "-".to_string(),
        },
    ];

    let table = Table::new(rows).to_string();
    println!("{}", table);

    Ok(())
}

fn format_delta(a: Option<f64>, b: Option<f64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) => {
            let pct = (b - a) / a * 100.0;
            if pct > 0.0 {
                format!("+{:.1}%", pct)
            } else {
                format!("{:.1}%", pct)
            }
        }
        _ => "-".to_string(),
    }
}

pub fn print_report(run: &EvalRunMetrics) -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════");
    println!("SCUD Eval Report: {}", run.run_id);
    println!("═══════════════════════════════════════════════════════");
    println!();
    println!("Mode:        {}", run.mode.name());
    println!("Task Set:    {}", run.taskset_name);
    println!("Harness:     {}", run.harness);
    println!("Model:       {}", run.model.as_deref().unwrap_or("default"));
    println!();
    println!("─── Results ───────────────────────────────────────────");
    println!("Total Tasks:       {}", run.total_tasks);
    println!("Succeeded:         {} ({:.1}%)",
        run.tasks_succeeded,
        run.tasks_succeeded as f64 / run.total_tasks as f64 * 100.0);
    println!("Failed:            {}", run.tasks_failed);
    println!("First-Pass Rate:   {:.1}%", run.first_pass_success_rate * 100.0);
    println!("Repair Attempts:   {}", run.total_repair_attempts);
    println!();
    println!("─── Timing ────────────────────────────────────────────");
    println!("Total Duration:    {:.1}s", run.total_duration_secs.unwrap_or(0.0));
    println!("Avg per Task:      {:.1}s",
        run.total_duration_secs.unwrap_or(0.0) / run.total_tasks as f64);
    println!();
    println!("─── Git Stats ─────────────────────────────────────────");
    println!("Lines Added:       +{}", run.total_lines_added);
    println!("Lines Removed:     -{}", run.total_lines_removed);
    println!("Files Changed:     {}", run.total_files_changed);

    if let Some(cost) = run.total_estimated_cost_usd {
        println!();
        println!("─── Cost Estimate ─────────────────────────────────────");
        println!("Estimated Cost:    ${:.4}", cost);
    }

    println!("═══════════════════════════════════════════════════════");

    Ok(())
}

pub fn to_markdown(run: &EvalRunMetrics) -> anyhow::Result<String> {
    Ok(format!(r#"# Eval Report: {}

## Configuration
- **Mode**: {}
- **Task Set**: {}
- **Harness**: {}
- **Model**: {}

## Results
| Metric | Value |
|--------|-------|
| Total Tasks | {} |
| Succeeded | {} ({:.1}%) |
| Failed | {} |
| First-Pass Rate | {:.1}% |
| Repair Attempts | {} |

## Timing
- **Total Duration**: {:.1}s
- **Avg per Task**: {:.1}s

## Git Stats
- Lines Added: +{}
- Lines Removed: -{}
- Files Changed: {}
"#,
        run.run_id,
        run.mode.name(),
        run.taskset_name,
        run.harness,
        run.model.as_deref().unwrap_or("default"),
        run.total_tasks,
        run.tasks_succeeded,
        run.tasks_succeeded as f64 / run.total_tasks as f64 * 100.0,
        run.tasks_failed,
        run.first_pass_success_rate * 100.0,
        run.total_repair_attempts,
        run.total_duration_secs.unwrap_or(0.0),
        run.total_duration_secs.unwrap_or(0.0) / run.total_tasks as f64,
        run.total_lines_added,
        run.total_lines_removed,
        run.total_files_changed,
    ))
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build -p scud-eval` compiles
- [ ] `scud-eval init` installs task sets
- [ ] `scud-eval list --tasksets` shows available sets
- [ ] Unit tests for comparison logic pass

#### Manual Verification:
- [ ] Full eval run completes with swarm mode
- [ ] Full eval run completes with ralph mode
- [ ] Full eval run completes with claude-direct mode
- [ ] Comparison output is readable and informative
- [ ] Results are reproducible across runs

---

## Testing Strategy

### Unit Tests:
- Metrics serialization/deserialization
- Git stats parsing
- Token estimation regex patterns
- Comparison calculations

### Integration Tests:
- End-to-end eval run with trivial taskset
- Session JSON parsing with real session files
- Workspace setup and teardown

### Manual Testing Steps:
1. Run `scud-eval init` and verify tasksets appear in `~/.scud-eval/tasksets/`
2. Run `scud-eval run --mode swarm-2 --tasks eval-trivial` and verify completion
3. Run `scud-eval run --mode ralph --tasks eval-trivial` and verify completion
4. Run `scud-eval compare <run1> <run2>` and verify sensible output
5. Inspect `~/.scud-eval/runs/<run-id>/metrics.json` for correctness

## Performance Considerations

- Eval runs are inherently slow (AI execution) - no optimization needed
- Task set SCG files are small - no streaming needed
- Results JSON is per-run - no pagination needed
- Git stats use single commits - O(commits) complexity is fine

## Migration Notes

N/A - new crate with no existing data to migrate.

## References

- Swarm session format: `scud-cli/src/commands/swarm/session.rs:209-277`
- Spawn session format: `scud-cli/src/commands/spawn/monitor.rs:33-80`
- SCG format: `scud-core/src/formats/scg.rs`
- Backpressure validation: `scud-cli/src/backpressure.rs:198-223`
