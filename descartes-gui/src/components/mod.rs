//! TUI components for the GUI
//!
//! Reusable components for the user interface, providing model selection,
//! agent selection/display, and streaming output views.
//!
//! These components are inspired by TUI patterns and adapted for iced.

pub mod agent_selector;
pub mod model_selector;
pub mod streaming_view;

// Re-export commonly used types for convenience
pub use agent_selector::{AgentDisplayStatus, AgentInfo, AgentSelectorState};
pub use model_selector::{default_models, ModelOption, ModelSelectorState};
pub use streaming_view::{OutputBuffer, OutputLine, OutputLineType, StreamingViewState};
