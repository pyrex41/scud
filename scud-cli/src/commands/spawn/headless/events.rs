//! Streaming event types for headless execution
//!
//! Defines the event types used to capture and transmit streaming
//! output from headless agent execution.

use serde::{Deserialize, Serialize};

/// A streaming event from an agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamEvent {
    /// Timestamp in milliseconds since session start
    pub timestamp_ms: u64,
    /// The event kind
    pub kind: StreamEventKind,
}

/// Types of streaming events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventKind {
    /// Text output delta
    TextDelta { text: String },

    /// Tool execution started
    ToolStart {
        tool_name: String,
        tool_id: String,
        input_summary: String,
    },

    /// Tool execution completed
    ToolResult {
        tool_name: String,
        tool_id: String,
        success: bool,
    },

    /// Agent completed successfully
    Complete { success: bool },

    /// Agent encountered an error
    Error { message: String },

    /// Session ID assigned (for continuation)
    SessionAssigned { session_id: String },
}

impl StreamEvent {
    /// Create a new event with the given kind
    ///
    /// The timestamp will be set to 0 and should be updated by the store
    /// when the event is added.
    pub fn new(kind: StreamEventKind) -> Self {
        Self {
            timestamp_ms: 0,
            kind,
        }
    }

    /// Create a new event with an explicit timestamp
    pub fn with_timestamp(kind: StreamEventKind, timestamp_ms: u64) -> Self {
        Self { timestamp_ms, kind }
    }

    /// Create a text delta event
    pub fn text_delta(text: impl Into<String>) -> Self {
        Self::new(StreamEventKind::TextDelta { text: text.into() })
    }

    /// Create a tool start event
    pub fn tool_start(name: &str, id: &str, input: &str) -> Self {
        Self::new(StreamEventKind::ToolStart {
            tool_name: name.to_string(),
            tool_id: id.to_string(),
            input_summary: input.to_string(),
        })
    }

    /// Create a tool result event
    pub fn tool_result(name: &str, id: &str, success: bool) -> Self {
        Self::new(StreamEventKind::ToolResult {
            tool_name: name.to_string(),
            tool_id: id.to_string(),
            success,
        })
    }

    /// Create a completion event
    pub fn complete(success: bool) -> Self {
        Self::new(StreamEventKind::Complete { success })
    }

    /// Create an error event
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(StreamEventKind::Error {
            message: message.into(),
        })
    }

    /// Create a session assigned event
    pub fn session_assigned(session_id: impl Into<String>) -> Self {
        Self::new(StreamEventKind::SessionAssigned {
            session_id: session_id.into(),
        })
    }

    /// Check if this event indicates completion (success or failure)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.kind,
            StreamEventKind::Complete { .. } | StreamEventKind::Error { .. }
        )
    }

    /// Check if this event indicates successful completion
    pub fn is_success(&self) -> bool {
        matches!(self.kind, StreamEventKind::Complete { success: true })
    }

    /// Get the text content if this is a text delta event
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            StreamEventKind::TextDelta { text } => Some(text),
            _ => None,
        }
    }

    /// Get the session ID if this is a session assigned event
    pub fn session_id(&self) -> Option<&str> {
        match &self.kind {
            StreamEventKind::SessionAssigned { session_id } => Some(session_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta_creation() {
        let event = StreamEvent::text_delta("Hello, world!");
        assert_eq!(event.timestamp_ms, 0);
        assert!(matches!(
            event.kind,
            StreamEventKind::TextDelta { ref text } if text == "Hello, world!"
        ));
        assert_eq!(event.text(), Some("Hello, world!"));
    }

    #[test]
    fn test_tool_start_creation() {
        let event = StreamEvent::tool_start("Read", "tool_123", "{path: src/main.rs}");
        match &event.kind {
            StreamEventKind::ToolStart {
                tool_name,
                tool_id,
                input_summary,
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_id, "tool_123");
                assert_eq!(input_summary, "{path: src/main.rs}");
            }
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_tool_result_creation() {
        let event = StreamEvent::tool_result("Edit", "tool_456", true);
        match &event.kind {
            StreamEventKind::ToolResult {
                tool_name,
                tool_id,
                success,
            } => {
                assert_eq!(tool_name, "Edit");
                assert_eq!(tool_id, "tool_456");
                assert!(*success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_tool_result_failure() {
        let event = StreamEvent::tool_result("Bash", "tool_789", false);
        match &event.kind {
            StreamEventKind::ToolResult { success, .. } => {
                assert!(!*success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_complete_event_success() {
        let event = StreamEvent::complete(true);
        assert!(event.is_terminal());
        assert!(event.is_success());
        assert!(matches!(
            event.kind,
            StreamEventKind::Complete { success: true }
        ));
    }

    #[test]
    fn test_complete_event_failure() {
        let event = StreamEvent::complete(false);
        assert!(event.is_terminal());
        assert!(!event.is_success());
        assert!(matches!(
            event.kind,
            StreamEventKind::Complete { success: false }
        ));
    }

    #[test]
    fn test_error_event() {
        let event = StreamEvent::error("Something went wrong");
        assert!(event.is_terminal());
        assert!(!event.is_success());
        match &event.kind {
            StreamEventKind::Error { message } => {
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_session_assigned() {
        let event = StreamEvent::session_assigned("sess-abc123");
        assert!(!event.is_terminal());
        assert_eq!(event.session_id(), Some("sess-abc123"));
    }

    #[test]
    fn test_with_timestamp() {
        let event = StreamEvent::with_timestamp(
            StreamEventKind::TextDelta {
                text: "test".to_string(),
            },
            1234,
        );
        assert_eq!(event.timestamp_ms, 1234);
    }

    #[test]
    fn test_text_accessor_none_for_non_text() {
        let event = StreamEvent::complete(true);
        assert_eq!(event.text(), None);
    }

    #[test]
    fn test_session_id_accessor_none_for_non_session() {
        let event = StreamEvent::text_delta("hello");
        assert_eq!(event.session_id(), None);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let events = vec![
            StreamEvent::text_delta("Hello"),
            StreamEvent::tool_start("Bash", "t1", "echo test"),
            StreamEvent::tool_result("Bash", "t1", true),
            StreamEvent::session_assigned("sess-123"),
            StreamEvent::complete(true),
            StreamEvent::error("failed"),
        ];

        for event in events {
            let json = serde_json::to_string(&event).expect("serialize");
            let parsed: StreamEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(event, parsed);
        }
    }

    #[test]
    fn test_serde_json_format() {
        let event = StreamEvent::with_timestamp(
            StreamEventKind::TextDelta {
                text: "Hello".to_string(),
            },
            100,
        );
        let json = serde_json::to_string(&event).unwrap();

        // Verify the JSON structure matches expected format
        assert!(json.contains("\"timestamp_ms\":100"));
        assert!(json.contains("\"type\":\"text_delta\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_non_terminal_events() {
        let events = vec![
            StreamEvent::text_delta("text"),
            StreamEvent::tool_start("Read", "t1", "{}"),
            StreamEvent::tool_result("Read", "t1", true),
            StreamEvent::session_assigned("sess-1"),
        ];

        for event in events {
            assert!(
                !event.is_terminal(),
                "Event should not be terminal: {:?}",
                event
            );
        }
    }
}
