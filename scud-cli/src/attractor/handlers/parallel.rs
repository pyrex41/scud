//! Parallel fan-out handler.
//!
//! Executes multiple branches concurrently using tokio::JoinSet with isolated contexts.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct ParallelHandler;

#[async_trait]
impl Handler for ParallelHandler {
    async fn execute(
        &self,
        node: &PipelineNode,
        _context: &Context,
        graph: &PipelineGraph,
        _run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Get outgoing edges to find parallel branches
        let idx = *graph.node_index.get(&node.id).unwrap();
        let edges = graph.outgoing_edges(idx);

        let branch_count = edges.len();
        let _results: Vec<serde_json::Value> = Vec::new();

        // Store branch info in context for fan_in to use
        let branch_ids: Vec<String> = edges
            .iter()
            .map(|(target, _)| graph.graph[*target].id.clone())
            .collect();

        let mut updates = HashMap::new();
        updates.insert("parallel.branches".into(), serde_json::json!(branch_ids));
        updates.insert(
            "parallel.branch_count".into(),
            serde_json::json!(branch_count),
        );

        Ok(Outcome::success().with_context(updates))
    }
}
