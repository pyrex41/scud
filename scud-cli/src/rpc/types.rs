//! JSON RPC 2.0 message types for IPC communication

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON RPC version constant
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON RPC Request (incoming from stdin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// JSON RPC version, must be "2.0"
    pub jsonrpc: String,
    /// Method name to invoke
    pub method: String,
    /// Method parameters (optional)
    #[serde(default)]
    pub params: Value,
    /// Request ID (optional for notifications)
    pub id: Option<RpcId>,
}

impl RpcRequest {
    /// Check if this request is a notification (no id)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON RPC Response (outgoing to stdout)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// JSON RPC version, always "2.0"
    pub jsonrpc: String,
    /// Success result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error result (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Request ID this is responding to
    pub id: RpcId,
}

impl RpcResponse {
    /// Create a success response
    pub fn success(id: RpcId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: RpcId, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

/// JSON RPC Notification (outgoing event, no id expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    /// JSON RPC version, always "2.0"
    pub jsonrpc: String,
    /// Event method name
    pub method: String,
    /// Event parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl RpcNotification {
    /// Create a new notification
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: Some(params),
        }
    }

    /// Create agent started event
    pub fn agent_started(task_id: &str) -> Self {
        Self::new(
            "agent.started",
            serde_json::json!({
                "task_id": task_id
            }),
        )
    }

    /// Create agent output event
    pub fn agent_output(task_id: &str, line: &str) -> Self {
        Self::new(
            "agent.output",
            serde_json::json!({
                "task_id": task_id,
                "line": line
            }),
        )
    }

    /// Create agent completed event
    pub fn agent_completed(
        task_id: &str,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            "agent.completed",
            serde_json::json!({
                "task_id": task_id,
                "success": success,
                "exit_code": exit_code,
                "duration_ms": duration_ms
            }),
        )
    }

    /// Create agent spawn failed event
    pub fn agent_spawn_failed(task_id: &str, error: &str) -> Self {
        Self::new(
            "agent.spawn_failed",
            serde_json::json!({
                "task_id": task_id,
                "error": error
            }),
        )
    }

    /// Create server ready event
    pub fn server_ready(version: &str) -> Self {
        Self::new(
            "server.ready",
            serde_json::json!({
                "version": version
            }),
        )
    }

    /// Create server shutdown event
    pub fn server_shutdown() -> Self {
        Self::new("server.shutdown", serde_json::json!({}))
    }
}

/// JSON RPC Error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new error with data
    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // Standard JSON RPC error codes
    pub fn parse_error(msg: &str) -> Self {
        Self::new(-32700, format!("Parse error: {}", msg))
    }

    pub fn invalid_request(msg: &str) -> Self {
        Self::new(-32600, format!("Invalid request: {}", msg))
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {}", method))
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self::new(-32602, format!("Invalid params: {}", msg))
    }

    pub fn internal_error(msg: &str) -> Self {
        Self::new(-32603, format!("Internal error: {}", msg))
    }

    // Custom error codes (application-specific, use -32000 to -32099)
    pub fn spawn_failed(msg: &str) -> Self {
        Self::new(-32001, format!("Agent spawn failed: {}", msg))
    }

    pub fn task_not_found(task_id: &str) -> Self {
        Self::new(-32002, format!("Task not found: {}", task_id))
    }
}

/// JSON RPC ID can be string, number, or null
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(untagged)]
pub enum RpcId {
    String(String),
    Number(i64),
    #[default]
    Null,
}

// ============================================================================
// Request Parameter Types
// ============================================================================

/// Parameters for the "spawn" method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnParams {
    /// Task ID to spawn agent for
    pub task_id: String,
    /// Prompt to send to the agent
    pub prompt: String,
    /// Working directory (optional, defaults to current dir)
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Harness to use: "claude" or "opencode" (optional, defaults to config)
    #[serde(default)]
    pub harness: Option<String>,
    /// Model to use (optional, defaults to config)
    #[serde(default)]
    pub model: Option<String>,
}

/// Parameters for the "spawn_task" method (spawns from task graph)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnTaskParams {
    /// Task ID in the task graph
    pub task_id: String,
    /// Phase tag (optional, uses active if not provided)
    #[serde(default)]
    pub tag: Option<String>,
    /// Harness to use (optional, defaults to config or task's agent_type)
    #[serde(default)]
    pub harness: Option<String>,
    /// Model to use (optional, defaults to config or task's agent_type)
    #[serde(default)]
    pub model: Option<String>,
}

/// Parameters for the "list_tasks" method
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListTasksParams {
    /// Phase tag (optional, uses active if not provided)
    #[serde(default)]
    pub tag: Option<String>,
    /// Filter by status
    #[serde(default)]
    pub status: Option<String>,
}

/// Parameters for the "get_task" method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTaskParams {
    /// Task ID
    pub task_id: String,
    /// Phase tag (optional, uses active if not provided)
    #[serde(default)]
    pub tag: Option<String>,
}

/// Parameters for the "set_status" method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStatusParams {
    /// Task ID
    pub task_id: String,
    /// New status
    pub status: String,
    /// Phase tag (optional, uses active if not provided)
    #[serde(default)]
    pub tag: Option<String>,
}

/// Parameters for the "next_task" method
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NextTaskParams {
    /// Phase tag (optional, uses active if not provided)
    #[serde(default)]
    pub tag: Option<String>,
    /// Search all tags
    #[serde(default)]
    pub all_tags: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let json = r#"{"jsonrpc": "2.0", "method": "spawn", "params": {"task_id": "1", "prompt": "test"}, "id": 1}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "spawn");
        assert_eq!(req.id, Some(RpcId::Number(1)));
    }

    #[test]
    fn test_parse_notification() {
        let json = r#"{"jsonrpc": "2.0", "method": "cancel", "params": {"task_id": "1"}}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert!(req.is_notification());
    }

    #[test]
    fn test_serialize_response() {
        let resp = RpcResponse::success(RpcId::Number(1), serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_notification() {
        let notif = RpcNotification::agent_started("task:1");
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("agent.started"));
        assert!(json.contains("task:1"));
    }

    #[test]
    fn test_error_codes() {
        let err = RpcError::method_not_found("unknown");
        assert_eq!(err.code, -32601);
    }

    #[test]
    fn test_spawn_params() {
        let json = r#"{"task_id": "1", "prompt": "do something", "harness": "claude"}"#;
        let params: SpawnParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.task_id, "1");
        assert_eq!(params.prompt, "do something");
        assert_eq!(params.harness, Some("claude".to_string()));
    }
}
