//! Fan-in / consolidation handler.
//!
//! Reads parallel.results from context, ranks by status, selects best.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::attractor::context::Context;
use crate::attractor::graph::{PipelineGraph, PipelineNode};
use crate::attractor::outcome::Outcome;
use crate::attractor::run_directory::RunDirectory;

use super::Handler;

pub struct FanInHandler;

#[async_trait]
impl Handler for FanInHandler {
    async fn execute(
        &self,
        _node: &PipelineNode,
        context: &Context,
        _graph: &PipelineGraph,
        _run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        // Read parallel results from context
        let results = context.get("parallel.results").await;

        let mut updates = HashMap::new();

        if let Some(results) = results {
            if let Some(arr) = results.as_array() {
                // Select the first successful result
                let best = arr.iter().find(|r| {
                    r.get("status")
                        .and_then(|s| s.as_str())
                        .map(|s| s == "success")
                        .unwrap_or(false)
                });

                if let Some(best) = best {
                    updates.insert("parallel.best_result".into(), best.clone());
                }
            }
        }

        Ok(Outcome::success().with_context(updates))
    }
}
