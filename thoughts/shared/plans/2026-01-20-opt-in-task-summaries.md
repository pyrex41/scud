# Opt-In Task Summaries with Metrics Implementation Plan

## Overview

Add optional `summary` field to Task model with structured metrics (model, tokens, tool calls, duration). Provide on-demand LLM/manual summaries from per-task logs, plus explicit metrics logging. Opt-in only—no auto-gen by default.

**Goals**:
- Generate concise "what this task accomplished" summaries
- Track execution metrics: model used, token counts, tool calls, duration
- Manual/CLI/agents can log metrics: `scud log-metrics <id> --model X --tokens-in Y --tokens-out Z --tools "Read:8,Edit:4" --duration 5`
- Generate summaries: `scud summary <id> --auto --set`
- Display in `scud show`/`scud next`

**Design Decision**: Metrics are logged explicitly for now via `scud log-metrics`. Later, Claude Code/OpenCode harnesses can provide this data automatically.

## Current State Analysis

### Existing Infrastructure
- **Task model** (`models/task.rs:74-118`): Has optional fields pattern, no summary/metrics
- **Log system** (`commands/log.rs`): Timestamped append-only logs in `.scud/logs/<id>.log`
- **SCG format** (`formats/scg.rs`): Has `@details`, `@agents` sections - can add `@summaries`
- **LLM client** (`llm/client.rs`): `complete_fast()` method for generation tasks

### What's Missing
- `summary` field on Task
- Metrics storage
- Summary/metrics commands
- SCG `@summaries` section

## Desired End State

1. `scud log-metrics main:5 --model claude-sonnet-4 --tokens-in 12450 --tokens-out 3200 --tools "Read:8,Edit:4,Bash:3" --duration 5`
2. `scud summary main:5 --auto --set` - generates summary from logs + metrics
3. `scud summary main:5 --last` - shows last log entry as summary
4. `scud show main:5` - displays summary and metrics if present

### Verification:
- `cargo test` passes
- `cargo clippy` passes
- All command variants work as expected

## What We're NOT Doing

- Not auto-capturing metrics from harnesses (future enhancement)
- Not auto-generating summaries on task completion (opt-in only)
- Not storing full transcripts (just logs + metrics)
- Not adding complex analytics dashboards

## Implementation Approach

Three-phase approach:
1. Add data model (Task field + metrics storage)
2. Add metrics logging command
3. Add summary command with LLM integration

---

## Phase 1: Data Model Changes

### Overview
Add `summary` field to Task model and create metrics storage format.

### Changes Required:

#### 1.1 Add summary field to Task model

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add summary field after `agent_type`

After line 117 (`pub agent_type: Option<String>`), add:

```rust
    /// Task completion summary (what was accomplished)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
```

Update `Task::new()` (around line 146) to initialize:
```rust
            agent_type: None,
            summary: None,  // Add this line
```

#### 1.2 Add metrics structure

**File**: `scud-cli/src/models/task.rs`
**Changes**: Add TaskMetrics struct before Task struct

```rust
/// Execution metrics for a task
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskMetrics {
    /// Model used (e.g., "claude-sonnet-4")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Input tokens consumed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u32>,

    /// Output tokens generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u32>,

    /// Tool calls breakdown (e.g., "Read:8,Edit:4,Bash:3")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,

    /// Duration in minutes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_mins: Option<u32>,

    /// Timestamp when metrics were recorded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
}
```

Add metrics field to Task struct (after summary):
```rust
    /// Execution metrics (model, tokens, tool calls, duration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<TaskMetrics>,
```

Update `Task::new()` to initialize:
```rust
            summary: None,
            metrics: None,  // Add this line
```

#### 1.3 Add SCG serialization for summaries and metrics

**File**: `scud-cli/src/formats/scg.rs`
**Changes**: Add `@summaries` and `@metrics` sections

In `parse_scg()` (after line 150, `let mut agent_types`), add:
```rust
    let mut summaries: HashMap<String, String> = HashMap::new();
    let mut metrics: HashMap<String, TaskMetrics> = HashMap::new();
```

In the section header match (around line 178), add:
```rust
                "@summaries" => "summaries",
                "@metrics" => "metrics",
```

Add parsing logic (after `Some("agents")` block, around line 283):
```rust
            Some("summaries") => {
                // Parse "id | summary text"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 && !parts[1].is_empty() {
                    summaries.insert(parts[0].clone(), unescape_text(&parts[1]));
                }
            }
            Some("metrics") => {
                // Parse "id | model | tokens_in | tokens_out | tool_calls | duration_mins | recorded_at"
                let parts = split_by_pipe(trimmed);
                if parts.len() >= 2 {
                    let mut m = TaskMetrics::default();
                    if parts.len() > 1 && !parts[1].is_empty() {
                        m.model = Some(parts[1].clone());
                    }
                    if parts.len() > 2 && !parts[2].is_empty() {
                        m.tokens_in = parts[2].parse().ok();
                    }
                    if parts.len() > 3 && !parts[3].is_empty() {
                        m.tokens_out = parts[3].parse().ok();
                    }
                    if parts.len() > 4 && !parts[4].is_empty() {
                        m.tool_calls = Some(parts[4].clone());
                    }
                    if parts.len() > 5 && !parts[5].is_empty() {
                        m.duration_mins = parts[5].parse().ok();
                    }
                    if parts.len() > 6 && !parts[6].is_empty() {
                        m.recorded_at = Some(parts[6].clone());
                    }
                    metrics.insert(parts[0].clone(), m);
                }
            }
```

Apply summaries and metrics to tasks (after applying agent_types, around line 358):
```rust
    // Apply summaries
    for (id, summary) in summaries {
        if let Some(task) = tasks.get_mut(&id) {
            task.summary = Some(summary);
        }
    }

    // Apply metrics
    for (id, m) in metrics {
        if let Some(task) = tasks.get_mut(&id) {
            task.metrics = Some(m);
        }
    }
```

In `serialize_scg()` (after @agents section, around line 522), add:
```rust
    // Summaries section
    let tasks_with_summaries: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.summary.is_some())
        .collect();

    if !tasks_with_summaries.is_empty() {
        output.push_str("@summaries\n");
        output.push_str("# id | summary\n");
        for task in tasks_with_summaries {
            if let Some(ref summary) = task.summary {
                output.push_str(&format!(
                    "{} | {}\n",
                    task.id,
                    escape_text(summary)
                ));
            }
        }
        output.push('\n');
    }

    // Metrics section
    let tasks_with_metrics: Vec<_> = sorted_tasks
        .iter()
        .filter(|t| t.metrics.is_some())
        .collect();

    if !tasks_with_metrics.is_empty() {
        output.push_str("@metrics\n");
        output.push_str("# id | model | tokens_in | tokens_out | tool_calls | duration_mins | recorded_at\n");
        for task in tasks_with_metrics {
            if let Some(ref m) = task.metrics {
                output.push_str(&format!(
                    "{} | {} | {} | {} | {} | {} | {}\n",
                    task.id,
                    m.model.as_deref().unwrap_or(""),
                    m.tokens_in.map(|n| n.to_string()).unwrap_or_default(),
                    m.tokens_out.map(|n| n.to_string()).unwrap_or_default(),
                    m.tool_calls.as_deref().unwrap_or(""),
                    m.duration_mins.map(|n| n.to_string()).unwrap_or_default(),
                    m.recorded_at.as_deref().unwrap_or(""),
                ));
            }
        }
        output.push('\n');
    }
```

#### 1.4 Export TaskMetrics from models module

**File**: `scud-cli/src/models/mod.rs`
**Changes**: Add TaskMetrics to exports

```rust
pub use task::{Priority, Task, TaskMetrics, TaskStatus};
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

#### Manual Verification:
- [ ] Create task, set summary manually, verify persists through SCG round-trip

---

## Phase 2: Metrics Logging Command

### Overview
Add `scud log-metrics` command for explicit metrics recording.

### Changes Required:

#### 2.1 Add LogMetrics command to CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add LogMetrics command variant

Find `Log` command (around line 526) and add after `LogAll`:
```rust
    /// Log execution metrics for a task
    LogMetrics {
        /// Task ID
        task_id: String,

        /// Model used (e.g., claude-sonnet-4)
        #[arg(long)]
        model: Option<String>,

        /// Input tokens consumed
        #[arg(long)]
        tokens_in: Option<u32>,

        /// Output tokens generated
        #[arg(long)]
        tokens_out: Option<u32>,

        /// Tool calls breakdown (e.g., "Read:8,Edit:4,Bash:3")
        #[arg(long)]
        tools: Option<String>,

        /// Duration in minutes
        #[arg(long)]
        duration: Option<u32>,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },
```

#### 2.2 Add command dispatch

**File**: `scud-cli/src/main.rs`
**Changes**: Add match arm for LogMetrics (after LogAll dispatch)

```rust
        Commands::LogMetrics {
            task_id,
            model,
            tokens_in,
            tokens_out,
            tools,
            duration,
            tag,
        } => commands::log::log_metrics(
            cli.project,
            &task_id,
            model.as_deref(),
            tokens_in,
            tokens_out,
            tools.as_deref(),
            duration,
            tag.as_deref(),
        ),
```

#### 2.3 Implement log_metrics function

**File**: `scud-cli/src/commands/log.rs`
**Changes**: Add log_metrics function

```rust
use crate::models::TaskMetrics;

/// Log execution metrics for a task
pub fn log_metrics(
    project_root: Option<PathBuf>,
    task_id: &str,
    model: Option<&str>,
    tokens_in: Option<u32>,
    tokens_out: Option<u32>,
    tools: Option<&str>,
    duration_mins: Option<u32>,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Get active tag if not provided
    let active_tag = match tag {
        Some(t) => t.to_string(),
        None => storage
            .get_active_group()?
            .ok_or_else(|| anyhow::anyhow!("No active tag. Use --tag or run: scud tags <tag>"))?,
    };

    // Load phase and find task
    let mut phase = storage.load_group(&active_tag)?;
    let task = phase
        .get_task_mut(task_id)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in tag '{}'", task_id, active_tag))?;

    // Build metrics
    let metrics = TaskMetrics {
        model: model.map(String::from),
        tokens_in,
        tokens_out,
        tool_calls: tools.map(String::from),
        duration_mins,
        recorded_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    // Update task
    task.metrics = Some(metrics);
    task.update();

    // Save
    storage.update_group(&active_tag, &phase)?;

    // Print confirmation
    println!("✓ Metrics logged for task {}", task_id);
    if let Some(m) = model {
        println!("  Model: {}", m);
    }
    if let Some(ti) = tokens_in {
        println!("  Tokens in: {}", ti);
    }
    if let Some(to) = tokens_out {
        println!("  Tokens out: {}", to);
    }
    if let Some(t) = tools {
        println!("  Tool calls: {}", t);
    }
    if let Some(d) = duration_mins {
        println!("  Duration: {} min", d);
    }

    Ok(())
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes

#### Manual Verification:
- [ ] `scud log-metrics main:1 --model claude-sonnet-4 --tokens-in 1000 --tokens-out 500 --tools "Read:5,Edit:2" --duration 3`
- [ ] `scud show main:1` shows metrics (after Phase 3)

---

## Phase 3: Summary Command

### Overview
Add `scud summary` command for viewing/generating summaries.

### Changes Required:

#### 3.1 Add Summary command to CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add Summary command variant

```rust
    /// View or generate task summary
    Summary {
        /// Task ID(s)
        task_ids: Vec<String>,

        /// Auto-generate summary from logs using LLM
        #[arg(long)]
        auto: bool,

        /// Use last log entry as summary
        #[arg(long)]
        last: bool,

        /// Save summary to task
        #[arg(long)]
        set: bool,

        /// Override LLM model
        #[arg(long)]
        model: Option<String>,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },
```

#### 3.2 Add command dispatch

**File**: `scud-cli/src/main.rs`
**Changes**: Add match arm for Summary

```rust
        Commands::Summary {
            task_ids,
            auto,
            last,
            set,
            model,
            tag,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(commands::summary::run(
                cli.project,
                &task_ids,
                auto,
                last,
                set,
                model.as_deref(),
                tag.as_deref(),
            ))
        }
```

#### 3.3 Create summary command module

**File**: `scud-cli/src/commands/summary.rs` (new file)

```rust
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::llm::LLMClient;
use crate::storage::Storage;

/// Run summary command
pub async fn run(
    project_root: Option<PathBuf>,
    task_ids: &[String],
    auto: bool,
    last: bool,
    set: bool,
    model_override: Option<&str>,
    tag: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Get active tag if not provided
    let active_tag = match tag {
        Some(t) => t.to_string(),
        None => storage
            .get_active_group()?
            .ok_or_else(|| anyhow::anyhow!("No active tag. Use --tag or run: scud tags <tag>"))?,
    };

    let mut phase = storage.load_group(&active_tag)?;

    for task_id in task_ids {
        let task = phase
            .get_task(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in tag '{}'", task_id, active_tag))?;

        // Get log content
        let logs_dir = storage.scud_dir().join("logs");
        let log_file = logs_dir.join(format!("{}.log", task_id));
        let log_content = if log_file.exists() {
            fs::read_to_string(&log_file).unwrap_or_default()
        } else {
            String::new()
        };

        let summary = if auto {
            // Generate with LLM
            if log_content.is_empty() {
                println!("{} No logs found for task {}. Cannot auto-generate summary.", "!".yellow(), task_id);
                continue;
            }
            generate_summary_with_llm(project_root.clone(), &log_content, task.metrics.as_ref(), model_override).await?
        } else if last {
            // Extract last log entry
            extract_last_entry(&log_content).unwrap_or_else(|| "No log entries found.".to_string())
        } else if let Some(ref existing) = task.summary {
            // Show existing summary
            existing.clone()
        } else {
            println!("{} No summary for task {}. Use --auto or --last to generate.", "!".yellow(), task_id);
            continue;
        };

        // Display summary
        println!("\n{}", format!("## Task {} Summary", task_id).cyan().bold());
        println!();

        // Show metrics if present
        if let Some(ref m) = task.metrics {
            println!("{}", "**Metrics:**".bold());
            if let Some(ref model) = m.model {
                println!("- Model: {}", model);
            }
            if let (Some(ti), Some(to)) = (m.tokens_in, m.tokens_out) {
                println!("- Tokens: {} in / {} out", ti, to);
            }
            if let Some(ref tools) = m.tool_calls {
                println!("- Tool calls: {}", tools);
            }
            if let Some(d) = m.duration_mins {
                println!("- Duration: {} min", d);
            }
            println!();
        }

        println!("{}", "**Summary:**".bold());
        println!("{}", summary);
        println!();
        println!("[Source: logs/{}.log]", task_id);

        // Save if requested
        if set {
            let task_mut = phase.get_task_mut(task_id).unwrap();
            task_mut.summary = Some(summary.clone());
            task_mut.update();
            storage.update_group(&active_tag, &phase)?;
            println!("\n{} Summary saved to task", "✓".green());
        }
    }

    Ok(())
}

/// Generate summary using LLM
async fn generate_summary_with_llm(
    project_root: Option<PathBuf>,
    log_content: &str,
    metrics: Option<&crate::models::TaskMetrics>,
    model_override: Option<&str>,
) -> Result<String> {
    let client = match project_root {
        Some(root) => LLMClient::new_with_project_root(root)?,
        None => LLMClient::new()?,
    };

    // Build metrics context
    let metrics_context = if let Some(m) = metrics {
        let mut parts = Vec::new();
        if let Some(ref model) = m.model {
            parts.push(format!("Model: {}", model));
        }
        if let (Some(ti), Some(to)) = (m.tokens_in, m.tokens_out) {
            parts.push(format!("Tokens: {} in / {} out", ti, to));
        }
        if let Some(ref tools) = m.tool_calls {
            parts.push(format!("Tool calls: {}", tools));
        }
        if let Some(d) = m.duration_mins {
            parts.push(format!("Duration: {} min", d));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("\n\nExecution metrics:\n{}", parts.join("\n"))
        }
    } else {
        String::new()
    };

    // Truncate log content if too long (keep most recent)
    let truncated_log = if log_content.len() > 8000 {
        let start = log_content.len() - 8000;
        format!("...(truncated)...\n{}", &log_content[start..])
    } else {
        log_content.to_string()
    };

    let prompt = format!(
        r#"Summarize what was accomplished in this task based on the activity log below.
Be concise (2-4 sentences). Focus on what was done, not process details.
Do not include the metrics in your summary - they will be shown separately.

Activity log:
{}{}

Summary:"#,
        truncated_log,
        metrics_context
    );

    let model_info = client.fast_model_info(model_override);
    println!("Generating summary using {}...", model_info);

    client
        .complete_fast(&prompt, model_override)
        .await
        .context("Failed to generate summary")
}

/// Extract the last log entry
fn extract_last_entry(log_content: &str) -> Option<String> {
    let mut last_content = String::new();
    let mut in_entry = false;

    for line in log_content.lines().rev() {
        if line.starts_with("--- ") && line.ends_with(" ---") {
            // Found start of last entry, we're done
            if !last_content.is_empty() {
                // Reverse the lines we collected
                let lines: Vec<&str> = last_content.lines().collect();
                return Some(lines.into_iter().rev().collect::<Vec<_>>().join("\n"));
            }
            in_entry = true;
            continue;
        }
        if in_entry && !line.is_empty() {
            if !last_content.is_empty() {
                last_content.push('\n');
            }
            last_content.push_str(line);
        }
    }

    if last_content.is_empty() {
        None
    } else {
        let lines: Vec<&str> = last_content.lines().collect();
        Some(lines.into_iter().rev().collect::<Vec<_>>().join("\n"))
    }
}
```

#### 3.4 Export summary module

**File**: `scud-cli/src/commands/mod.rs`
**Changes**: Add summary module

```rust
pub mod summary;
```

#### 3.5 Update show command to display summary/metrics

**File**: `scud-cli/src/commands/show.rs`
**Changes**: Add summary and metrics to output

Find where task details are printed and add after existing fields:
```rust
    // Show metrics if present
    if let Some(ref m) = task.metrics {
        println!();
        println!("{}", "Metrics:".cyan());
        if let Some(ref model) = m.model {
            println!("  Model: {}", model);
        }
        if let (Some(ti), Some(to)) = (m.tokens_in, m.tokens_out) {
            println!("  Tokens: {} in / {} out", ti, to);
        }
        if let Some(ref tools) = m.tool_calls {
            println!("  Tools: {}", tools);
        }
        if let Some(d) = m.duration_mins {
            println!("  Duration: {} min", d);
        }
    }

    // Show summary if present
    if let Some(ref summary) = task.summary {
        println!();
        println!("{}", "Summary:".cyan());
        // Truncate for display
        let display = if summary.len() > 200 {
            format!("{}...", &summary[..200])
        } else {
            summary.clone()
        };
        println!("  {}", display);
    }
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

#### Manual Verification:
- [ ] `scud log main:1 "Found issue in auth module"`
- [ ] `scud log-metrics main:1 --model claude-sonnet-4 --tokens-in 5000 --tokens-out 1200 --duration 3`
- [ ] `scud summary main:1 --last` shows last log entry
- [ ] `scud summary main:1 --auto` generates LLM summary (requires LLM config)
- [ ] `scud summary main:1 --auto --set` saves summary to task
- [ ] `scud show main:1` displays summary and metrics

---

## Testing Strategy

### Unit Tests
- TaskMetrics serialization/deserialization
- SCG round-trip with summaries and metrics
- extract_last_entry function

### Integration Tests
- `scud log-metrics` command
- `scud summary --last` command
- Summary persistence through SCG

### Manual Testing Steps
1. Initialize project: `scud init`
2. Create task: `scud add "Test task" --tag test`
3. Log activity: `scud log test:1 "Did some work"`
4. Log metrics: `scud log-metrics test:1 --model claude-sonnet-4 --tokens-in 1000 --tokens-out 500 --tools "Read:5" --duration 2`
5. View with --last: `scud summary test:1 --last`
6. Generate with LLM: `scud summary test:1 --auto` (if LLM configured)
7. Save summary: `scud summary test:1 --auto --set`
8. Verify in show: `scud show test:1`

## Performance Considerations

- Log truncation for LLM input (max 8000 chars)
- Summary field has no explicit max length (SCG escapes newlines)
- Metrics are lightweight integers/strings

## Migration Notes

- Existing tasks get `summary: None` and `metrics: None` by default
- SCG format is backward compatible (new sections optional)
- No migration script needed

## Future Enhancements (Not in Scope)

- Harness integration for automatic metrics capture
- `auto_summarize_on_done` config option
- Bulk summary generation for all done tasks
- Summary templates/prompts customization

## References

- Task model: `scud-cli/src/models/task.rs`
- Log command: `scud-cli/src/commands/log.rs`
- SCG format: `scud-cli/src/formats/scg.rs`
- LLM client: `scud-cli/src/llm/client.rs`
- Research: `thoughts/shared/research/2026-01-20-status-updates-and-transcript-summaries.md`
