//! OpenCode Server integration
//!
//! Provides HTTP client and SSE event streaming for OpenCode Server mode.
//! This module enables SCUD to communicate with OpenCode's headless server
//! for agent orchestration with structured events and graceful cancellation.
//!
//! # Architecture
//!
//! ```text
//! SCUD Swarm ──HTTP──► OpenCode Server (localhost:4096)
//!                 ◄─SSE── real-time events
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use scud::opencode::{OpenCodeClient, OpenCodeManager};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Get global manager (auto-starts server if needed)
//!     let manager = scud::opencode::global_manager();
//!     manager.ensure_running().await?;
//!
//!     // Create session and send prompt
//!     let client = manager.client();
//!     let session = client.create_session("Task 1").await?;
//!     client.send_message(&session.id, "Implement feature X", None).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod events;
pub mod manager;
pub mod orchestrator;
pub mod types;

pub use client::OpenCodeClient;
pub use events::{EventStream, OpenCodeEvent};
pub use manager::{
    global_manager, init_global_manager, OpenCodeManager, ServerConfig, DEFAULT_PORT,
};
pub use orchestrator::{execute_wave_server, AgentHandle, AgentOrchestrator};
pub use types::*;
