//! `scud attractor import` — Convert DOT pipeline to SCG format.

use anyhow::{Context, Result};
use std::path::Path;

use crate::attractor::dot_parser::parse_dot;
use crate::attractor::graph::PipelineGraph;
use crate::attractor::scg_bridge;
use crate::formats::serialize_scg_pipeline;

/// Import a DOT pipeline and convert to SCG format.
pub fn run(file: &Path, output: Option<&Path>) -> Result<()> {
    let source =
        std::fs::read_to_string(file).context(format!("Failed to read: {}", file.display()))?;

    let dot_graph = parse_dot(&source).context("Failed to parse DOT file")?;
    let pipeline = PipelineGraph::from_dot(&dot_graph).context("Failed to build pipeline graph")?;

    let scg_result = scg_bridge::scg_from_pipeline(&pipeline);
    let scg_text = serialize_scg_pipeline(&scg_result);

    if let Some(out_path) = output {
        std::fs::write(out_path, &scg_text)
            .context(format!("Failed to write: {}", out_path.display()))?;
        eprintln!("Wrote SCG to {}", out_path.display());
    } else {
        print!("{}", scg_text);
    }

    Ok(())
}
