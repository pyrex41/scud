//! TUI components for the GUI
//!
//! Reusable components for the user interface, providing model selection,
//! agent selection/display, and streaming output views.
//!
//! These components are inspired by TUI patterns and adapted for iced.

#[allow(dead_code)]
pub mod agent_selector;
#[allow(dead_code)]
pub mod model_selector;
#[allow(dead_code)]
pub mod streaming_view;

// Re-export commonly used types for convenience
#[allow(unused_imports)]
pub use agent_selector::{AgentDisplayStatus, AgentInfo, AgentSelectorState};
#[allow(unused_imports)]
pub use model_selector::{default_models, ModelOption, ModelSelectorState};
#[allow(unused_imports)]
pub use streaming_view::{OutputBuffer, OutputLine, OutputLineType, StreamingViewState};
