//! Core pipeline execution engine.
//!
//! Implements the lifecycle: PARSE → VALIDATE → INITIALIZE → EXECUTE → FINALIZE.
//! The core loop follows the spec Section 3.2 exactly:
//! 1. Execute node handler → get Outcome
//! 2. Apply context_updates from Outcome
//! 3. Save checkpoint
//! 4. Select next edge (5-step algorithm)
//! 5. At terminal nodes: check goal gates → retry_target if unsatisfied
//! 6. Retry logic with exponential backoff + jitter

use anyhow::{Context, Result};
use petgraph::graph::NodeIndex;
use std::time::Instant;
use tokio::sync::mpsc;

use super::checkpoint::Checkpoint;
use super::conditions::{evaluate_condition, parse_condition};
use super::context::{Context as PipelineContext, ContextSnapshot};
use super::events::PipelineEvent;
use super::graph::{PipelineGraph, PipelineNode};
use super::handlers::HandlerRegistry;
use super::interviewer::Interviewer;
use super::outcome::{Outcome, StageStatus};
use super::retry::RetryPolicy;
use super::run_directory::RunDirectory;

/// The pipeline execution engine.
pub struct PipelineRunner {
    pub handler_registry: HandlerRegistry,
    pub interviewer: Box<dyn Interviewer>,
    pub event_tx: Option<mpsc::Sender<PipelineEvent>>,
}

impl PipelineRunner {
    /// Create a new PipelineRunner.
    pub fn new(
        handler_registry: HandlerRegistry,
        interviewer: Box<dyn Interviewer>,
    ) -> Self {
        Self {
            handler_registry,
            interviewer,
            event_tx: None,
        }
    }

    /// Set the event channel for observability.
    pub fn with_events(mut self, tx: mpsc::Sender<PipelineEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Execute a pipeline graph from start to finish.
    pub async fn run(
        &self,
        graph: &PipelineGraph,
        context: &PipelineContext,
        run_dir: &RunDirectory,
        checkpoint: Option<Checkpoint>,
    ) -> Result<StageStatus> {
        let run_start = Instant::now();
        let run_id = run_dir
            .root()
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        self.emit(PipelineEvent::pipeline_started(&graph.name, &run_id))
            .await;

        // Determine starting node
        let (mut current_idx, mut cp) = if let Some(cp) = checkpoint {
            // Resume from checkpoint
            let idx = *graph
                .node_index
                .get(&cp.current_node)
                .context("Checkpoint node not found in graph")?;
            (idx, cp)
        } else {
            let snap = ContextSnapshot::from(context.snapshot().await);
            let cp = Checkpoint::new(
                &graph.graph[graph.start_node].id,
                snap,
            );
            (graph.start_node, cp)
        };

        let mut nodes_executed = 0;

        loop {
            let node = &graph.graph[current_idx];
            let node_id = node.id.clone();
            let node_start = Instant::now();

            self.emit(PipelineEvent::node_started(&node_id, &node.handler_type))
                .await;

            // Execute handler
            let outcome = self.execute_node(node, context, graph, run_dir).await?;

            let duration_ms = node_start.elapsed().as_millis() as u64;
            self.emit(PipelineEvent::node_completed(
                &node_id,
                outcome.status.clone(),
                duration_ms,
            ))
            .await;

            nodes_executed += 1;

            // Apply context updates
            if !outcome.context_updates.is_empty() {
                context.apply_updates(&outcome.context_updates).await;
            }

            // Update checkpoint
            cp.current_node = node_id.clone();
            cp.mark_completed(&node_id, outcome.status.clone());
            cp.context = ContextSnapshot::from(context.snapshot().await);
            cp.save(&run_dir.checkpoint_path())?;

            self.emit(PipelineEvent::CheckpointSaved {
                node_id: node_id.clone(),
            })
            .await;

            // Check if this is a terminal node (exit)
            if node.handler_type == "exit" {
                // Goal gate check
                let goal_satisfied = self.check_goal_gates(graph, &outcome, context).await;

                self.emit(PipelineEvent::GoalGateCheck {
                    node_id: node_id.clone(),
                    satisfied: goal_satisfied,
                })
                .await;

                if goal_satisfied {
                    let total_ms = run_start.elapsed().as_millis() as u64;
                    self.emit(PipelineEvent::PipelineCompleted {
                        status: StageStatus::Success,
                        total_duration_ms: total_ms,
                        nodes_executed,
                    })
                    .await;
                    return Ok(StageStatus::Success);
                } else if let Some(ref retry_target) = node.retry_target {
                    // Route to retry target
                    if let Some(&target_idx) = graph.node_index.get(retry_target) {
                        current_idx = target_idx;
                        continue;
                    }
                }
                // No retry target or not found — finish anyway
                let total_ms = run_start.elapsed().as_millis() as u64;
                self.emit(PipelineEvent::PipelineCompleted {
                    status: outcome.status.clone(),
                    total_duration_ms: total_ms,
                    nodes_executed,
                })
                .await;
                return Ok(outcome.status);
            }

            // Handle failure with retry logic
            if !outcome.status.is_success() && outcome.status != StageStatus::Skipped {
                let policy = RetryPolicy::from_max_retries(node.max_retries);
                let attempt = cp.increment_retry(&node_id);

                if policy.should_retry(attempt) {
                    let delay = policy.delay_for_attempt(attempt);
                    self.emit(PipelineEvent::RetryScheduled {
                        node_id: node_id.clone(),
                        attempt,
                        max_retries: node.max_retries,
                        delay_ms: delay.as_millis() as u64,
                    })
                    .await;

                    tokio::time::sleep(delay).await;
                    // Stay on the same node for retry
                    continue;
                }

                // Exhausted retries — check for retry_target
                if let Some(ref target) = node.retry_target {
                    if let Some(&target_idx) = graph.node_index.get(target) {
                        current_idx = target_idx;
                        continue;
                    }
                }
                if let Some(ref fallback) = node.fallback_retry_target {
                    if let Some(&target_idx) = graph.node_index.get(fallback) {
                        current_idx = target_idx;
                        continue;
                    }
                }

                // No retry options — fail the pipeline
                let total_ms = run_start.elapsed().as_millis() as u64;
                self.emit(PipelineEvent::PipelineCompleted {
                    status: StageStatus::Failure,
                    total_duration_ms: total_ms,
                    nodes_executed,
                })
                .await;
                return Ok(StageStatus::Failure);
            }

            // Select next edge (5-step algorithm)
            let next_idx = self
                .select_next_edge(graph, current_idx, &outcome, context)
                .await?;

            match next_idx {
                Some(idx) => current_idx = idx,
                None => {
                    // No outgoing edges — treat as pipeline completion
                    let total_ms = run_start.elapsed().as_millis() as u64;
                    self.emit(PipelineEvent::PipelineCompleted {
                        status: outcome.status,
                        total_duration_ms: total_ms,
                        nodes_executed,
                    })
                    .await;
                    return Ok(StageStatus::Success);
                }
            }
        }
    }

    /// Execute a single node using the handler registry.
    async fn execute_node(
        &self,
        node: &PipelineNode,
        context: &PipelineContext,
        graph: &PipelineGraph,
        run_dir: &RunDirectory,
    ) -> Result<Outcome> {
        let handler = self.handler_registry.get(&node.handler_type);
        handler
            .execute(node, context, graph, run_dir)
            .await
            .context(format!("Handler '{}' failed for node '{}'", node.handler_type, node.id))
    }

    /// 5-step edge selection algorithm (spec Section 3.3).
    async fn select_next_edge(
        &self,
        graph: &PipelineGraph,
        current: NodeIndex,
        outcome: &Outcome,
        context: &PipelineContext,
    ) -> Result<Option<NodeIndex>> {
        let edges = graph.outgoing_edges(current);
        if edges.is_empty() {
            return Ok(None);
        }

        let current_id = &graph.graph[current].id;
        let ctx_snapshot = context.snapshot().await;

        // Step 1: Condition-matching edges
        let mut condition_matches: Vec<(NodeIndex, &str)> = Vec::new();
        for (target, edge) in &edges {
            if !edge.condition.is_empty() {
                let cond = parse_condition(&edge.condition);
                if evaluate_condition(&cond, outcome, &ctx_snapshot) {
                    condition_matches.push((*target, &edge.label));
                }
            }
        }
        if condition_matches.len() == 1 {
            let (target, label) = condition_matches[0];
            self.emit(PipelineEvent::edge_selected(
                current_id,
                &graph.graph[target].id,
                label,
                1,
            ))
            .await;
            return Ok(Some(target));
        }

        // Step 2: Preferred label match
        if let Some(ref preferred) = outcome.preferred_label {
            let normalized = normalize_label(preferred);
            for (target, edge) in &edges {
                if normalize_label(&edge.label) == normalized {
                    self.emit(PipelineEvent::edge_selected(
                        current_id,
                        &graph.graph[*target].id,
                        &edge.label,
                        2,
                    ))
                    .await;
                    return Ok(Some(*target));
                }
            }
        }

        // Step 3: Suggested next IDs from outcome
        for suggested in &outcome.suggested_next {
            if let Some(&target_idx) = graph.node_index.get(suggested) {
                // Verify edge exists
                for (target, edge) in &edges {
                    if *target == target_idx {
                        self.emit(PipelineEvent::edge_selected(
                            current_id,
                            suggested,
                            &edge.label,
                            3,
                        ))
                        .await;
                        return Ok(Some(target_idx));
                    }
                }
            }
        }

        // Step 4: Highest weight among unconditional edges
        let unconditional: Vec<_> = edges
            .iter()
            .filter(|(_, e)| e.condition.is_empty())
            .collect();

        if !unconditional.is_empty() {
            let max_weight = unconditional.iter().map(|(_, e)| e.weight).max().unwrap();
            let heaviest: Vec<_> = unconditional
                .iter()
                .filter(|(_, e)| e.weight == max_weight)
                .collect();

            if heaviest.len() == 1 {
                let (target, edge) = heaviest[0];
                self.emit(PipelineEvent::edge_selected(
                    current_id,
                    &graph.graph[*target].id,
                    &edge.label,
                    4,
                ))
                .await;
                return Ok(Some(*target));
            }

            // Step 5: Lexical tiebreak
            let mut candidates: Vec<_> = heaviest
                .iter()
                .map(|(target, edge)| {
                    let t = *target;
                    (t, graph.graph[t].id.clone(), edge.label.clone())
                })
                .collect();
            candidates.sort_by(|a, b| a.1.cmp(&b.1));

            let (target, ref id, ref label) = candidates[0];
            self.emit(PipelineEvent::edge_selected(current_id, id, label, 5))
                .await;
            return Ok(Some(target));
        }

        // Fallback: if all edges have conditions and none matched, no edge selected
        Ok(None)
    }

    /// Check goal gates on terminal nodes.
    async fn check_goal_gates(
        &self,
        _graph: &PipelineGraph,
        _outcome: &Outcome,
        context: &PipelineContext,
    ) -> bool {
        // Check if any node with goal_gate=true has not been completed
        // For MVP, check if context has a "goal_satisfied" key
        if let Some(val) = context.get("goal_satisfied").await {
            if let Some(b) = val.as_bool() {
                return b;
            }
            if let Some(s) = val.as_str() {
                return s == "true";
            }
        }
        // Default: satisfied (no explicit gate)
        true
    }

    async fn emit(&self, event: PipelineEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event).await;
        }
    }
}

/// Normalize a label for comparison: lowercase, trim, strip [K] accelerators.
fn normalize_label(label: &str) -> String {
    let s = label.trim().to_lowercase();
    // Strip [X] accelerator markers
    if s.starts_with('[') {
        if let Some(pos) = s.find(']') {
            return s[pos + 1..].trim().to_string();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_label() {
        assert_eq!(normalize_label("Success"), "success");
        assert_eq!(normalize_label("  Approve  "), "approve");
        assert_eq!(normalize_label("[A] Approve"), "approve");
        assert_eq!(normalize_label("[K] Keep Going"), "keep going");
    }
}
