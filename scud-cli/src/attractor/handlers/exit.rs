//! Exit node handler — no-op (engine checks goal gates after this).

use anyhow::Result;
use async_trait::async_trait;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct ExitHandler;

#[async_trait]
impl Handler for ExitHandler {
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
