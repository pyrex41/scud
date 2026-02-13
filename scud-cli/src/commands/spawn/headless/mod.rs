//! Headless agent execution with streaming output
//!
//! Provides infrastructure for running agents without tmux,
//! capturing structured JSON events for display in TUI/GUI.

pub mod events;
pub mod runner;
pub mod store;

#[cfg(feature = "direct-api")]
pub mod direct_api;

pub use events::{StreamEvent, StreamEventKind};
pub use runner::{
    create_runner, AnyRunner, ClaudeHeadless, HeadlessRunner, OpenCodeHeadless, SessionHandle,
};
#[cfg(feature = "direct-api")]
pub use direct_api::DirectApiRunner;
pub use store::{SessionStream, SessionStatus, StreamStore};