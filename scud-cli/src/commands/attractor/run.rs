//! `scud attractor run` — Execute an Attractor pipeline.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::attractor::checkpoint::Checkpoint;
use crate::attractor::context::Context as PipelineContext;
use crate::attractor::dot_parser::parse_dot;
use crate::attractor::graph::PipelineGraph;
use crate::attractor::handlers::HandlerRegistry;
use crate::attractor::interviewer::{AutoApproveInterviewer, ConsoleInterviewer};
use crate::attractor::outcome::StageStatus;
use crate::attractor::run_directory::{RunDirectory, RunManifest};
use crate::attractor::runner::PipelineRunner;
use crate::attractor::scg_bridge;
use crate::attractor::transforms::apply_transforms;
use crate::attractor::validator;
use crate::backend;
use crate::formats::parse_scg_result;

/// Run an Attractor pipeline.
pub async fn run(
    file: &Path,
    resume: Option<&Path>,
    headless: bool,
    simulated: bool,
    model: Option<&str>,
    provider: Option<&str>,
    runs_dir: Option<&Path>,
) -> Result<()> {
    // Read and parse the pipeline file (.scg or .dot)
    let source = std::fs::read_to_string(file)
        .context(format!("Failed to read pipeline file: {}", file.display()))?;

    let is_scg = file.extension().and_then(|e| e.to_str()) == Some("scg");
    let mut pipeline = if is_scg {
        let result = parse_scg_result(&source).context("Failed to parse SCG file")?;
        scg_bridge::pipeline_from_scg(&result).context("Failed to build pipeline graph from SCG")?
    } else {
        let dot_graph = parse_dot(&source).context("Failed to parse DOT file")?;
        PipelineGraph::from_dot(&dot_graph).context("Failed to build pipeline graph")?
    };

    // Apply transforms
    apply_transforms(&mut pipeline);

    // Apply CLI-level model override to nodes without explicit model
    if let Some(m) = model {
        for idx in pipeline.graph.node_indices() {
            if pipeline.graph[idx].llm_model.is_none() {
                pipeline.graph[idx].llm_model = Some(m.to_string());
            }
        }
    }

    // Validate
    let issues = validator::validate(&pipeline);
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == validator::Severity::Error)
        .collect();
    if !errors.is_empty() {
        eprintln!("{}", "Validation errors:".red().bold());
        for issue in &errors {
            eprintln!("  {} {}: {}", "ERROR".red(), issue.rule, issue.message);
        }
        anyhow::bail!("Pipeline has {} validation error(s)", errors.len());
    }

    // Print warnings
    for issue in &issues {
        if issue.severity == validator::Severity::Warning {
            eprintln!("  {} {}: {}", "WARN".yellow(), issue.rule, issue.message);
        }
    }

    // Set up run directory
    let base_dir = runs_dir.map(PathBuf::from).unwrap_or_else(|| {
        file.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });

    let run_id = format!(
        "{}-{}",
        pipeline.name,
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let run_dir = RunDirectory::create(&base_dir, &run_id)?;

    // Write manifest
    run_dir.write_manifest(&RunManifest {
        run_id: run_id.clone(),
        pipeline_name: pipeline.name.clone(),
        pipeline_file: file.display().to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        status: "running".into(),
    })?;

    // Load checkpoint for resume
    let checkpoint = if let Some(resume_path) = resume {
        Some(Checkpoint::load(resume_path).context("Failed to load checkpoint for resume")?)
    } else {
        None
    };

    // Create context early so we can store pipeline-level config
    let context = PipelineContext::new();

    // Create backend
    let agent_backend: std::sync::Arc<dyn backend::AgentBackend> = if simulated {
        std::sync::Arc::new(crate::backend::simulated::SimulatedBackend)
    } else {
        let harness = if let Some(p) = provider {
            crate::commands::spawn::terminal::Harness::parse(p)?
        } else {
            // Fall back to config's swarm.harness, then "rho"
            let config_path = file
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(".scud")
                .join("config.toml");
            let harness_name = crate::config::Config::load(&config_path)
                .map(|c| c.swarm.harness.clone())
                .unwrap_or_else(|_| "rho".to_string());
            crate::commands::spawn::terminal::Harness::parse(&harness_name)?
        };
        std::sync::Arc::from(backend::create_backend(&harness)?)
    };

    // Create handler registry
    let registry = HandlerRegistry::with_backend(agent_backend);

    // Create interviewer
    let interviewer: Box<dyn crate::attractor::interviewer::Interviewer> = if headless {
        Box::new(AutoApproveInterviewer)
    } else {
        Box::new(ConsoleInterviewer)
    };

    // Create and run the pipeline
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1000);

    let runner = PipelineRunner::new(registry, interviewer).with_events(event_tx);

    // Spawn event printer
    let print_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                crate::attractor::events::PipelineEvent::NodeStarted {
                    node_id,
                    handler_type,
                } => {
                    eprintln!(
                        "  {} {} ({})",
                        "→".blue(),
                        node_id.bold(),
                        handler_type.dimmed()
                    );
                }
                crate::attractor::events::PipelineEvent::NodeCompleted {
                    node_id,
                    status,
                    duration_ms,
                } => {
                    let status_str = match status {
                        StageStatus::Success => "✓".green().to_string(),
                        StageStatus::Failure => "✗".red().to_string(),
                        StageStatus::Skipped => "⊘".dimmed().to_string(),
                        _ => "?".yellow().to_string(),
                    };
                    eprintln!("  {} {} ({}ms)", status_str, node_id, duration_ms);
                }
                crate::attractor::events::PipelineEvent::EdgeSelected {
                    from_node,
                    to_node,
                    edge_label,
                    ..
                } => {
                    if !edge_label.is_empty() {
                        eprintln!(
                            "    {} {} → {} [{}]",
                            "⤷".dimmed(),
                            from_node.dimmed(),
                            to_node,
                            edge_label
                        );
                    }
                }
                crate::attractor::events::PipelineEvent::PipelineCompleted {
                    status,
                    total_duration_ms,
                    nodes_executed,
                } => {
                    let status_str = match status {
                        StageStatus::Success => "COMPLETED".green().bold().to_string(),
                        StageStatus::Failure => "FAILED".red().bold().to_string(),
                        _ => "FINISHED".yellow().bold().to_string(),
                    };
                    eprintln!();
                    eprintln!(
                        "Pipeline {} ({} nodes, {:.1}s)",
                        status_str,
                        nodes_executed,
                        total_duration_ms as f64 / 1000.0
                    );
                }
                crate::attractor::events::PipelineEvent::Error { message, .. } => {
                    eprintln!("  {} {}", "ERROR".red(), message);
                }
                _ => {}
            }
        }
    });

    eprintln!(
        "{} {} (run: {})",
        "Running pipeline".bold(),
        pipeline.name.cyan(),
        run_id.dimmed()
    );
    eprintln!();

    let status = runner
        .run(&pipeline, &context, &run_dir, checkpoint)
        .await?;

    // Wait for event printer to finish
    drop(runner);
    let _ = print_handle.await;

    match status {
        StageStatus::Success => {
            eprintln!("\nRun directory: {}", run_dir.root().display());
            Ok(())
        }
        _ => {
            eprintln!("\nRun directory: {}", run_dir.root().display());
            anyhow::bail!("Pipeline did not complete successfully")
        }
    }
}
