use crate::metrics::TaskMetrics;
use anyhow::Result;
use chrono::{DateTime, Utc};
use scud_core::{Storage, TaskStatus};
use serde::Deserialize;
use std::path::Path;

// Minimal structs for session JSON (from scud-cli swarm/session.rs)
#[derive(Debug, Deserialize)]
pub struct SwarmSessionJson {
    pub waves: Vec<WaveState>,
}

#[derive(Debug, Deserialize)]
pub struct WaveState {
    pub rounds: Vec<RoundState>,
    pub repairs: Vec<RepairAttempt>,
    pub validation: Option<ValidationResult>,
}

#[derive(Debug, Deserialize)]
pub struct RoundState {
    pub task_ids: Vec<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepairAttempt {
    pub attributed_tasks: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidationResult {
    pub all_passed: bool,
    pub commands: Vec<ValidationCommand>,
}

#[derive(Debug, Deserialize)]
pub struct ValidationCommand {
    pub command: String,
    pub passed: bool,
    pub duration_secs: f64,
    pub run_count: u32,
}

/// Collect metrics from a completed swarm session
pub fn collect_swarm_metrics(session_path: &Path, workspace: &Path) -> Result<Vec<TaskMetrics>> {
    let session_content = std::fs::read_to_string(session_path)?;
    let session: SwarmSessionJson = serde_json::from_str(&session_content)?;

    // Load task data from scud-core storage to enrich metrics
    let storage = Storage::new(Some(workspace.to_path_buf()));
    let phase = storage.load_active_group().ok();

    let mut task_metrics = vec![];

    for wave in session.waves {
        for round in wave.rounds {
            for task_id in round.task_ids {
                let git_stats = collect_git_stats_for_task(workspace, &task_id)?;

                let started = parse_rfc3339(&round.started_at)?;
                let completed = round
                    .completed_at
                    .as_ref()
                    .map(|s| parse_rfc3339(s))
                    .transpose()?;

                let repair_count = wave
                    .repairs
                    .iter()
                    .filter(|r| r.attributed_tasks.contains(&task_id))
                    .count() as u32;

                let first_pass = wave
                    .validation
                    .as_ref()
                    .map(|v| v.all_passed)
                    .unwrap_or(true);

                let duration_secs = completed.map(|c| {
                    let duration = c.signed_duration_since(started);
                    duration.num_seconds() as f64
                });

                // Look up task data from storage for title, complexity, and final status
                let task_data = phase.as_ref().and_then(|p| p.get_task(&task_id));
                let task_title = task_data.map(|t| t.title.clone()).unwrap_or_default();
                let complexity = task_data.map(|t| t.complexity).unwrap_or(0);
                let final_success = task_data
                    .map(|t| t.status == TaskStatus::Done)
                    .unwrap_or(false);

                task_metrics.push(TaskMetrics {
                    task_id,
                    task_title,
                    complexity,
                    started_at: started,
                    completed_at: completed,
                    duration_secs,
                    success: final_success,
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

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>> {
    let dt = chrono::DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc);
    Ok(dt)
}

/// Get git diff stats for changes attributed to a task
fn collect_git_stats_for_task(workspace: &Path, task_id: &str) -> Result<Option<GitStats>> {
    // Find commits with [task_id] prefix
    let output = std::process::Command::new("git")
        .args([
            "log",
            "--oneline",
            "--all",
            &format!("--grep=\\[{}]", task_id),
        ])
        .current_dir(workspace)
        .output()?;

    let stdout = std::str::from_utf8(&output.stdout)?;
    let commits: Vec<&str> = stdout
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

#[derive(Default, Debug, Clone, Copy)]
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

    let text = std::str::from_utf8(&output.stdout)?;

    // Parse " (\d+) files? changed, (\d+) insertions?\(\+\), (\d+) deletions?\(-\)"
    let re = regex::Regex::new(
        r"(\d+)\s*file[s]?\s*changed[,\s]*(\d+)\s*insertion[s]?\(\+\)[,\s]*(\d+)\s*deletion[s]?\(-\)",
    )?;
    if let Some(caps) = re.captures(text.trim()) {
        return Ok(GitStats {
            files: caps[1].parse().unwrap_or(0),
            additions: caps[2].parse().unwrap_or(0),
            deletions: caps[3].parse().unwrap_or(0),
        });
    }

    Ok(GitStats::default())
}
