//! `scud attractor export` — Export pipeline to DOT or SCG format.

use anyhow::{Context, Result};
use petgraph::visit::EdgeRef;
use std::path::Path;

use crate::attractor::dot_parser::parse_dot;
use crate::attractor::graph::PipelineGraph;
use crate::attractor::scg_bridge;
use crate::formats::{parse_scg_result, serialize_scg_pipeline};

/// Export a pipeline file to another format.
pub fn run(file: &Path, format: &str, output: Option<&Path>) -> Result<()> {
    let source =
        std::fs::read_to_string(file).context(format!("Failed to read: {}", file.display()))?;

    let is_scg = file.extension().and_then(|e| e.to_str()) == Some("scg");

    let result = match format {
        "dot" => {
            if is_scg {
                // SCG -> PipelineGraph -> DOT
                let scg = parse_scg_result(&source).context("Failed to parse SCG file")?;
                let pipeline = scg_bridge::pipeline_from_scg(&scg)
                    .context("Failed to build pipeline graph from SCG")?;
                pipeline_to_dot(&pipeline)
            } else {
                // Already DOT, just pass through
                source
            }
        }
        "scg" => {
            if is_scg {
                // Already SCG, pass through
                source
            } else {
                // DOT -> PipelineGraph -> SCG
                let dot = parse_dot(&source).context("Failed to parse DOT file")?;
                let pipeline =
                    PipelineGraph::from_dot(&dot).context("Failed to build pipeline graph")?;
                let scg_result = scg_bridge::scg_from_pipeline(&pipeline);
                serialize_scg_pipeline(&scg_result)
            }
        }
        _ => anyhow::bail!(
            "Unsupported export format '{}'. Use 'dot' or 'scg'.",
            format
        ),
    };

    if let Some(out_path) = output {
        std::fs::write(out_path, &result)
            .context(format!("Failed to write: {}", out_path.display()))?;
        eprintln!("Wrote {} to {}", format, out_path.display());
    } else {
        print!("{}", result);
    }

    Ok(())
}

/// Serialize a PipelineGraph to DOT format.
fn pipeline_to_dot(pipeline: &PipelineGraph) -> String {
    let mut out = String::new();
    out.push_str(&format!("digraph {} {{\n", pipeline.name));

    // Graph attrs
    if let Some(ref goal) = pipeline.graph_attrs.goal {
        out.push_str(&format!("    graph [goal={:?}]\n", goal));
    }

    // Node defaults
    out.push_str("    node [style=filled, fillcolor=white]\n\n");

    // Nodes
    for idx in pipeline.graph.node_indices() {
        let node = &pipeline.graph[idx];
        let mut attrs = vec![
            format!("shape={}", node.shape),
            format!("label={:?}", node.label),
        ];

        if !node.prompt.is_empty() {
            attrs.push(format!("prompt={:?}", node.prompt));
        }
        if node.max_retries > 0 {
            attrs.push(format!("max_retries={}", node.max_retries));
        }
        if node.goal_gate {
            attrs.push("goal_gate=true".into());
        }
        if let Some(ref rt) = node.retry_target {
            attrs.push(format!("retry_target={:?}", rt));
        }
        if let Some(ref timeout) = node.timeout {
            out.push_str(""); // satisfy the compiler
            attrs.push(format!("timeout=\"{}s\"", timeout.as_secs()));
        }

        out.push_str(&format!("    {} [{}]\n", node.id, attrs.join(", ")));
    }

    out.push('\n');

    // Edges
    for edge_ref in pipeline.graph.edge_references() {
        let from = &pipeline.graph[edge_ref.source()];
        let to = &pipeline.graph[edge_ref.target()];
        let edge = edge_ref.weight();

        let mut attrs = Vec::new();
        if !edge.label.is_empty() {
            attrs.push(format!("label={:?}", edge.label));
        }
        if !edge.condition.is_empty() {
            attrs.push(format!("condition={:?}", edge.condition));
        }
        if edge.weight != 0 {
            attrs.push(format!("weight={}", edge.weight));
        }

        if attrs.is_empty() {
            out.push_str(&format!("    {} -> {}\n", from.id, to.id));
        } else {
            out.push_str(&format!(
                "    {} -> {} [{}]\n",
                from.id,
                to.id,
                attrs.join(", ")
            ));
        }
    }

    out.push_str("}\n");
    out
}
