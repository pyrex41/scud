//! Bridge between SCG pipeline format and PipelineGraph.
//!
//! Converts SCG parse results into the same `PipelineGraph` the runner consumes,
//! and converts `PipelineGraph` back into SCG for DOT import round-trips.

use anyhow::{Context, Result};
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::time::Duration;

use super::dot_parser::AttrValue;
use super::graph::{GraphAttrs, PipelineEdge, PipelineGraph, PipelineNode};
use crate::formats::{
    natural_sort_ids, PipelineNodeAttrs, ScgEdgeAttrs, ScgParseResult, ScgPipeline,
};
use crate::models::{Phase, Priority, Task, TaskStatus};

/// Build a PipelineGraph from a parsed SCG pipeline result.
pub fn pipeline_from_scg(result: &ScgParseResult) -> Result<PipelineGraph> {
    let pipeline = result
        .pipeline
        .as_ref()
        .context("SCG file is not in pipeline mode (missing 'mode pipeline' in @meta)")?;

    let phase = &result.phase;
    let mut graph = DiGraph::new();
    let mut node_index = HashMap::new();

    // Build nodes from tasks + pipeline attrs
    for task in &phase.tasks {
        let attrs = pipeline.node_attrs.get(&task.id);
        let handler_type = attrs
            .map(|a| a.handler_type.clone())
            .unwrap_or_else(|| "codergen".into());

        // In pipeline SCG, task.description stores the prompt and task.details stores tool_command
        let node = PipelineNode {
            id: task.id.clone(),
            label: task.title.clone(),
            shape: shape_from_handler(&handler_type),
            handler_type,
            prompt: task.description.clone(),
            max_retries: attrs.map(|a| a.max_retries).unwrap_or(0),
            goal_gate: attrs.map(|a| a.goal_gate).unwrap_or(false),
            retry_target: attrs.and_then(|a| a.retry_target.clone()),
            fallback_retry_target: None,
            fidelity: None,
            thread_id: None,
            classes: vec![],
            timeout: attrs.and_then(|a| a.timeout.as_ref().and_then(|t| parse_duration(t))),
            llm_model: None,
            llm_provider: None,
            reasoning_effort: "high".into(),
            auto_status: true,
            allow_partial: false,
            extra_attrs: build_extra_attrs(task),
        };

        let idx = graph.add_node(node);
        node_index.insert(task.id.clone(), idx);
    }

    // Build edges
    for edge_attr in &pipeline.edge_attrs {
        let from_idx = *node_index
            .get(&edge_attr.from)
            .context(format!("Edge source '{}' not found", edge_attr.from))?;
        let to_idx = *node_index
            .get(&edge_attr.to)
            .context(format!("Edge target '{}' not found", edge_attr.to))?;

        let edge = PipelineEdge {
            label: edge_attr.label.clone(),
            condition: edge_attr.condition.clone(),
            weight: edge_attr.weight,
            fidelity: None,
            thread_id: None,
            loop_restart: false,
        };

        graph.add_edge(from_idx, to_idx, edge);
    }

    // Find start and exit nodes by handler_type
    let start_node = node_index
        .values()
        .copied()
        .find(|idx| graph[*idx].handler_type == "start")
        .context("No start node found (need a node with handler_type 'start' in @pipeline)")?;

    let exit_node = node_index
        .values()
        .copied()
        .find(|idx| graph[*idx].handler_type == "exit")
        .context("No exit node found (need a node with handler_type 'exit' in @pipeline)")?;

    // Build graph attrs
    let graph_attrs = GraphAttrs {
        goal: pipeline.goal.clone(),
        fidelity: None,
        model_stylesheet: pipeline.model_stylesheet.clone(),
        extra: HashMap::new(),
    };

    Ok(PipelineGraph {
        name: phase.name.clone(),
        graph_attrs,
        graph,
        node_index,
        start_node,
        exit_node,
    })
}

/// Convert a PipelineGraph back into an ScgParseResult.
pub fn scg_from_pipeline(graph: &PipelineGraph) -> ScgParseResult {
    let mut tasks = Vec::new();
    let mut node_attrs = HashMap::new();
    let mut edge_attrs = Vec::new();

    // Convert nodes to tasks + pipeline attrs
    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];

        let mut task = Task::new(node.id.clone(), node.label.clone(), node.prompt.clone());
        task.status = TaskStatus::Pending;
        task.complexity = 0;
        task.priority = Priority::Medium;

        // Store tool_command in details if present in extra_attrs
        if let Some(AttrValue::Str(cmd)) = node.extra_attrs.get("tool_command") {
            task.details = Some(cmd.clone());
        }

        tasks.push(task);

        let pipeline_attr = PipelineNodeAttrs {
            handler_type: node.handler_type.clone(),
            max_retries: node.max_retries,
            retry_target: node.retry_target.clone(),
            goal_gate: node.goal_gate,
            timeout: node.timeout.map(|d| format_duration(d)),
        };
        node_attrs.insert(node.id.clone(), pipeline_attr);
    }

    // Convert edges
    for edge_ref in graph.graph.edge_references() {
        let from = &graph.graph[edge_ref.source()];
        let to = &graph.graph[edge_ref.target()];
        let edge = edge_ref.weight();

        edge_attrs.push(ScgEdgeAttrs {
            from: from.id.clone(),
            to: to.id.clone(),
            label: edge.label.clone(),
            condition: edge.condition.clone(),
            weight: edge.weight,
        });
    }

    // Sort tasks for stable output
    tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

    let mut phase = Phase::new(graph.name.clone());
    phase.tasks = tasks;

    let pipeline = ScgPipeline {
        goal: graph.graph_attrs.goal.clone(),
        model_stylesheet: graph.graph_attrs.model_stylesheet.clone(),
        node_attrs,
        edge_attrs,
    };

    ScgParseResult {
        phase,
        pipeline: Some(pipeline),
    }
}

/// Map handler type back to a DOT shape for display purposes.
fn shape_from_handler(handler: &str) -> String {
    match handler {
        "start" => "Mdiamond",
        "exit" => "Msquare",
        "codergen" => "box",
        "wait.human" => "hexagon",
        "conditional" => "diamond",
        "parallel" => "component",
        "parallel.fan_in" => "tripleoctagon",
        "tool" => "parallelogram",
        "stack.manager_loop" => "house",
        _ => "box",
    }
    .into()
}

/// Parse a duration string like "30s", "5m", "1h".
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.ends_with('s') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(Duration::from_secs(n))
    } else if s.ends_with('m') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(Duration::from_secs(n * 60))
    } else if s.ends_with('h') {
        let n: u64 = s[..s.len() - 1].parse().ok()?;
        Some(Duration::from_secs(n * 3600))
    } else {
        None
    }
}

/// Format a Duration as a human-readable string.
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs % 3600 == 0 && secs > 0 {
        format!("{}h", secs / 3600)
    } else if secs % 60 == 0 && secs > 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

/// Build extra_attrs from task details fields beyond the standard ones.
fn build_extra_attrs(task: &Task) -> HashMap<String, AttrValue> {
    let mut attrs = HashMap::new();
    // Store tool_command from task.details if present
    if let Some(ref details) = task.details {
        if !details.is_empty() {
            attrs.insert(
                "tool_command".to_string(),
                AttrValue::Str(details.clone()),
            );
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{parse_scg_result, serialize_scg_pipeline};

    fn sample_pipeline_scg() -> &'static str {
        r#"# SCUD Graph v1
# Phase: build-api

@meta {
  name build-api
  mode pipeline
  goal Build a REST API
  model_stylesheet * { model: "claude-3-haiku"; reasoning_effort: "medium" }
}

@nodes
# id | title | status | complexity | priority
start | Start | P | 0 | M
design | Design API Schema | P | 5 | H
review | Approve Design | P | 0 | M
implement | Write Code | P | 8 | H
test | Run Tests | P | 3 | M
finish | Done | P | 0 | M

@edges
# from -> to [| label | condition | weight]
start -> design
design -> review
review -> implement | Approve | | 10
review -> design | Revise | | 0
implement -> test
test -> finish | | outcome=success
test -> implement | | outcome=failure

@pipeline
# id | handler_type | max_retries | retry_target | goal_gate | timeout
start | start
design | codergen | 3
review | wait.human
implement | codergen | 2 | | false | 5m
test | tool
finish | exit | 0 | design | true

@details
design | description |
  Create a detailed API schema for: $goal
implement | description |
  Implement the API based on $context.design.response
test | details |
  cargo build && cargo test 2>&1
"#
    }

    #[test]
    fn test_parse_pipeline_scg() {
        let result = parse_scg_result(sample_pipeline_scg()).unwrap();
        assert!(result.pipeline.is_some());

        let pipeline = result.pipeline.as_ref().unwrap();
        assert_eq!(pipeline.goal, Some("Build a REST API".into()));
        assert!(pipeline.model_stylesheet.is_some());

        // Check nodes parsed
        assert_eq!(result.phase.tasks.len(), 6);

        // Check pipeline attrs
        assert_eq!(pipeline.node_attrs.len(), 6);
        let design_attrs = pipeline.node_attrs.get("design").unwrap();
        assert_eq!(design_attrs.handler_type, "codergen");
        assert_eq!(design_attrs.max_retries, 3);

        let impl_attrs = pipeline.node_attrs.get("implement").unwrap();
        assert_eq!(impl_attrs.timeout, Some("5m".into()));
        assert_eq!(impl_attrs.max_retries, 2);

        let finish_attrs = pipeline.node_attrs.get("finish").unwrap();
        assert!(finish_attrs.goal_gate);
        assert_eq!(finish_attrs.retry_target, Some("design".into()));

        // Check edges
        assert_eq!(pipeline.edge_attrs.len(), 7);
        let approve_edge = pipeline
            .edge_attrs
            .iter()
            .find(|e| e.from == "review" && e.to == "implement")
            .unwrap();
        assert_eq!(approve_edge.label, "Approve");
        assert_eq!(approve_edge.weight, 10);
    }

    #[test]
    fn test_pipeline_scg_to_graph() {
        let result = parse_scg_result(sample_pipeline_scg()).unwrap();
        let graph = pipeline_from_scg(&result).unwrap();

        assert_eq!(graph.name, "build-api");
        assert_eq!(graph.graph_attrs.goal, Some("Build a REST API".into()));
        assert_eq!(graph.graph.node_count(), 6);
        assert_eq!(graph.graph.edge_count(), 7);

        // Start and exit found
        assert_eq!(graph.graph[graph.start_node].handler_type, "start");
        assert_eq!(graph.graph[graph.exit_node].handler_type, "exit");

        // Check specific node attrs
        let design = graph.node("design").unwrap();
        assert_eq!(design.handler_type, "codergen");
        assert_eq!(design.max_retries, 3);

        let implement = graph.node("implement").unwrap();
        assert_eq!(implement.timeout, Some(Duration::from_secs(300)));

        let finish = graph.node("finish").unwrap();
        assert!(finish.goal_gate);
        assert_eq!(finish.retry_target, Some("design".into()));
    }

    #[test]
    fn test_pipeline_round_trip() {
        let result = parse_scg_result(sample_pipeline_scg()).unwrap();
        let serialized = serialize_scg_pipeline(&result);
        let result2 = parse_scg_result(&serialized).unwrap();

        assert!(result2.pipeline.is_some());
        let p1 = result.pipeline.as_ref().unwrap();
        let p2 = result2.pipeline.as_ref().unwrap();

        assert_eq!(p1.goal, p2.goal);
        assert_eq!(p1.node_attrs.len(), p2.node_attrs.len());
        assert_eq!(p1.edge_attrs.len(), p2.edge_attrs.len());

        // Verify specific fields survive round-trip
        let d1 = p1.node_attrs.get("design").unwrap();
        let d2 = p2.node_attrs.get("design").unwrap();
        assert_eq!(d1.handler_type, d2.handler_type);
        assert_eq!(d1.max_retries, d2.max_retries);

        // Verify edge attrs
        for (e1, e2) in p1.edge_attrs.iter().zip(p2.edge_attrs.iter()) {
            assert_eq!(e1.from, e2.from);
            assert_eq!(e1.to, e2.to);
            assert_eq!(e1.label, e2.label);
            assert_eq!(e1.condition, e2.condition);
            assert_eq!(e1.weight, e2.weight);
        }
    }

    #[test]
    fn test_graph_to_scg_round_trip() {
        let result = parse_scg_result(sample_pipeline_scg()).unwrap();
        let graph = pipeline_from_scg(&result).unwrap();
        let result2 = scg_from_pipeline(&graph);

        assert!(result2.pipeline.is_some());
        let p2 = result2.pipeline.as_ref().unwrap();
        assert_eq!(p2.goal, Some("Build a REST API".into()));
        assert_eq!(p2.node_attrs.len(), 6);

        let start = p2.node_attrs.get("start").unwrap();
        assert_eq!(start.handler_type, "start");
    }

    #[test]
    fn test_backwards_compat_non_pipeline() {
        // Standard (non-pipeline) SCG should parse with pipeline=None
        let content = r#"# SCUD Graph v1
# Phase: normal

@meta {
  name normal
  updated 2025-01-01T00:00:00Z
}

@nodes
# id | title | status | complexity | priority
1 | Task One | P | 3 | H
2 | Task Two | P | 5 | M

@edges
2 -> 1
"#;
        let result = parse_scg_result(content).unwrap();
        assert!(result.pipeline.is_none());
        assert_eq!(result.phase.tasks.len(), 2);

        // Dependencies should work normally
        let t2 = result.phase.get_task("2").unwrap();
        assert_eq!(t2.dependencies, vec!["1".to_string()]);
    }

    #[test]
    fn test_parse_duration_strings() {
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_format_duration_strings() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(300)), "5m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(90)), "90s");
    }
}
