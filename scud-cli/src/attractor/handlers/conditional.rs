//! Conditional node handler — routing pass-through.
//!
//! The conditional handler itself is a no-op. Routing happens
//! in the runner's edge selection algorithm based on edge conditions.

use anyhow::Result;
use async_trait::async_trait;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct ConditionalHandler;

#[async_trait]
impl Handler for ConditionalHandler {
    async fn execute(
        &self,
        _node: &PipelineNode,
        _context: &Context,
        _graph: &PipelineGraph,
        _run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        Ok(Outcome::success())
    }
}
