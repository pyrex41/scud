---
date: 2025-11-26T01:21:41Z
researcher: Claude
git_commit: 74f4ca86f499cf3a399e2ae1d833db5e868609d2
branch: master
repository: bmad-tm
topic: "SCUD CLI UX Analysis and Improvement Opportunities"
tags: [research, codebase, scud-cli, rust, ux, parallelization, command-consolidation]
status: complete
last_updated: 2025-11-25
last_updated_by: Claude
---

# Research: SCUD CLI UX Analysis and Improvement Opportunities

**Date**: 2025-11-26T01:21:41Z
**Researcher**: Claude
**Git Commit**: 74f4ca86f499cf3a399e2ae1d833db5e868609d2
**Branch**: master
**Repository**: bmad-tm

## Research Question

Review the scud-cli tool for UX improvement opportunities, specifically:
1. Merge `analyze-complexity` into `expand` (run analyze first, then expand)
2. Enable parallel task expansion
3. Replace `use-tag` hidden state with explicit `--tag` flags or interactive prompts
4. Identify similar patterns across the codebase

## Summary

The scud-cli is a Rust-based task management CLI that suffers from several UX anti-patterns:

1. **Hidden State Dependency**: 14+ commands require `active_epic` to be set via `use-tag`, creating a memorization burden
2. **Sequential Processing**: AI commands process tasks one-by-one despite being embarrassingly parallel
3. **Underutilized Interactivity**: `dialoguer` crate is included but only used in one location
4. **Command Fragmentation**: Several command pairs could be consolidated (`analyze-complexity`/`expand`, `tags`/`use-tag`, group commands)
5. **Inconsistent Retry Logic**: Only `expand` has retry logic; other AI commands fail immediately

---

## Detailed Findings

### 1. Hidden State Pattern: `active_epic`

The CLI requires users to run `scud use-tag <tag>` before most task operations. This creates invisible state that users must remember.

#### How It Works

**State Location**: `.taskmaster/workflow-state.json`
- Field: `WorkflowState.active_epic: Option<String>`
- Code: `scud-cli/src/models/workflow.rs:67`
- Cache: In-memory `RwLock<Option<Option<String>>>` at `scud-cli/src/storage/mod.rs:18`

**Commands Requiring Active Epic** (14 total):
| Command | File | Line |
|---------|------|------|
| `list` | `commands/list.rs` | 12 |
| `show` | `commands/show.rs` | 11 |
| `stats` | `commands/stats.rs` | 11 |
| `next` | `commands/next.rs` | 11 |
| `set-status` | `commands/set_status.rs` | 22 |
| `assign` | `commands/assign.rs` | 11 |
| `claim` | `commands/claim.rs` | 11 |
| `release` | `commands/release.rs` | 11 |
| `analyze-complexity` | `commands/ai/analyze_complexity.rs` | 20 |
| `expand` | `commands/ai/expand.rs` | 29 |

**Error Message**: `"No active epic. Run: scud use-tag <epic-tag>"`

#### Suggested Changes

**Option A: Explicit `--tag` flag on all commands**
```rust
// Instead of requiring use-tag first
scud list                          // fails without active epic
scud use-tag phase1 && scud list   // current workflow

// Allow explicit tag
scud list --tag phase1             // works directly
scud expand --all --tag phase1     // works directly
```

**Option B: Interactive prompt when tag not specified**
```rust
// In commands/list.rs, instead of:
let active_epic = storage.get_active_epic()?
    .ok_or_else(|| anyhow::anyhow!("No active epic..."))?;

// Use:
let active_epic = match storage.get_active_epic()? {
    Some(tag) => tag,
    None => {
        let tags: Vec<_> = storage.load_tasks()?.keys().cloned().collect();
        Select::new()
            .with_prompt("Select epic")
            .items(&tags)
            .interact()?
    }
};
```

**Option C: Default to "all tags" for read operations**
```rust
// scud list with no active tag shows all tasks across all epics
// scud expand --all expands in all epics (or prompts to select)
```

---

### 2. Sequential Processing in AI Commands

Both `analyze-complexity` and `expand` process tasks sequentially despite having independent operations.

#### Current Implementation

**analyze_complexity.rs:47-85**:
```rust
for id in task_ids {
    // Each iteration awaits LLM call (2-5 seconds each)
    let analysis: ComplexityAnalysis = client.complete_json(&prompt).await?;
    task.complexity = analysis.complexity;
}
```

**expand.rs:61-210**:
```rust
for id in task_ids {
    // Each iteration awaits LLM call with retry logic
    for attempt in 1..=3 {
        match client.complete_json::<Vec<ExpandedTask>>(&prompt).await {
            // ...
        }
    }
    // Then mutates epic.add_task() for each subtask
}
```

#### Performance Impact

| Tasks | Sequential Time | Parallel Time (est.) |
|-------|-----------------|---------------------|
| 5 | 15-25 sec | 3-5 sec |
| 10 | 30-50 sec | 3-5 sec |
| 20 | 60-100 sec | 5-10 sec |

#### Suggested Changes

**For analyze-complexity** (fully parallelizable):
```rust
use futures::future::join_all;

let tasks_to_analyze: Vec<_> = task_ids.iter()
    .map(|id| {
        let client = &client;
        let epic = &epic;
        async move {
            let task = epic.get_task(id)?;
            let prompt = Prompts::analyze_complexity(...);
            let analysis = client.complete_json(&prompt).await?;
            Ok::<_, anyhow::Error>((id.clone(), analysis))
        }
    })
    .collect();

let results = join_all(tasks_to_analyze).await;

// Apply results sequentially (fast, in-memory)
for result in results {
    let (id, analysis) = result?;
    epic.get_task_mut(&id).unwrap().complexity = analysis.complexity;
}
```

**For expand** (parallel LLM calls, sequential mutations):
```rust
// Phase 1: Parallel LLM calls
let expansion_results = join_all(
    task_ids.iter().map(|id| expand_task_async(&client, id))
).await;

// Phase 2: Sequential mutations (fast)
for (id, subtasks) in expansion_results {
    for subtask in subtasks {
        epic.add_task(subtask);
    }
}
```

---

### 3. Merge `analyze-complexity` into `expand`

These commands are always run together: analyze first, then expand tasks with high complexity.

#### Current Workflow
```bash
scud analyze-complexity       # Analyzes all tasks, suggests expansion
# Output: "Run: scud expand --all"
scud expand --all            # Expands high-complexity tasks
```

#### Current Code Flow

**analyze_complexity.rs:107-119** (after analysis):
```rust
if !tasks_needing_expansion.is_empty() {
    println!("{} {} task(s) with complexity >13:", tasks_needing_expansion.len());
    // ...
    println!("{}", "Run: scud expand --all".blue());
}
```

**expand.rs:42-46**:
```rust
// expand --all filters by needs_expansion()
epic.tasks.iter()
    .filter(|t| t.needs_expansion())  // complexity >= 3 and not already expanded
    .map(|t| t.id.clone())
    .collect()
```

#### Suggested Changes

**Option A: Add `--auto-expand` flag to analyze-complexity**
```rust
// main.rs
AnalyzeComplexity {
    #[arg(short, long)]
    task: Option<String>,

    #[arg(long)]
    auto_expand: bool,  // NEW: automatically expand after analysis
}

// analyze_complexity.rs
if args.auto_expand && !tasks_needing_expansion.is_empty() {
    expand::run_internal(project_root, None, true).await?;
}
```

**Option B: Make expand auto-analyze if needed**
```rust
// expand.rs - before expansion
for id in &task_ids {
    let task = epic.get_task(id)?;
    if task.complexity == 0 {
        // Task hasn't been analyzed yet - analyze it first
        let analysis = client.complete_json(&Prompts::analyze_complexity(...)).await?;
        epic.get_task_mut(id).unwrap().complexity = analysis.complexity;
    }
}
```

**Option C: Remove analyze-complexity, integrate into expand**
```rust
// New expand behavior:
// 1. If task has complexity 0, analyze it first
// 2. If complexity >= threshold, expand it
// 3. All in one command

Expand {
    task_id: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long, default_value = "13")]
    threshold: u32,  // Complexity threshold for auto-expansion
}
```

---

### 4. Interactive Prompts (Underutilized)

The `dialoguer` crate is in `Cargo.toml:31` but only used once.

#### Current Usage

**Only in init.rs:42-46**:
```rust
let selection = Select::new()
    .with_prompt("Select your LLM provider")
    .items(&providers)
    .default(0)
    .interact()?;
```

#### High-Value Opportunities

| Command | Current Behavior | Interactive Alternative |
|---------|-----------------|------------------------|
| `tags` + `use-tag` | Two commands | `tags` shows list, prompts for selection |
| `set-status` | Requires exact string | Select from valid statuses |
| `expand` (no args) | Fails with error | MultiSelect tasks to expand |
| `create-group --epics` | Comma-separated string | MultiSelect from available epics |
| `show` (no task) | Fails with error | Select from task list |

#### Example Implementation

**tags.rs with interactive selection**:
```rust
pub fn run(project_root: Option<PathBuf>, set_tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let tasks = storage.load_tasks()?;
    let tags: Vec<_> = tasks.keys().collect();

    // If tag provided, set it directly
    if let Some(tag) = set_tag {
        return storage.set_active_epic(tag);
    }

    // Display tags with stats
    for tag in &tags {
        // ... display logic
    }

    // Interactive selection
    let selection = Select::new()
        .with_prompt("Select epic to activate (or Ctrl+C to cancel)")
        .items(&tags)
        .interact_opt()?;

    if let Some(idx) = selection {
        storage.set_active_epic(tags[idx])?;
    }

    Ok(())
}
```

---

### 5. Command Consolidation Opportunities

#### High Priority

**A. `tags` + `use-tag` → Single `tags` command**
```rust
// Current
Tags,                           // List tags
UseTag { tag: String },         // Set active tag

// Proposed
Tags {
    /// Optional tag to set as active
    tag: Option<String>,
}
// scud tags           -> lists tags, prompts for selection
// scud tags phase1    -> sets active tag directly
```

**B. `assign` + `claim` → Single `assign` command**
```rust
// Current
Assign { task_id: String, assignee: String },
Claim { task_id: String, name: String },

// Proposed
Assign {
    task_id: String,
    assignee: Option<String>,
    #[arg(long)]
    self_claim: bool,   // Claims for yourself
    #[arg(long)]
    name: Option<String>,  // Your name when self-claiming
}
// scud assign 5 alice      -> assigns to alice
// scud assign 5 --self     -> claims for yourself (prompts for name if not set)
```

**C. Group commands → Subcommand pattern**
```rust
// Current: 4 separate commands
CreateGroup, ListGroups, GroupStatus, AddToGroup

// Proposed: Single command with subcommands
Group {
    #[command(subcommand)]
    command: GroupCommands,
}

enum GroupCommands {
    Create { name: String, epics: String, description: Option<String> },
    List,
    Status { group_id: String },
    Add { group_id: String, epic_tag: String },
}
// scud group create "Phase 1" --epics auth,api
// scud group list
// scud group status phase-1
// scud group add phase-1 frontend
```

#### Medium Priority

**D. `list` + `stats` → Combined view option**
```rust
List {
    #[arg(short, long)]
    status: Option<String>,
    #[arg(long)]
    stats: bool,  // NEW: show stats header
}
// scud list --stats    -> shows stats then task list
```

---

### 6. LLM Client Architecture

#### Current State

- **No batching**: Each task = separate HTTP request
- **No parallelism**: Sequential `for` loop with `.await`
- **Inconsistent retry**: Only `expand.rs` has retry logic (3 attempts, 1s delay)
- **No rate limiting**: Could overwhelm API with parallel requests

#### Key Files

| File | Purpose | Lines |
|------|---------|-------|
| `llm/client.rs` | HTTP client, provider routing | 318 |
| `llm/prompts.rs` | Static prompt templates | 167 |
| `commands/ai/analyze_complexity.rs` | Sequential analysis | 123 |
| `commands/ai/expand.rs` | Sequential expansion with retry | 223 |

#### Suggested Architecture

```rust
// New: Parallel executor with rate limiting
pub struct LLMExecutor {
    client: LLMClient,
    semaphore: Semaphore,  // Limit concurrent requests
    retry_config: RetryConfig,
}

impl LLMExecutor {
    pub async fn execute_batch<T, F>(&self, tasks: Vec<F>) -> Vec<Result<T>>
    where
        F: Future<Output = Result<T>>,
    {
        let futures = tasks.into_iter().map(|task| {
            let permit = self.semaphore.acquire();
            async move {
                let _permit = permit.await;
                self.with_retry(task).await
            }
        });

        join_all(futures).await
    }
}
```

---

### 7. Promised But Missing Features

**create_group.rs:69-70** suggests:
```
scud list --group {group_id}
scud stats --group {group_id}
```

But these flags don't exist:
- `List` command only has `--status` (`main.rs:62-63`)
- `Stats` command has no parameters (`main.rs:84`)

Users following this guidance get "unexpected argument" errors.

---

## Code References

### Hidden State Management
- `scud-cli/src/storage/mod.rs:247-280` - `get_active_epic()`, `set_active_epic()`
- `scud-cli/src/models/workflow.rs:64-72` - `WorkflowState` struct
- `scud-cli/src/commands/use_tag.rs:7-23` - `use-tag` command

### Sequential Processing
- `scud-cli/src/commands/ai/analyze_complexity.rs:47-85` - Sequential analysis loop
- `scud-cli/src/commands/ai/expand.rs:61-210` - Sequential expansion loop

### Interactive Prompts
- `scud-cli/src/commands/init.rs:42-46` - Only dialoguer usage
- `scud-cli/Cargo.toml:31` - `dialoguer = "0.11"`

### Command Definitions
- `scud-cli/src/main.rs:41-190` - All command enum variants
- `scud-cli/src/commands/mod.rs:1-22` - Command module exports

### LLM Integration
- `scud-cli/src/llm/client.rs:62-256` - HTTP client
- `scud-cli/src/llm/prompts.rs:1-166` - Prompt templates

---

## Architecture Documentation

### Current Data Flow

```
User Command
    │
    ▼
main.rs (CLI parsing via clap)
    │
    ▼
commands/*.rs (command handlers)
    │
    ├──► storage/mod.rs (JSON file I/O with locking)
    │        │
    │        ▼
    │    .taskmaster/tasks.json
    │    .taskmaster/workflow-state.json
    │
    └──► llm/client.rs (AI operations)
             │
             ▼
         External LLM API (Anthropic, OpenAI, etc.)
```

### State Dependencies

```
                    ┌─────────────────┐
                    │   scud init     │
                    └────────┬────────┘
                             │ creates
                             ▼
              ┌──────────────────────────────┐
              │  .taskmaster/ directory      │
              │  - tasks.json                │
              │  - workflow-state.json       │
              │  - config.toml               │
              └──────────────┬───────────────┘
                             │
                             │ required by
                             ▼
              ┌──────────────────────────────┐
              │   scud parse-prd             │
              │   (creates epic, sets        │
              │    active_epic)              │
              └──────────────┬───────────────┘
                             │
                             │ sets active_epic
                             ▼
              ┌──────────────────────────────┐
              │   scud use-tag <tag>         │
              │   (switches active_epic)     │
              └──────────────┬───────────────┘
                             │
                             │ required by 14+ commands
                             ▼
    ┌────────────────────────────────────────────────────┐
    │  list, show, stats, next, set-status,             │
    │  assign, claim, release, analyze-complexity,       │
    │  expand                                            │
    └────────────────────────────────────────────────────┘
```

---

## Prioritized Recommendations

### Phase 1: Quick Wins (Low effort, high impact)

1. **Add `--tag` flag to all epic-dependent commands**
   - Backward compatible (flag is optional)
   - Removes need for `use-tag` in scripts
   - ~30 min per command

2. **Standardize retry logic**
   - Extract retry from `expand.rs` into `llm/client.rs`
   - Apply to all AI commands
   - ~2 hours

3. **Fix misleading help text**
   - Remove `--group` suggestions from `create_group.rs`
   - Or implement the missing flags
   - ~30 min

### Phase 2: UX Improvements (Medium effort)

4. **Add interactive prompts**
   - `tags` command prompts for selection
   - `set-status` shows status menu
   - `expand` (no args) shows task selection
   - ~4 hours total

5. **Merge `tags` + `use-tag`**
   - Single command with optional argument
   - Interactive selection when no argument
   - ~2 hours

6. **Consolidate group commands**
   - Convert to subcommand pattern
   - ~3 hours

### Phase 3: Performance (Higher effort)

7. **Parallelize AI commands**
   - `analyze-complexity`: Full parallelization
   - `expand`: Parallel LLM calls, sequential mutations
   - ~6-8 hours

8. **Merge analyze-complexity into expand**
   - Auto-analyze if complexity is 0
   - Remove separate command (deprecate first)
   - ~4 hours

9. **Add rate limiting to LLM executor**
   - Semaphore-based concurrency limit
   - Configurable via config.toml
   - ~4 hours

---

## Open Questions

1. **Backward compatibility**: Should `use-tag` be deprecated or kept alongside `--tag` flags?
2. **Default behavior**: When no tag specified and no active tag, should commands:
   - Fail with error (current)
   - Prompt interactively
   - Operate on all epics
3. **Rate limits**: What are the rate limits for each LLM provider? Should parallelization be configurable?
4. **Group features**: Should `--group` filtering be implemented for `list` and `stats` as suggested in output?
