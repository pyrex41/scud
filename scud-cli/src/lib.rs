//! # SCUD - Fast, DAG-driven Task Manager
//!
//! SCUD (Simple, Concurrent, Unified, Directed) is a task management system designed
//! for AI-driven development workflows. It provides both a CLI tool and a library
//! for managing tasks organized in directed acyclic graphs (DAGs).
//!
//! ## Features
//!
//! - **DAG-based execution**: Tasks have dependencies that form a directed acyclic graph,
//!   ensuring work proceeds in the correct order
//! - **Multi-phase support**: Organize tasks into phases (tags) for different project areas
//! - **File locking**: Safe concurrent access with atomic read/write operations
//! - **SCG format**: Token-efficient text format for task storage
//! - **AI integration**: Parse PRDs and expand complex tasks using LLM providers
//!
//! ## Library Usage
//!
//! SCUD can be used as a library to programmatically manage tasks:
//!
//! ```no_run
//! use scud::storage::Storage;
//! use scud::models::{Phase, Task, TaskStatus};
//!
//! // Initialize storage (uses current directory by default)
//! let storage = Storage::new(None);
//!
//! // Load all phases (task groups by tag)
//! let phases = storage.load_tasks().expect("Failed to load tasks");
//!
//! // Load a specific phase by tag
//! let auth_phase = storage.load_group("auth").expect("Phase not found");
//!
//! // Find the next available task (dependencies met, status pending)
//! if let Some(next_task) = auth_phase.find_next_task() {
//!     println!("Next task: {} - {}", next_task.id, next_task.title);
//! }
//!
//! // Get phase statistics
//! let stats = auth_phase.get_stats();
//! println!("Progress: {}/{} done", stats.done, stats.total);
//! ```
//!
//! ## Storage Format
//!
//! Tasks are stored in `.scud/tasks/tasks.scg` using the SCG (SCUD Graph) format.
//! Multiple phases are separated by `---` delimiters:
//!
//! ```text
//! @phase auth
//!
//! [1] Implement login endpoint
//! status: pending
//! complexity: 5
//! deps: 2, 3
//!
//! ---
//!
//! @phase api
//!
//! [1] Create REST framework
//! status: done
//! ```
//!
//! ## CLI Usage
//!
//! The `scud` binary provides commands for task management:
//!
//! - `scud init` - Initialize a new SCUD project
//! - `scud list` - List tasks in the active phase
//! - `scud next` - Find and optionally claim the next available task
//! - `scud show <id>` - Display task details
//! - `scud set-status <id> <status>` - Update task status
//! - `scud waves` - Show tasks organized by execution waves
//! - `scud stats` - Display completion statistics
//!
//! ## Task Generation Pipeline
//!
//! SCUD provides a multi-phase pipeline for generating tasks from PRD documents.
//! This can be invoked via the CLI (`scud generate`) or programmatically:
//!
//! ```no_run
//! use scud::commands::generate::{generate, GenerateOptions};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create options with required fields
//!     let mut options = GenerateOptions::new(
//!         PathBuf::from("docs/prd.md"),
//!         "my-feature".to_string(),
//!     );
//!
//!     // Customize the pipeline
//!     options.num_tasks = 15;        // Generate up to 15 tasks
//!     options.verbose = true;        // Show detailed output
//!     options.no_expand = false;     // Run expansion phase
//!     options.no_check_deps = false; // Run dependency validation
//!
//!     // Run the pipeline: parse → expand → check-deps
//!     generate(options).await?;
//!     Ok(())
//! }
//! ```
//!
//! The pipeline consists of three phases:
//!
//! 1. **Parse**: Convert a PRD document into initial tasks using AI
//! 2. **Expand**: Break down complex tasks into subtasks
//! 3. **Check Dependencies**: Validate and fix task dependencies
//!
//! Each phase can be skipped using `no_expand` and `no_check_deps` options.

/// Agent definitions for model routing.
///
/// Allows tasks to specify which AI harness and model should run them.
/// Agent definitions are loaded from `.scud/agents/<name>.toml` files.
///
/// # Example
///
/// ```toml
/// # .scud/agents/reviewer.toml
/// [agent]
/// name = "reviewer"
/// description = "Code review agent using smarter model"
///
/// [model]
/// harness = "claude"
/// model = "opus"
/// ```
pub mod agents;

/// Failure attribution using git blame.
///
/// Maps validation errors to specific tasks by parsing error output for
/// file:line references and using git blame to find which commits changed
/// those lines. Task IDs are extracted from commit message prefixes like `[TASK-ID]`.
///
/// # Example
///
/// ```no_run
/// use scud::attribution::{attribute_failure, AttributionConfidence};
/// use std::path::Path;
///
/// let working_dir = Path::new(".");
/// let wave_tasks = vec!["auth:1".to_string(), "auth:2".to_string()];
///
/// let attribution = attribute_failure(
///     working_dir,
///     "error: src/main.rs:42: undefined variable",
///     "",
///     &wave_tasks,
///     None,
/// ).unwrap();
///
/// match attribution.confidence {
///     AttributionConfidence::High => println!("Clear responsible: {:?}", attribution.responsible_tasks),
///     AttributionConfidence::Medium => println!("Multiple suspects: {:?}", attribution.responsible_tasks),
///     AttributionConfidence::Low => println!("Cannot determine - all tasks suspect"),
/// }
/// ```
pub mod attribution;

/// Backpressure validation for maintaining code quality during automated execution.
///
/// Runs programmatic validation (build, test, lint) after task completion.
/// See [`backpressure::run_validation`] for the main entry point.
pub mod backpressure;

/// CLI command implementations.
///
/// This module contains the implementation of all SCUD CLI commands including
/// task listing, status updates, AI-powered parsing, and more. Each submodule
/// corresponds to a CLI subcommand.
///
/// Key submodules:
/// - `ai` - AI-powered commands (parse PRD, expand tasks, analyze complexity)
/// - `generate` - Multi-phase task generation pipeline (parse → expand → check-deps)
/// - `list` - List tasks with filtering
/// - `next` - Find next available task
/// - `set_status` - Update task status
/// - `waves` - Display execution waves
/// - `stats` - Show completion statistics
/// - `spawn` - Parallel task execution
/// - `swarm` - Wave-based parallel execution with backpressure
///
/// ## Generate Pipeline
///
/// The [`commands::generate`] module provides a complete pipeline for task generation:
///
/// ```no_run
/// use scud::commands::generate::{generate, GenerateOptions};
/// use std::path::PathBuf;
///
/// # async fn example() -> anyhow::Result<()> {
/// let options = GenerateOptions::new(
///     PathBuf::from("prd.md"),
///     "feature".to_string(),
/// );
/// generate(options).await?;
/// # Ok(())
/// # }
/// ```
pub mod commands;

/// Configuration management for SCUD projects.
///
/// Handles loading and saving of `.scud/config.toml` which stores:
/// - LLM provider settings (provider, model, API endpoints)
/// - Model tiers (smart/fast models for different task types)
/// - Token limits and other provider-specific settings
///
/// # Example
///
/// ```no_run
/// use scud::config::Config;
/// use std::path::Path;
///
/// let config = Config::load(Path::new(".scud/config.toml"))
///     .unwrap_or_default();
/// println!("Using provider: {}", config.llm.provider);
/// ```
pub mod config;

/// Task graph serialization formats.
///
/// Provides parsers and serializers for the SCG (SCUD Graph) format,
/// a token-efficient text format designed for AI context windows.
///
/// # Format Overview
///
/// ```text
/// @phase my-project
///
/// [1] First task
/// status: pending
/// complexity: 3
///
/// [2] Second task
/// status: done
/// deps: 1
/// ```
///
/// Key exports:
/// - [`formats::parse_scg`] - Parse SCG text into a Phase
/// - [`formats::serialize_scg`] - Serialize a Phase to SCG text
/// - [`formats::Format`] - Enum of supported formats (SCG, JSON)
pub mod formats;

/// LLM client and prompt management.
///
/// Provides integration with various LLM providers for AI-powered features:
/// - PRD parsing to generate initial task lists
/// - Task expansion into subtasks
/// - Complexity analysis and dependency detection
///
/// Supported providers:
/// - `claude-cli` - Anthropic Claude via CLI
/// - `anthropic` - Direct Anthropic API
/// - `xai` - xAI/Grok API
/// - `openai` - OpenAI API
/// - `openrouter` - OpenRouter API
/// - `codex` - OpenAI Codex CLI
pub mod llm;

/// Core data models for tasks and phases.
///
/// This module defines the fundamental types used throughout SCUD:
///
/// - [`models::Task`] - Individual work items with status, complexity, dependencies
/// - [`models::Phase`] - A collection of related tasks (identified by a tag)
/// - [`models::TaskStatus`] - Task lifecycle states (Pending, InProgress, Done, etc.)
/// - [`models::Priority`] - Task priority levels (Critical, High, Medium, Low)
/// - [`models::IdFormat`] - ID generation strategy (Sequential or UUID)
///
/// # Example
///
/// ```
/// use scud::models::{Task, TaskStatus, Phase};
///
/// let mut phase = Phase::new("my-feature".to_string());
///
/// let mut task = Task::new(
///     "1".to_string(),
///     "Implement feature".to_string(),
///     "Add the new functionality".to_string(),
/// );
/// task.complexity = 5;
/// task.dependencies = vec!["setup:1".to_string()]; // Cross-phase dependency
///
/// phase.add_task(task);
///
/// // Find tasks ready to work on
/// if let Some(next) = phase.find_next_task() {
///     println!("Ready: {}", next.title);
/// }
/// ```
pub mod models;

/// File-based task storage with locking.
///
/// Manages reading and writing of task data to the filesystem with:
/// - File locking for safe concurrent access
/// - Caching of active phase selection
/// - Atomic read-modify-write operations
///
/// The storage layer handles:
/// - `.scud/tasks/tasks.scg` - Main task storage
/// - `.scud/active-tag` - Currently selected phase
/// - `.scud/config.toml` - Project configuration
/// - `.scud/guidance/*.md` - AI context files
///
/// # Example
///
/// ```no_run
/// use scud::storage::Storage;
/// use scud::models::TaskStatus;
///
/// let storage = Storage::new(None); // Use current directory
///
/// // Load and modify a phase atomically
/// let mut phase = storage.load_group("my-phase").unwrap();
/// if let Some(task) = phase.get_task_mut("1") {
///     task.set_status(TaskStatus::Done);
/// }
/// storage.update_group("my-phase", &phase).unwrap();
/// ```
pub mod storage;

/// Returns a greeting message.
///
/// # Example
///
/// ```
/// assert_eq!(scud::hello(), "Hello, World!");
/// ```
pub fn hello() -> &'static str {
    "Hello, World!"
}
