//! Pipeline execution events for observability.

use serde::{Deserialize, Serialize};

use super::outcome::StageStatus;

/// Events emitted during pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineEvent {
    /// Pipeline execution started.
    PipelineStarted {
        pipeline_name: String,
        run_id: String,
    },

    /// A node has started executing.
    NodeStarted {
        node_id: String,
        handler_type: String,
    },

    /// A node has completed.
    NodeCompleted {
        node_id: String,
        status: StageStatus,
        duration_ms: u64,
    },

    /// An edge was selected for traversal.
    EdgeSelected {
        from_node: String,
        to_node: String,
        edge_label: String,
        selection_step: u8,
    },

    /// A retry is about to happen.
    RetryScheduled {
        node_id: String,
        attempt: u32,
        max_retries: u32,
        delay_ms: u64,
    },

    /// A goal gate check was performed.
    GoalGateCheck { node_id: String, satisfied: bool },

    /// A checkpoint was saved.
    CheckpointSaved { node_id: String },

    /// Pipeline execution completed.
    PipelineCompleted {
        status: StageStatus,
        total_duration_ms: u64,
        nodes_executed: usize,
    },

    /// An error occurred.
    Error {
        node_id: Option<String>,
        message: String,
    },

    /// Human input requested.
    HumanInputRequested {
        node_id: String,
        question: String,
        choices: Vec<String>,
    },

    /// Human input received.
    HumanInputReceived { node_id: String, answer: String },
}

impl PipelineEvent {
    pub fn pipeline_started(name: &str, run_id: &str) -> Self {
        Self::PipelineStarted {
            pipeline_name: name.to_string(),
            run_id: run_id.to_string(),
        }
    }

    pub fn node_started(id: &str, handler: &str) -> Self {
        Self::NodeStarted {
            node_id: id.to_string(),
            handler_type: handler.to_string(),
        }
    }

    pub fn node_completed(id: &str, status: StageStatus, duration_ms: u64) -> Self {
        Self::NodeCompleted {
            node_id: id.to_string(),
            status,
            duration_ms,
        }
    }

    pub fn edge_selected(from: &str, to: &str, label: &str, step: u8) -> Self {
        Self::EdgeSelected {
            from_node: from.to_string(),
            to_node: to.to_string(),
            edge_label: label.to_string(),
            selection_step: step,
        }
    }

    pub fn error(node_id: Option<&str>, message: &str) -> Self {
        Self::Error {
            node_id: node_id.map(String::from),
            message: message.to_string(),
        }
    }
}
