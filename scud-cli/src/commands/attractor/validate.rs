//! `scud attractor validate` — Validate an Attractor pipeline without executing it.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

use crate::attractor::dot_parser::parse_dot;
use crate::attractor::graph::PipelineGraph;
use crate::attractor::transforms::apply_transforms;
use crate::attractor::validator::{self, Severity};

/// Validate a pipeline DOT file.
pub fn run(file: &Path) -> Result<()> {
    let dot_source = std::fs::read_to_string(file)
        .context(format!("Failed to read: {}", file.display()))?;
    let dot_graph = parse_dot(&dot_source).context("Failed to parse DOT file")?;
    let mut pipeline = PipelineGraph::from_dot(&dot_graph).context("Failed to build pipeline graph")?;

    apply_transforms(&mut pipeline);

    let issues = validator::validate(&pipeline);

    let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
    let warnings: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Warning).collect();

    println!(
        "{}: {} ({} nodes, {} edges)",
        "Pipeline".bold(),
        pipeline.name.cyan(),
        pipeline.graph.node_count(),
        pipeline.graph.edge_count()
    );
    println!();

    // Print nodes
    println!("{}", "Nodes:".bold());
    for idx in pipeline.graph.node_indices() {
        let node = &pipeline.graph[idx];
        println!(
            "  {} ({}) {}",
            node.id,
            node.handler_type.dimmed(),
            if node.prompt.is_empty() {
                String::new()
            } else {
                format!("- {}", truncate(&node.prompt, 60))
            }
        );
    }
    println!();

    if !warnings.is_empty() {
        println!("{} ({}):", "Warnings".yellow().bold(), warnings.len());
        for issue in &warnings {
            println!(
                "  {} [{}] {}",
                "WARN".yellow(),
                issue.rule,
                issue.message
            );
        }
        println!();
    }

    if !errors.is_empty() {
        println!("{} ({}):", "Errors".red().bold(), errors.len());
        for issue in &errors {
            println!(
                "  {} [{}] {}",
                "ERROR".red(),
                issue.rule,
                issue.message
            );
        }
        println!();
        anyhow::bail!("Validation failed with {} error(s)", errors.len());
    }

    println!("{}", "✓ Pipeline is valid".green().bold());
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max - 3])
    } else {
        s.to_string()
    }
}
