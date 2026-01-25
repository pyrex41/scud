use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    pub run_count: u32, // How many times this command was run
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    Swarm { round_size: usize },
    Ralph,
    ClaudeDirect, // Single session baseline
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
