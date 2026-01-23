//! JSON RPC IPC module for subagent communication
//!
//! Provides a JSON RPC 2.0 protocol for inter-process communication between
//! SCUD and external orchestrators. The server reads requests from stdin
//! and emits events/responses to stdout.
//!
//! ## Protocol
//!
//! Requests (stdin):
//! ```json
//! {"jsonrpc": "2.0", "method": "spawn", "params": {"task_id": "1", "prompt": "..."}, "id": 1}
//! {"jsonrpc": "2.0", "method": "ping", "id": 2}
//! {"jsonrpc": "2.0", "method": "shutdown", "id": 3}
//! ```
//!
//! Responses/Events (stdout):
//! ```json
//! {"jsonrpc": "2.0", "result": {"status": "ok"}, "id": 1}
//! {"jsonrpc": "2.0", "method": "agent.started", "params": {"task_id": "1"}}
//! {"jsonrpc": "2.0", "method": "agent.output", "params": {"task_id": "1", "line": "..."}}
//! {"jsonrpc": "2.0", "method": "agent.completed", "params": {"task_id": "1", "success": true}}
//! ```

mod server;
mod types;

pub use server::{RpcServer, RpcServerConfig};
pub use types::*;
