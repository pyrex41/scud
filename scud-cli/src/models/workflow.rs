use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Ideation,
    Planning,
    Architecture,
    Implementation,
    Retrospective,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Ideation => "ideation",
            Phase::Planning => "planning",
            Phase::Architecture => "architecture",
            Phase::Implementation => "implementation",
            Phase::Retrospective => "retrospective",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ideation" => Some(Phase::Ideation),
            "planning" => Some(Phase::Planning),
            "architecture" => Some(Phase::Architecture),
            "implementation" => Some(Phase::Implementation),
            "retrospective" => Some(Phase::Retrospective),
            _ => None,
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Phase::Ideation => Some(Phase::Planning),
            Phase::Planning => Some(Phase::Architecture),
            Phase::Architecture => Some(Phase::Implementation),
            Phase::Implementation => Some(Phase::Retrospective),
            Phase::Retrospective => None, // Completed
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInfo {
    pub status: String,
    pub completed_at: Option<String>,
    pub agent: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedEpic {
    pub tag: String,
    pub completed_at: String,
    pub metrics: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub version: String,
    pub current_phase: String,
    pub active_epic: Option<String>,
    pub phases: HashMap<String, PhaseInfo>,
    pub history: Vec<serde_json::Value>,
    pub completed_epics: Vec<CompletedEpic>,
    pub last_updated: Option<String>,
}

impl WorkflowState {
    pub fn new() -> Self {
        let mut phases = HashMap::new();

        phases.insert(
            "ideation".to_string(),
            PhaseInfo {
                status: "active".to_string(),
                completed_at: None,
                agent: "tm-pm".to_string(),
                description: "Product definition and PRD creation".to_string(),
            },
        );

        phases.insert(
            "planning".to_string(),
            PhaseInfo {
                status: "pending".to_string(),
                completed_at: None,
                agent: "tm-sm".to_string(),
                description: "Epic breakdown and task planning".to_string(),
            },
        );

        phases.insert(
            "architecture".to_string(),
            PhaseInfo {
                status: "pending".to_string(),
                completed_at: None,
                agent: "tm-architect".to_string(),
                description: "Technical design and architecture".to_string(),
            },
        );

        phases.insert(
            "implementation".to_string(),
            PhaseInfo {
                status: "pending".to_string(),
                completed_at: None,
                agent: "tm-dev".to_string(),
                description: "Task execution and development".to_string(),
            },
        );

        phases.insert(
            "retrospective".to_string(),
            PhaseInfo {
                status: "pending".to_string(),
                completed_at: None,
                agent: "tm-retrospective".to_string(),
                description: "Post-epic analysis and learning capture".to_string(),
            },
        );

        WorkflowState {
            version: "1.0.0".to_string(),
            current_phase: "ideation".to_string(),
            active_epic: None,
            phases,
            history: Vec::new(),
            completed_epics: Vec::new(),
            last_updated: None,
        }
    }

    pub fn set_phase(&mut self, phase: Phase) {
        self.current_phase = phase.as_str().to_string();
        self.last_updated = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn get_current_phase(&self) -> Option<Phase> {
        Phase::from_str(&self.current_phase)
    }

    pub fn update(&mut self) {
        self.last_updated = Some(chrono::Utc::now().to_rfc3339());
    }
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self::new()
    }
}
