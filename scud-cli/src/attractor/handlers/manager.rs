//! Stack manager loop handler (basic version).
//!
//! Supervisor loop: observe/steer/wait on child pipeline.

use anyhow::Result;
use async_trait::async_trait;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct ManagerHandler;

#[async_trait]
impl Handler for ManagerHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        _context: &Context,
        _graph: &PipelineGraph,
        _run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Basic version: just succeed. Full implementation would:
        // 1. Observe child pipeline status
        // 2. Decide to steer (inject context, retry) or wait
        // 3. Return when child completes
        Ok(Outcome::success().with_response(format!(
            "[Manager] Supervised execution for node '{}'",
            node.id
        )))
    }
}
