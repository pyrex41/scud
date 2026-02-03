//! Headless agent execution with streaming output
//!
//! Provides infrastructure for running agents without tmux,
//! capturing structured JSON events for display in TUI/GUI.

pub mod events;
pub mod runner;
pub mod store;

pub use events::{StreamEvent, StreamEventKind};
pub use runner::{
    create_runner, AnyRunner, ClaudeHeadless, HeadlessRunner, OpenCodeHeadless, SessionHandle,
};
pub use store::{SessionStream, SessionStatus, StreamStore};