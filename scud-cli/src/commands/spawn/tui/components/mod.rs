//! TUI components for the swarm monitor
//!
//! This module contains reusable UI components for the terminal interface.
//! Components follow ratatui patterns and can be composed into larger views.
//!
//! ## Components
//!
//! - **ModelSelector**: For selecting AI models/harnesses when spawning agents
//! - **AgentSelector**: For selecting and switching between running agents
//! - **StreamingView**: For displaying streaming output from agents
//!
//! ## Usage
//!
//! Components are designed to be used with ratatui's `StatefulWidget` pattern:
//!
//! ```ignore
//! use scud_cli::commands::spawn::tui::components::*;
//!
//! // Create widget state
//! let mut state = ModelSelectorState::new(0);
//!
//! // Render widget with state
//! let widget = ModelSelector::new(&models).focused(true);
//! frame.render_stateful_widget(widget, area, &mut state);
//!
//! // Handle input to update state
//! state.next(models.len());
//! ```

pub mod agent_selector;
pub mod model_selector;
pub mod streaming_view;

// Re-export key types for convenient access
pub use agent_selector::{
    AgentBadge, AgentDisplayStatus, AgentInfo, AgentSelector, AgentSelectorState,
};
pub use model_selector::{
    default_models, ModelOption, ModelSelector, ModelSelectorCompact, ModelSelectorState,
};
pub use streaming_view::{
    OutputDisplay, OutputLine, OutputLineType, StreamingView, StreamingViewState,
    StreamingViewStrings,
};
