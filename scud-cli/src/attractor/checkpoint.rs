//! Checkpoint save/load for pipeline resumption.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::context::ContextSnapshot;
use super::outcome::StageStatus;

/// Serializable checkpoint for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// ISO 8601 timestamp of when this checkpoint was created.
    pub timestamp: String,
    /// Current node ID being executed (or last completed).
    pub current_node: String,
    /// Set of completed node IDs.
    pub completed_nodes: Vec<String>,
    /// Retry counts per node.
    pub node_retries: HashMap<String, u32>,
    /// Status of each visited node.
    pub node_statuses: HashMap<String, StageStatus>,
    /// Context snapshot at checkpoint time.
    pub context: ContextSnapshot,
    /// Execution log entries.
    pub log: Vec<LogEntry>,
}

/// A log entry in the checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub node_id: String,
    pub message: String,
}

impl Checkpoint {
    /// Create a new checkpoint at the given node.
    pub fn new(current_node: &str, context: ContextSnapshot) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            current_node: current_node.to_string(),
            completed_nodes: vec![],
            node_retries: HashMap::new(),
            node_statuses: HashMap::new(),
            context,
            log: vec![],
        }
    }

    /// Mark a node as completed.
    pub fn mark_completed(&mut self, node_id: &str, status: StageStatus) {
        if !self.completed_nodes.contains(&node_id.to_string()) {
            self.completed_nodes.push(node_id.to_string());
        }
        self.node_statuses.insert(node_id.to_string(), status);
    }

    /// Increment retry count for a node.
    pub fn increment_retry(&mut self, node_id: &str) -> u32 {
        let count = self.node_retries.entry(node_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Get retry count for a node.
    pub fn retry_count(&self, node_id: &str) -> u32 {
        self.node_retries.get(node_id).copied().unwrap_or(0)
    }

    /// Add a log entry.
    pub fn log(&mut self, node_id: &str, message: impl Into<String>) {
        self.log.push(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_id: node_id.to_string(),
            message: message.into(),
        });
    }

    /// Save checkpoint to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("Failed to serialize checkpoint")?;
        std::fs::write(path, json).context("Failed to write checkpoint file")?;
        Ok(())
    }

    /// Load checkpoint from a file.
    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path).context("Failed to read checkpoint file")?;
        let checkpoint: Self =
            serde_json::from_str(&json).context("Failed to deserialize checkpoint")?;
        Ok(checkpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        let mut ctx_values = HashMap::new();
        ctx_values.insert("key".into(), serde_json::json!("value"));
        let snapshot = ContextSnapshot::from(ctx_values);

        let mut cp = Checkpoint::new("node_a", snapshot);
        cp.mark_completed("node_a", StageStatus::Success);
        cp.increment_retry("node_b");
        cp.log("node_a", "Did something");

        cp.save(&path).unwrap();
        let loaded = Checkpoint::load(&path).unwrap();

        assert_eq!(loaded.current_node, "node_a");
        assert_eq!(loaded.completed_nodes, vec!["node_a"]);
        assert_eq!(loaded.retry_count("node_b"), 1);
        assert_eq!(loaded.log.len(), 1);
    }
}
