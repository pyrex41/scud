//! Node handler trait and registry.

pub mod codergen;
pub mod conditional;
pub mod exit;
pub mod fan_in;
pub mod human;
pub mod manager;
pub mod parallel;
pub mod start;
pub mod tool;

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use super::context::Context;
use super::graph::{PipelineGraph, PipelineNode};
use super::outcome::Outcome;
use super::run_directory::RunDirectory;

/// Trait for node handlers that execute pipeline stages.
#[async_trait]
pub trait Handler: Send + Sync {
    /// Execute this handler for the given node.
    async fn execute(
        &self,
        node: &PipelineNode,
        context: &Context,
        graph: &PipelineGraph,
        run_dir: &RunDirectory,
    ) -> Result<Outcome>;
}

/// Registry that maps handler type names to handler implementations.
pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn Handler>>,
    default_handler: Box<dyn Handler>,
}

impl HandlerRegistry {
    /// Create a new registry with default handlers.
    pub fn new_with_defaults() -> Self {
        let mut handlers: HashMap<String, Box<dyn Handler>> = HashMap::new();
        handlers.insert("start".into(), Box::new(start::StartHandler));
        handlers.insert("exit".into(), Box::new(exit::ExitHandler));
        handlers.insert(
            "conditional".into(),
            Box::new(conditional::ConditionalHandler),
        );
        handlers.insert("wait.human".into(), Box::new(human::HumanHandler));
        handlers.insert("tool".into(), Box::new(tool::ToolHandler));
        handlers.insert("parallel".into(), Box::new(parallel::ParallelHandler));
        handlers.insert("parallel.fan_in".into(), Box::new(fan_in::FanInHandler));
        handlers.insert(
            "stack.manager_loop".into(),
            Box::new(manager::ManagerHandler),
        );

        Self {
            handlers,
            default_handler: Box::new(codergen::CodergenHandler::simulated()),
        }
    }

    /// Create a registry with a codergen handler backed by an AgentBackend.
    ///
    /// Both the explicit "codergen" entry and the default handler use the
    /// provided backend, so unknown handler types also get real LLM execution.
    pub fn with_backend(backend: std::sync::Arc<dyn crate::backend::AgentBackend>) -> Self {
        let mut registry = Self::new_with_defaults();
        registry.handlers.insert(
            "codergen".into(),
            Box::new(codergen::CodergenHandler::new(backend.clone())),
        );
        registry.default_handler = Box::new(codergen::CodergenHandler::new(backend));
        registry
    }

    /// Register a custom handler.
    pub fn register(&mut self, handler_type: &str, handler: Box<dyn Handler>) {
        self.handlers.insert(handler_type.to_string(), handler);
    }

    /// Get the handler for a given type, falling back to the default.
    pub fn get(&self, handler_type: &str) -> &dyn Handler {
        self.handlers
            .get(handler_type)
            .map(|h| h.as_ref())
            .unwrap_or(self.default_handler.as_ref())
    }
}
