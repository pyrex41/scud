//! Stage execution outcomes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status of a stage execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// Stage completed successfully.
    Success,
    /// Stage failed.
    Failure,
    /// Stage was skipped.
    Skipped,
    /// Stage is waiting for human input.
    WaitingForHuman,
    /// Stage timed out.
    Timeout,
    /// Stage was cancelled.
    Cancelled,
}

impl StageStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, StageStatus::Success)
    }

    pub fn as_str(&self) -> &str {
        match self {
            StageStatus::Success => "success",
            StageStatus::Failure => "failure",
            StageStatus::Skipped => "skipped",
            StageStatus::WaitingForHuman => "waiting_for_human",
            StageStatus::Timeout => "timeout",
            StageStatus::Cancelled => "cancelled",
        }
    }
}

/// Outcome of executing a pipeline node handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    /// The execution status.
    pub status: StageStatus,
    /// The preferred label for edge selection (e.g., "success", "fail", "approve").
    pub preferred_label: Option<String>,
    /// Suggested next node IDs (used in edge selection step 3).
    pub suggested_next: Vec<String>,
    /// Context updates to apply after this node completes.
    pub context_updates: HashMap<String, serde_json::Value>,
    /// Response text from the handler (e.g., LLM output).
    pub response_text: Option<String>,
    /// Human-readable summary of what happened.
    pub summary: Option<String>,
}

impl Outcome {
    /// Create a simple success outcome.
    pub fn success() -> Self {
        Self {
            status: StageStatus::Success,
            preferred_label: None,
            suggested_next: vec![],
            context_updates: HashMap::new(),
            response_text: None,
            summary: None,
        }
    }

    /// Create a failure outcome with a message.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            status: StageStatus::Failure,
            preferred_label: None,
            suggested_next: vec![],
            context_updates: HashMap::new(),
            response_text: None,
            summary: Some(message.into()),
        }
    }

    /// Create a success outcome with a preferred label for edge routing.
    pub fn success_with_label(label: impl Into<String>) -> Self {
        Self {
            status: StageStatus::Success,
            preferred_label: Some(label.into()),
            suggested_next: vec![],
            context_updates: HashMap::new(),
            response_text: None,
            summary: None,
        }
    }

    /// Set context updates on this outcome.
    pub fn with_context(mut self, updates: HashMap<String, serde_json::Value>) -> Self {
        self.context_updates = updates;
        self
    }

    /// Set response text on this outcome.
    pub fn with_response(mut self, text: impl Into<String>) -> Self {
        self.response_text = Some(text.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_outcome() {
        let o = Outcome::success();
        assert!(o.status.is_success());
        assert!(o.preferred_label.is_none());
        assert!(o.context_updates.is_empty());
    }

    #[test]
    fn test_failure_outcome() {
        let o = Outcome::failure("bad things");
        assert!(!o.status.is_success());
        assert_eq!(o.summary.as_deref(), Some("bad things"));
    }

    #[test]
    fn test_success_with_label() {
        let o = Outcome::success_with_label("approve");
        assert!(o.status.is_success());
        assert_eq!(o.preferred_label.as_deref(), Some("approve"));
    }

    #[test]
    fn test_with_context() {
        let mut ctx = HashMap::new();
        ctx.insert("key".into(), serde_json::json!("value"));
        let o = Outcome::success().with_context(ctx);
        assert_eq!(o.context_updates.len(), 1);
    }
}
