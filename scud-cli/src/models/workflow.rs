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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Phase Tests ====================

    #[test]
    fn test_phase_as_str() {
        assert_eq!(Phase::Ideation.as_str(), "ideation");
        assert_eq!(Phase::Planning.as_str(), "planning");
        assert_eq!(Phase::Architecture.as_str(), "architecture");
        assert_eq!(Phase::Implementation.as_str(), "implementation");
        assert_eq!(Phase::Retrospective.as_str(), "retrospective");
    }

    #[test]
    fn test_phase_from_str_valid() {
        assert_eq!(Phase::from_str("ideation"), Some(Phase::Ideation));
        assert_eq!(Phase::from_str("planning"), Some(Phase::Planning));
        assert_eq!(Phase::from_str("architecture"), Some(Phase::Architecture));
        assert_eq!(
            Phase::from_str("implementation"),
            Some(Phase::Implementation)
        );
        assert_eq!(
            Phase::from_str("retrospective"),
            Some(Phase::Retrospective)
        );
    }

    #[test]
    fn test_phase_from_str_invalid() {
        assert_eq!(Phase::from_str("invalid"), None);
        assert_eq!(Phase::from_str(""), None);
        assert_eq!(Phase::from_str("IDEATION"), None); // Case sensitive
        assert_eq!(Phase::from_str("idea"), None);
    }

    #[test]
    fn test_phase_next() {
        assert_eq!(Phase::Ideation.next(), Some(Phase::Planning));
        assert_eq!(Phase::Planning.next(), Some(Phase::Architecture));
        assert_eq!(Phase::Architecture.next(), Some(Phase::Implementation));
        assert_eq!(Phase::Implementation.next(), Some(Phase::Retrospective));
        assert_eq!(Phase::Retrospective.next(), None); // Final phase
    }

    #[test]
    fn test_phase_serialization() {
        let phase = Phase::Architecture;
        let json = serde_json::to_string(&phase).unwrap();
        assert_eq!(json, r#""architecture""#);

        let deserialized: Phase = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Phase::Architecture);
    }

    // ==================== WorkflowState Tests ====================

    #[test]
    fn test_workflow_state_new() {
        let state = WorkflowState::new();

        assert_eq!(state.version, "1.0.0");
        assert_eq!(state.current_phase, "ideation");
        assert_eq!(state.active_epic, None);
        assert_eq!(state.phases.len(), 5);
        assert_eq!(state.history.len(), 0);
        assert_eq!(state.completed_epics.len(), 0);
        assert_eq!(state.last_updated, None);
    }

    #[test]
    fn test_workflow_state_default() {
        let state = WorkflowState::default();
        let new_state = WorkflowState::new();

        assert_eq!(state.version, new_state.version);
        assert_eq!(state.current_phase, new_state.current_phase);
        assert_eq!(state.active_epic, new_state.active_epic);
    }

    #[test]
    fn test_workflow_state_initial_phases() {
        let state = WorkflowState::new();

        // Check ideation phase is active
        let ideation = state.phases.get("ideation").unwrap();
        assert_eq!(ideation.status, "active");
        assert_eq!(ideation.agent, "tm-pm");
        assert_eq!(ideation.completed_at, None);

        // Check other phases are pending
        let planning = state.phases.get("planning").unwrap();
        assert_eq!(planning.status, "pending");
        assert_eq!(planning.agent, "tm-sm");

        let architecture = state.phases.get("architecture").unwrap();
        assert_eq!(architecture.status, "pending");
        assert_eq!(architecture.agent, "tm-architect");

        let implementation = state.phases.get("implementation").unwrap();
        assert_eq!(implementation.status, "pending");
        assert_eq!(implementation.agent, "tm-dev");

        let retrospective = state.phases.get("retrospective").unwrap();
        assert_eq!(retrospective.status, "pending");
        assert_eq!(retrospective.agent, "tm-retrospective");
    }

    #[test]
    fn test_set_phase() {
        let mut state = WorkflowState::new();
        assert_eq!(state.current_phase, "ideation");
        assert_eq!(state.last_updated, None);

        state.set_phase(Phase::Planning);
        assert_eq!(state.current_phase, "planning");
        assert!(state.last_updated.is_some());

        state.set_phase(Phase::Implementation);
        assert_eq!(state.current_phase, "implementation");
    }

    #[test]
    fn test_get_current_phase() {
        let mut state = WorkflowState::new();

        assert_eq!(state.get_current_phase(), Some(Phase::Ideation));

        state.set_phase(Phase::Architecture);
        assert_eq!(state.get_current_phase(), Some(Phase::Architecture));

        state.set_phase(Phase::Retrospective);
        assert_eq!(state.get_current_phase(), Some(Phase::Retrospective));
    }

    #[test]
    fn test_get_current_phase_invalid() {
        let mut state = WorkflowState::new();
        state.current_phase = "invalid_phase".to_string();

        assert_eq!(state.get_current_phase(), None);
    }

    #[test]
    fn test_active_epic_management() {
        let mut state = WorkflowState::new();
        assert_eq!(state.active_epic, None);

        state.active_epic = Some("epic-1-auth".to_string());
        assert_eq!(state.active_epic, Some("epic-1-auth".to_string()));

        state.active_epic = None;
        assert_eq!(state.active_epic, None);
    }

    #[test]
    fn test_update_timestamp() {
        let mut state = WorkflowState::new();
        assert_eq!(state.last_updated, None);

        state.update();
        assert!(state.last_updated.is_some());

        let first_update = state.last_updated.clone();
        std::thread::sleep(std::time::Duration::from_millis(10));

        state.update();
        assert!(state.last_updated.is_some());
        assert_ne!(state.last_updated, first_update); // Should be different
    }

    #[test]
    fn test_completed_epics() {
        let mut state = WorkflowState::new();
        assert_eq!(state.completed_epics.len(), 0);

        let epic = CompletedEpic {
            tag: "epic-1-auth".to_string(),
            completed_at: "2025-11-16T10:30:00Z".to_string(),
            metrics: None,
        };

        state.completed_epics.push(epic);
        assert_eq!(state.completed_epics.len(), 1);
        assert_eq!(state.completed_epics[0].tag, "epic-1-auth");
    }

    #[test]
    fn test_completed_epic_with_metrics() {
        let mut metrics = HashMap::new();
        metrics.insert(
            "tasks_completed".to_string(),
            serde_json::Value::Number(serde_json::Number::from(12)),
        );
        metrics.insert(
            "total_complexity".to_string(),
            serde_json::Value::Number(serde_json::Number::from(55)),
        );

        let epic = CompletedEpic {
            tag: "epic-2-dashboard".to_string(),
            completed_at: "2025-11-16T12:00:00Z".to_string(),
            metrics: Some(metrics),
        };

        assert_eq!(epic.tag, "epic-2-dashboard");
        assert!(epic.metrics.is_some());
        assert_eq!(epic.metrics.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_workflow_state_serialization() {
        let state = WorkflowState::new();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: WorkflowState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, state.version);
        assert_eq!(deserialized.current_phase, state.current_phase);
        assert_eq!(deserialized.active_epic, state.active_epic);
        assert_eq!(deserialized.phases.len(), state.phases.len());
    }

    #[test]
    fn test_phase_info_structure() {
        let phase_info = PhaseInfo {
            status: "completed".to_string(),
            completed_at: Some("2025-11-16T10:00:00Z".to_string()),
            agent: "tm-pm".to_string(),
            description: "Product definition".to_string(),
        };

        assert_eq!(phase_info.status, "completed");
        assert_eq!(
            phase_info.completed_at,
            Some("2025-11-16T10:00:00Z".to_string())
        );
        assert_eq!(phase_info.agent, "tm-pm");
    }

    #[test]
    fn test_full_workflow_cycle() {
        let mut state = WorkflowState::new();
        state.active_epic = Some("epic-1-test".to_string());

        // Progress through all phases
        assert_eq!(state.get_current_phase(), Some(Phase::Ideation));

        state.set_phase(Phase::Planning);
        assert_eq!(state.get_current_phase(), Some(Phase::Planning));

        state.set_phase(Phase::Architecture);
        assert_eq!(state.get_current_phase(), Some(Phase::Architecture));

        state.set_phase(Phase::Implementation);
        assert_eq!(state.get_current_phase(), Some(Phase::Implementation));

        state.set_phase(Phase::Retrospective);
        assert_eq!(state.get_current_phase(), Some(Phase::Retrospective));

        // Complete epic
        let epic = CompletedEpic {
            tag: state.active_epic.clone().unwrap(),
            completed_at: chrono::Utc::now().to_rfc3339(),
            metrics: None,
        };
        state.completed_epics.push(epic);
        state.active_epic = None;

        // Reset to ideation
        state.set_phase(Phase::Ideation);
        assert_eq!(state.get_current_phase(), Some(Phase::Ideation));
        assert_eq!(state.active_epic, None);
        assert_eq!(state.completed_epics.len(), 1);
    }
}
