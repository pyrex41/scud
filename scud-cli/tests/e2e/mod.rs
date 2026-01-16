//! E2E test infrastructure for SCUD
//!
//! This module provides:
//! - `fixtures` - TestProject helper for creating temporary SCUD projects
//! - `mock_llm` - MockLLMClient for AI command testing
//! - `mock_terminal` - MockTerminalManager for spawn command testing

pub mod fixtures;
pub mod mock_llm;
pub mod mock_terminal;
