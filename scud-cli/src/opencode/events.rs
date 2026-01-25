//! SSE event streaming for OpenCode Server
//!
//! Provides real-time event streaming via Server-Sent Events (SSE) for
//! monitoring agent execution, tool calls, and completion status.

use anyhow::Result;
use eventsource_client::{self as es, Client};
use futures::stream::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;

/// Events received from OpenCode Server SSE stream
#[derive(Debug, Clone)]
pub enum OpenCodeEvent {
    /// Server connected
    Connected,

    /// Message started
    MessageStart {
        session_id: String,
        message_id: String,
    },

    /// Text delta (streaming output)
    TextDelta { session_id: String, text: String },

    /// Tool execution started
    ToolStart {
        session_id: String,
        tool_id: String,
        tool_name: String,
        input: serde_json::Value,
    },

    /// Tool execution completed
    ToolResult {
        session_id: String,
        tool_id: String,
        tool_name: String,
        output: String,
        success: bool,
    },

    /// Message completed
    MessageComplete { session_id: String, success: bool },

    /// Session error
    SessionError { session_id: String, error: String },

    /// Unknown event type
    Unknown { event_type: String, data: String },
}

/// Raw SSE event from server
#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(flatten)]
    data: serde_json::Value,
}

impl OpenCodeEvent {
    /// Parse from SSE event data
    pub fn parse(event_type: &str, data: &str) -> Self {
        // Try to parse as JSON
        let parsed: Result<RawEvent, _> = serde_json::from_str(data);

        match parsed {
            Ok(raw) => Self::from_raw(&raw),
            Err(_) => {
                // Fallback for non-JSON events
                match event_type {
                    "server.connected" | "connected" => OpenCodeEvent::Connected,
                    _ => OpenCodeEvent::Unknown {
                        event_type: event_type.to_string(),
                        data: data.to_string(),
                    },
                }
            }
        }
    }

    fn from_raw(raw: &RawEvent) -> Self {
        let session_id = raw.session_id.clone().unwrap_or_default();

        match raw.event_type.as_str() {
            "message.start" => OpenCodeEvent::MessageStart {
                session_id,
                message_id: raw
                    .data
                    .get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },

            "text.delta" | "content.delta" => OpenCodeEvent::TextDelta {
                session_id,
                text: raw
                    .data
                    .get("text")
                    .or_else(|| raw.data.get("delta"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            },

            "tool.start" | "tool_use.start" => OpenCodeEvent::ToolStart {
                session_id,
                tool_id: raw
                    .data
                    .get("tool_id")
                    .or_else(|| raw.data.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_name: raw
                    .data
                    .get("tool")
                    .or_else(|| raw.data.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                input: raw
                    .data
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            },

            "tool.result" | "tool_use.result" => OpenCodeEvent::ToolResult {
                session_id,
                tool_id: raw
                    .data
                    .get("tool_id")
                    .or_else(|| raw.data.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_name: raw
                    .data
                    .get("tool")
                    .or_else(|| raw.data.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                output: raw
                    .data
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                success: raw
                    .data
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            },

            "message.complete" | "message.done" => OpenCodeEvent::MessageComplete {
                session_id,
                success: raw
                    .data
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            },

            "session.error" | "error" => OpenCodeEvent::SessionError {
                session_id,
                error: raw
                    .data
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string(),
            },

            _ => OpenCodeEvent::Unknown {
                event_type: raw.event_type.clone(),
                data: serde_json::to_string(&raw.data).unwrap_or_default(),
            },
        }
    }

    /// Get session ID if event is session-specific
    pub fn session_id(&self) -> Option<&str> {
        match self {
            OpenCodeEvent::MessageStart { session_id, .. } => Some(session_id),
            OpenCodeEvent::TextDelta { session_id, .. } => Some(session_id),
            OpenCodeEvent::ToolStart { session_id, .. } => Some(session_id),
            OpenCodeEvent::ToolResult { session_id, .. } => Some(session_id),
            OpenCodeEvent::MessageComplete { session_id, .. } => Some(session_id),
            OpenCodeEvent::SessionError { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    /// Check if this event indicates a session has ended
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OpenCodeEvent::MessageComplete { .. } | OpenCodeEvent::SessionError { .. }
        )
    }
}

/// Event stream subscription
pub struct EventStream {
    rx: mpsc::Receiver<OpenCodeEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventStream {
    /// Create a new event stream connected to the given URL
    pub async fn connect(url: &str) -> Result<Self> {
        let (tx, rx) = mpsc::channel(1000);
        let url = url.to_string();

        let handle = tokio::spawn(async move {
            let client = match es::ClientBuilder::for_url(&url) {
                Ok(builder) => builder.build(),
                Err(e) => {
                    eprintln!("Failed to create SSE client: {}", e);
                    return;
                }
            };

            let mut stream = Box::pin(client.stream());

            while let Some(event) = stream.next().await {
                match event {
                    Ok(es::SSE::Event(ev)) => {
                        let parsed = OpenCodeEvent::parse(&ev.event_type, &ev.data);
                        if tx.send(parsed).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Ok(es::SSE::Comment(_)) => continue,
                    Ok(es::SSE::Connected(_)) => {
                        let _ = tx.send(OpenCodeEvent::Connected).await;
                    }
                    Err(e) => {
                        eprintln!("SSE error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self { rx, _handle: handle })
    }

    /// Receive next event
    pub async fn recv(&mut self) -> Option<OpenCodeEvent> {
        self.rx.recv().await
    }

    /// Try to receive without blocking
    pub fn try_recv(&mut self) -> Option<OpenCodeEvent> {
        self.rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_start() {
        let data =
            r#"{"type": "message.start", "session_id": "abc123", "message_id": "msg1"}"#;
        let event = OpenCodeEvent::parse("message", data);

        match event {
            OpenCodeEvent::MessageStart {
                session_id,
                message_id,
            } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(message_id, "msg1");
            }
            _ => panic!("Expected MessageStart, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_text_delta() {
        let data = r#"{"type": "text.delta", "session_id": "abc", "text": "Hello world"}"#;
        let event = OpenCodeEvent::parse("text", data);

        match event {
            OpenCodeEvent::TextDelta { session_id, text } => {
                assert_eq!(session_id, "abc");
                assert_eq!(text, "Hello world");
            }
            _ => panic!("Expected TextDelta, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_text_delta_with_delta_field() {
        // Some servers use "delta" instead of "text"
        let data = r#"{"type": "content.delta", "session_id": "abc", "delta": "content"}"#;
        let event = OpenCodeEvent::parse("content", data);

        match event {
            OpenCodeEvent::TextDelta { text, .. } => {
                assert_eq!(text, "content");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_parse_tool_start() {
        let data = r#"{"type": "tool.start", "session_id": "abc123", "tool": "read_file", "input": {"path": "src/main.rs"}}"#;
        let event = OpenCodeEvent::parse("tool", data);

        match event {
            OpenCodeEvent::ToolStart {
                session_id,
                tool_name,
                input,
                ..
            } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(tool_name, "read_file");
                assert_eq!(input["path"], "src/main.rs");
            }
            _ => panic!("Expected ToolStart, got {:?}", event),
        }
    }

    #[test]
    fn test_parse_tool_result() {
        let data = r#"{"type": "tool.result", "session_id": "abc", "tool": "bash", "output": "done", "success": true}"#;
        let event = OpenCodeEvent::parse("tool", data);

        match event {
            OpenCodeEvent::ToolResult {
                tool_name,
                output,
                success,
                ..
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(output, "done");
                assert!(success);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_message_complete() {
        let data = r#"{"type": "message.complete", "session_id": "xyz", "success": true}"#;
        let event = OpenCodeEvent::parse("message", data);

        match event {
            OpenCodeEvent::MessageComplete { session_id, success } => {
                assert_eq!(session_id, "xyz");
                assert!(success);
            }
            _ => panic!("Expected MessageComplete"),
        }
    }

    #[test]
    fn test_parse_session_error() {
        let data = r#"{"type": "session.error", "session_id": "err1", "error": "Connection failed"}"#;
        let event = OpenCodeEvent::parse("error", data);

        match event {
            OpenCodeEvent::SessionError { session_id, error } => {
                assert_eq!(session_id, "err1");
                assert_eq!(error, "Connection failed");
            }
            _ => panic!("Expected SessionError"),
        }
    }

    #[test]
    fn test_parse_unknown_event() {
        let data = r#"{"type": "custom.event", "foo": "bar"}"#;
        let event = OpenCodeEvent::parse("custom", data);

        match event {
            OpenCodeEvent::Unknown { event_type, .. } => {
                assert_eq!(event_type, "custom.event");
            }
            _ => panic!("Expected Unknown"),
        }
    }

    #[test]
    fn test_parse_connected() {
        let event = OpenCodeEvent::parse("server.connected", "");
        assert!(matches!(event, OpenCodeEvent::Connected));

        let event = OpenCodeEvent::parse("connected", "");
        assert!(matches!(event, OpenCodeEvent::Connected));
    }

    #[test]
    fn test_session_id_extraction() {
        let event = OpenCodeEvent::TextDelta {
            session_id: "test123".to_string(),
            text: "hi".to_string(),
        };
        assert_eq!(event.session_id(), Some("test123"));

        let event = OpenCodeEvent::Connected;
        assert_eq!(event.session_id(), None);
    }

    #[test]
    fn test_is_terminal() {
        let complete = OpenCodeEvent::MessageComplete {
            session_id: "a".to_string(),
            success: true,
        };
        assert!(complete.is_terminal());

        let error = OpenCodeEvent::SessionError {
            session_id: "b".to_string(),
            error: "fail".to_string(),
        };
        assert!(error.is_terminal());

        let delta = OpenCodeEvent::TextDelta {
            session_id: "c".to_string(),
            text: "hi".to_string(),
        };
        assert!(!delta.is_terminal());
    }
}
