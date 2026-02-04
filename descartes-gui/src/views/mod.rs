//! View components for the GUI
//!
//! Individual view modules for different parts of the UI.

pub mod agents;
pub mod header;
pub mod monitor;
pub mod output;
pub mod waves;

// Re-export ViewMode from header for convenience
pub use header::ViewMode;
