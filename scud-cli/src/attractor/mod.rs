//! Attractor Mode — DOT-based AI workflow graph execution.
//!
//! Attractor Mode lets you define multi-step AI pipelines as DOT graphs
//! where nodes represent tasks (LLM calls, human gates, tool invocations)
//! and edges represent transitions with optional conditions.
//!
//! # Architecture
//!
//! ```text
//! DOT file → Parser → PipelineGraph → Transforms → Validator → Runner
//!                                                                 ↓
//!                                           AgentBackend ← Handler Registry
//! ```
//!
//! # Modules
//!
//! - [`dot_parser`] — Parse DOT subset into raw graph structures
//! - [`graph`] — Internal petgraph-based pipeline representation
//! - [`context`] — Thread-safe key-value execution context
//! - [`checkpoint`] — Checkpoint save/load for resumption
//! - [`run_directory`] — Filesystem layout for pipeline runs
//! - [`outcome`] — Stage execution outcomes and status
//! - [`runner`] — Core execution engine with edge selection
//! - [`conditions`] — Condition expression parser/evaluator
//! - [`retry`] — Retry policies with exponential backoff
//! - [`events`] — Pipeline execution events for observability
//! - [`handlers`] — Node handler trait and implementations
//! - [`interviewer`] — Human-in-the-loop interaction trait
//! - [`transforms`] — Pre-execution graph transforms
//! - [`stylesheet`] — CSS-like model/provider styling
//! - [`validator`] — Graph lint rules and validation

pub mod checkpoint;
pub mod conditions;
pub mod context;
pub mod dot_parser;
pub mod events;
pub mod graph;
pub mod scg_bridge;
pub mod handlers;
pub mod interviewer;
pub mod outcome;
pub mod retry;
pub mod run_directory;
pub mod runner;
pub mod stylesheet;
pub mod transforms;
pub mod validator;
