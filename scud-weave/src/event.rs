//! Event model for weave coordination.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::matcher::GlobPattern;

/// A coordination event representing an action by an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// The kind of event being coordinated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventKind {
    FileWrite,
    FileCreate,
    DependencyAdd,
    DependencyRemove,
    SchemaChange,
    ApiChange,
    ConfigChange,
    TestRun,
    TestPass,
    TestFail,
    LintPass,
    LintFail,
    Commit,
    Build,
    TaskClaim,
    TaskComplete,
    DangerousCommand,
    Custom(String),
}

impl EventKind {
    /// Parse an event kind from a string.
    pub fn parse(s: &str) -> Self {
        match s {
            "FileWrite" => EventKind::FileWrite,
            "FileCreate" => EventKind::FileCreate,
            "DependencyAdd" => EventKind::DependencyAdd,
            "DependencyRemove" => EventKind::DependencyRemove,
            "SchemaChange" => EventKind::SchemaChange,
            "ApiChange" => EventKind::ApiChange,
            "ConfigChange" => EventKind::ConfigChange,
            "TestRun" => EventKind::TestRun,
            "TestPass" => EventKind::TestPass,
            "TestFail" => EventKind::TestFail,
            "LintPass" => EventKind::LintPass,
            "LintFail" => EventKind::LintFail,
            "Commit" => EventKind::Commit,
            "Build" => EventKind::Build,
            "TaskClaim" => EventKind::TaskClaim,
            "TaskComplete" => EventKind::TaskComplete,
            "DangerousCommand" => EventKind::DangerousCommand,
            other => EventKind::Custom(other.to_string()),
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &str {
        match self {
            EventKind::FileWrite => "FileWrite",
            EventKind::FileCreate => "FileCreate",
            EventKind::DependencyAdd => "DependencyAdd",
            EventKind::DependencyRemove => "DependencyRemove",
            EventKind::SchemaChange => "SchemaChange",
            EventKind::ApiChange => "ApiChange",
            EventKind::ConfigChange => "ConfigChange",
            EventKind::TestRun => "TestRun",
            EventKind::TestPass => "TestPass",
            EventKind::TestFail => "TestFail",
            EventKind::LintPass => "LintPass",
            EventKind::LintFail => "LintFail",
            EventKind::Commit => "Commit",
            EventKind::Build => "Build",
            EventKind::TaskClaim => "TaskClaim",
            EventKind::TaskComplete => "TaskComplete",
            EventKind::DangerousCommand => "DangerousCommand",
            EventKind::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A pattern for matching events in b-thread rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventPattern {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EventKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<GlobPattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Match any agent EXCEPT this one.
    #[serde(default)]
    pub negate_agent: bool,
    /// Match any target NOT in this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_not: Vec<GlobPattern>,
}

impl EventPattern {
    /// Create a pattern matching a specific event kind.
    pub fn kind(kind: EventKind) -> Self {
        EventPattern {
            kind: Some(kind),
            agent: None,
            target: None,
            task_id: None,
            negate_agent: false,
            target_not: Vec::new(),
        }
    }

    /// Check if an event matches this pattern.
    pub fn matches_event(&self, event: &Event) -> bool {
        // Check kind
        if let Some(ref pk) = self.kind {
            if pk != &event.kind {
                return false;
            }
        }

        // Check agent (with optional negation)
        if let Some(ref pa) = self.agent {
            match &event.agent {
                Some(ea) => {
                    if self.negate_agent {
                        if ea == pa {
                            return false;
                        }
                    } else if ea != pa {
                        return false;
                    }
                }
                None => {
                    // No agent on event: negated match succeeds, normal match fails
                    if !self.negate_agent {
                        return false;
                    }
                }
            }
        }

        // Check target glob
        if let Some(ref pt) = self.target {
            match &event.target {
                Some(et) => {
                    if !pt.matches(et) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Check target_not exclusions
        if !self.target_not.is_empty() {
            if let Some(ref et) = event.target {
                for exclude in &self.target_not {
                    if exclude.matches(et) {
                        return false;
                    }
                }
            }
        }

        // Check task_id
        if let Some(ref pt) = self.task_id {
            match &event.task_id {
                Some(et) => {
                    if et != pt {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_kind_parse_roundtrip() {
        let kinds = [
            "FileWrite",
            "TestPass",
            "Commit",
            "DangerousCommand",
            "CustomThing",
        ];
        for kind_str in kinds {
            let kind = EventKind::parse(kind_str);
            assert_eq!(kind.as_str(), kind_str);
        }
    }

    #[test]
    fn test_event_pattern_kind_only() {
        let pattern = EventPattern::kind(EventKind::FileWrite);
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&event));

        let wrong_kind = Event {
            kind: EventKind::Commit,
            agent: Some("agent-1".to_string()),
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&wrong_kind));
    }

    #[test]
    fn test_event_pattern_kind_and_agent() {
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: Some("agent-1".to_string()),
            target: None,
            task_id: None,
            negate_agent: false,
            target_not: Vec::new(),
        };

        let matching = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: Some("x".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&matching));

        let wrong_agent = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-2".to_string()),
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&wrong_agent));

        let no_agent = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&no_agent));
    }

    #[test]
    fn test_event_pattern_negated_agent() {
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: Some("admin".to_string()),
            target: None,
            task_id: None,
            negate_agent: true,
            target_not: Vec::new(),
        };

        // Non-admin matches (negated)
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&event));

        // Admin does NOT match (negated)
        let admin_event = Event {
            kind: EventKind::FileWrite,
            agent: Some("admin".to_string()),
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&admin_event));

        // No agent on event: negated match succeeds
        let no_agent = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&no_agent));
    }

    #[test]
    fn test_event_pattern_target_glob() {
        use crate::matcher::GlobPattern;
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: None,
            target: Some(GlobPattern::new("src/**/*.rs")),
            task_id: None,
            negate_agent: false,
            target_not: Vec::new(),
        };

        let matching = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: Some("src/weave/mod.rs".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&matching));

        let non_matching = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: Some("tests/test.rs".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&non_matching));

        let no_target = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&no_target));
    }

    #[test]
    fn test_event_pattern_target_not_exclusion() {
        use crate::matcher::GlobPattern;
        let pattern = EventPattern {
            kind: Some(EventKind::FileWrite),
            agent: None,
            target: None,
            task_id: None,
            negate_agent: false,
            target_not: vec![GlobPattern::new("docs/**"), GlobPattern::new("*.md")],
        };

        // Normal file passes
        let ok = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&ok));

        // Excluded by first pattern
        let docs = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: Some("docs/api.md".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&docs));

        // Excluded by second pattern
        let md = Event {
            kind: EventKind::FileWrite,
            agent: None,
            target: Some("README.md".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&md));
    }

    #[test]
    fn test_event_pattern_with_task_id() {
        let pattern = EventPattern {
            kind: Some(EventKind::TaskClaim),
            agent: None,
            target: None,
            task_id: Some("auth:1.1".to_string()),
            negate_agent: false,
            target_not: Vec::new(),
        };

        let matching = Event {
            kind: EventKind::TaskClaim,
            agent: Some("agent-1".to_string()),
            target: None,
            task_id: Some("auth:1.1".to_string()),
            metadata: HashMap::new(),
        };
        assert!(pattern.matches_event(&matching));

        let wrong_task = Event {
            kind: EventKind::TaskClaim,
            agent: None,
            target: None,
            task_id: Some("auth:1.2".to_string()),
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&wrong_task));

        let no_task = Event {
            kind: EventKind::TaskClaim,
            agent: None,
            target: None,
            task_id: None,
            metadata: HashMap::new(),
        };
        assert!(!pattern.matches_event(&no_task));
    }

    #[test]
    fn test_event_pattern_wildcard_matches_all_kinds() {
        // Pattern with no kind matches any kind
        let pattern = EventPattern {
            kind: None,
            agent: None,
            target: None,
            task_id: None,
            negate_agent: false,
            target_not: Vec::new(),
        };

        for kind in [EventKind::FileWrite, EventKind::Commit, EventKind::Build] {
            let event = Event {
                kind,
                agent: None,
                target: None,
                task_id: None,
                metadata: HashMap::new(),
            };
            assert!(pattern.matches_event(&event));
        }
    }

    #[test]
    fn test_event_kind_all_variants_roundtrip() {
        let variants = [
            "FileWrite", "FileCreate", "DependencyAdd", "DependencyRemove",
            "SchemaChange", "ApiChange", "ConfigChange", "TestRun",
            "TestPass", "TestFail", "LintPass", "LintFail",
            "Commit", "Build", "TaskClaim", "TaskComplete", "DangerousCommand",
        ];
        for name in variants {
            let kind = EventKind::parse(name);
            assert_eq!(kind.as_str(), name, "Roundtrip failed for {}", name);
        }
    }

    #[test]
    fn test_event_kind_custom_roundtrip() {
        let kind = EventKind::parse("MyCustomEvent");
        assert_eq!(kind.as_str(), "MyCustomEvent");
        match kind {
            EventKind::Custom(s) => assert_eq!(s, "MyCustomEvent"),
            _ => panic!("Expected Custom variant"),
        }
    }

    #[test]
    fn test_event_serde_roundtrip() {
        let event = Event {
            kind: EventKind::FileWrite,
            agent: Some("agent-1".to_string()),
            target: Some("src/main.rs".to_string()),
            task_id: None,
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }
}
