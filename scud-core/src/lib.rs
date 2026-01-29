//! # SCUD Core - Shared Types for SCUD Task Management
//!
//! This crate provides the core data types and utilities shared between
//! SCUD CLI and Descartes GUI applications.
//!
//! ## Features
//!
//! - **Task types**: [`Task`], [`TaskStatus`], [`Priority`]
//! - **Phase management**: [`Phase`], [`PhaseStats`], [`IdFormat`]
//! - **SCG format**: Token-efficient text format for task graphs
//! - **Wave computation**: Parallel execution wave calculation
//! - **Storage**: File-based task persistence with locking
//!
//! ## Example
//!
//! ```
//! use scud_core::{Task, Phase, compute_waves};
//!
//! // Create a phase with tasks
//! let mut phase = Phase::new("my-feature".to_string());
//!
//! let task1 = Task::new(
//!     "1".to_string(),
//!     "Implement feature".to_string(),
//!     "Add the new functionality".to_string(),
//! );
//!
//! let mut task2 = Task::new(
//!     "2".to_string(),
//!     "Write tests".to_string(),
//!     "Add test coverage".to_string(),
//! );
//! task2.dependencies = vec!["1".to_string()];
//!
//! phase.add_task(task1);
//! phase.add_task(task2);
//!
//! // Find tasks ready to work on
//! if let Some(next) = phase.find_next_task() {
//!     println!("Ready: {}", next.title);
//! }
//!
//! // Compute parallel execution waves
//! let task_refs: Vec<&Task> = phase.tasks.iter().collect();
//! let result = compute_waves(&task_refs);
//! println!("Waves: {}", result.waves.len());
//! ```

/// Core data models for tasks and phases.
///
/// This module defines the fundamental types used throughout SCUD:
///
/// - [`models::Task`] - Individual work items with status, complexity, dependencies
/// - [`models::Phase`] - A collection of related tasks (identified by a tag)
/// - [`models::TaskStatus`] - Task lifecycle states (Pending, InProgress, Done, etc.)
/// - [`models::Priority`] - Task priority levels (Critical, High, Medium, Low)
/// - [`models::IdFormat`] - ID generation strategy (Sequential or UUID)
pub mod models;

/// Task graph serialization formats.
///
/// Provides parsers and serializers for the SCG (SCUD Graph) format,
/// a token-efficient text format designed for AI context windows.
///
/// Key exports:
/// - [`formats::parse_scg`] - Parse SCG text into a Phase
/// - [`formats::serialize_scg`] - Serialize a Phase to SCG text
/// - [`formats::Format`] - Enum of supported formats (SCG, JSON)
pub mod formats;

/// Wave computation for parallel task execution.
///
/// Computes execution waves using topological sort, grouping tasks
/// that can run in parallel based on their dependencies.
pub mod waves;

/// File-based task storage with locking.
///
/// Manages reading and writing of task data to the filesystem with:
/// - File locking for safe concurrent access
/// - Caching of active phase selection
/// - Atomic read-modify-write operations
pub mod storage;

/// Event publishing system using ZMQ.
///
/// Provides ZMQ-based event publishing for task management events.
/// Supports PUB socket for broadcasting events and REP socket for queries.
pub mod publisher;

// Re-export commonly used types at the crate root for ergonomic API
pub use models::{IdFormat, Phase, PhaseStats, Priority, Task, TaskStatus};

// Re-export format utilities
pub use formats::{natural_sort_ids, parse_scg, serialize_scg, Format};

// Re-export wave computation types and functions
pub use waves::{compute_waves, detect_id_collisions, Wave, WaveResult};

// Re-export storage
pub use storage::Storage;
