//! Sync SCUD tasks to Claude Code's Tasks format
//!
//! This module provides bidirectional sync between SCUD's task storage
//! and Claude Code's native Tasks system (`~/.claude/tasks/`).
//!
//! # Overview
//!
//! Claude Code has a built-in Tasks feature that agents can access via
//! `TaskList`, `TaskUpdate`, `TaskCreate` tools. By syncing SCUD tasks
//! to this format, agents can see the full task list and dependencies
//! during execution.
//!
//! # Usage
//!
//! ```no_run
//! use scud::sync::claude_tasks;
//! use scud::models::phase::Phase;
//!
//! let phase = Phase::new("my-feature".to_string());
//! // ... add tasks to phase ...
//!
//! // Sync to Claude Tasks format
//! let task_file = claude_tasks::sync_phase(&phase, "my-feature").unwrap();
//! println!("Synced to: {}", task_file.display());
//!
//! // Get the task list ID for environment variable
//! let list_id = claude_tasks::task_list_id("my-feature");
//! // Set CLAUDE_CODE_TASK_LIST_ID={list_id} when spawning agents
//! ```

pub mod claude_tasks;

pub use claude_tasks::*;
