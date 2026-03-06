//! OpenCode Server API types
//!
//! Type definitions matching the OpenCode Server REST API schema.

use serde::{Deserialize, Serialize};

/// Session creation request
#[derive(Debug, Serialize)]
pub struct CreateSessionRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

/// Session response from server
#[derive(Debug, Deserialize, Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub message_count: usize,
}

/// Message/prompt request
#[derive(Debug, Serialize)]
pub struct MessageRequest {
    pub parts: Vec<MessagePart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelSpec>,
}

/// Part of a message (currently only text supported)
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text { text: String },
}

/// Model specification for a message
#[derive(Debug, Serialize)]
pub struct ModelSpec {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// Session status response
#[derive(Debug, Deserialize)]
pub struct SessionStatus {
    pub id: String,
    pub status: SessionState,
    #[serde(default)]
    pub active_message: Option<String>,
}

/// State of a session
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Running,
    Completed,
    Error,
}

/// Server info response
#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub ready: bool,
}

/// Error response from server
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session_request_serialization() {
        let request = CreateSessionRequest {
            title: "Test Session".to_string(),
            system_prompt: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"title\":\"Test Session\""));
        // system_prompt should be omitted when None
        assert!(!json.contains("system_prompt"));
    }

    #[test]
    fn test_create_session_request_with_system_prompt() {
        let request = CreateSessionRequest {
            title: "Test".to_string(),
            system_prompt: Some("You are helpful".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("system_prompt"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn test_message_part_serialization() {
        let part = MessagePart::Text {
            text: "Hello".to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_model_spec_serialization() {
        let spec = ModelSpec {
            provider_id: "xai".to_string(),
            model_id: "grok-3".to_string(),
        };

        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"providerID\":\"xai\""));
        assert!(json.contains("\"modelID\":\"grok-3\""));
    }

    #[test]
    fn test_session_deserialization() {
        let json =
            r#"{"id": "abc123", "title": "Test", "created_at": "2024-01-01", "message_count": 5}"#;
        let session: Session = serde_json::from_str(json).unwrap();

        assert_eq!(session.id, "abc123");
        assert_eq!(session.title, "Test");
        assert_eq!(session.message_count, 5);
    }

    #[test]
    fn test_session_deserialization_minimal() {
        // Server might not always include all fields
        let json = r#"{"id": "abc", "title": "Test"}"#;
        let session: Session = serde_json::from_str(json).unwrap();

        assert_eq!(session.id, "abc");
        assert_eq!(session.message_count, 0); // default
    }

    #[test]
    fn test_session_state_deserialization() {
        let json = r#""running""#;
        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state, SessionState::Running);

        let json = r#""idle""#;
        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state, SessionState::Idle);
    }
}
