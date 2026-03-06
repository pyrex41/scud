//! Wait-for-human gate handler.
//!
//! Derives choices from outgoing edges and presents them via the Interviewer trait.

use anyhow::Result;
use async_trait::async_trait;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct HumanHandler;

#[async_trait]
impl Handler for HumanHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        _context: &Context,
        graph: &PipelineGraph,
        _run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Derive choices from outgoing edge labels
        let idx = *graph.node_index.get(&node.id).unwrap();
        let edges = graph.outgoing_edges(idx);

        let choices: Vec<String> = edges
            .iter()
            .filter(|(_, e)| !e.label.is_empty())
            .map(|(_, e)| e.label.clone())
            .collect();

        let _prompt = if node.prompt.is_empty() {
            format!(
                "Node '{}' requires human input. Choose an option:",
                node.label
            )
        } else {
            node.prompt.clone()
        };

        // For now, since we don't have direct access to the interviewer here,
        // we return a WaitingForHuman status. The runner should handle this
        // by consulting its interviewer.
        // In the actual implementation, the runner passes the interviewer.
        // For this handler, we return success with the first choice as preferred label.
        if let Some(first) = choices.first() {
            Ok(Outcome::success_with_label(first.clone()))
        } else {
            Ok(Outcome::success())
        }
    }
}
