use anyhow::Result;
use clap::{Parser, Subcommand};
use scud_eval::metrics::ExecutionMode;
use scud_eval::{comparison, config, runner, storage, tasksets};

#[derive(Parser)]
#[command(name = "scud-eval")]
#[command(about = "Evaluate SCUD execution modes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an evaluation
    Run {
        /// Task set name (eval-trivial, eval-moderate, eval-complex, eval-real-scud)
        #[arg(short, long)]
        taskset: String,
        /// Execution mode (swarm-N, ralph, claude-direct)
        #[arg(short, long)]
        mode: String,
    },
    /// List available task sets or previous runs
    List {
        #[command(subcommand)]
        what: ListTarget,
    },
    /// Compare multiple eval runs
    Compare {
        /// Run IDs to compare
        run_ids: Vec<String>,
    },
    /// Show detailed report for a run
    Report {
        /// Run ID
        run_id: String,
    },
}

#[derive(Subcommand)]
enum ListTarget {
    /// List available task sets
    Tasksets,
    /// List previous runs
    Runs,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { taskset, mode } => cmd_run(&taskset, &mode),
        Commands::List { what } => match what {
            ListTarget::Tasksets => cmd_list_tasksets(),
            ListTarget::Runs => cmd_list_runs(),
        },
        Commands::Compare { run_ids } => cmd_compare(&run_ids),
        Commands::Report { run_id } => cmd_report(&run_id),
    }
}

fn cmd_run(taskset_name: &str, mode_str: &str) -> Result<()> {
    // Parse the execution mode
    let config = config::parse_mode_config(mode_str, taskset_name, "claude-code", None)?;

    // Find the taskset
    let tasksets = tasksets::builtin_tasksets();
    let taskset = tasksets
        .iter()
        .find(|ts| ts.name == taskset_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown taskset: {}", taskset_name))?;

    // Generate run ID
    let run_id = format!(
        "{}-{}-{}",
        taskset_name,
        config.mode.name(),
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );

    // Setup workspace
    let workspace = runner::setup_eval_workspace(taskset, &run_id)?;

    println!("Eval run: {}", run_id);
    println!("Mode: {}", config.mode.name());
    println!("Taskset: {} - {}", taskset.name, taskset.description);
    println!("Workspace: {}", workspace.path.display());
    println!();

    // Execute based on mode
    match config.mode {
        ExecutionMode::Swarm { round_size } => {
            runner::run_swarm(&workspace, round_size)?;
        }
        ExecutionMode::Ralph => {
            runner::run_ralph(&workspace)?;
        }
        ExecutionMode::ClaudeDirect => {
            runner::run_claude_direct(&workspace)?;
        }
    }

    Ok(())
}

fn cmd_list_tasksets() -> Result<()> {
    let tasksets = tasksets::builtin_tasksets();

    println!("Available task sets:");
    println!();

    for ts in tasksets {
        let type_label = match ts.task_type {
            tasksets::TaskSetType::Synthetic => "synthetic",
            tasksets::TaskSetType::Real => "real",
        };
        println!(
            "  {} ({}) - {}",
            ts.name, type_label, ts.description
        );
    }

    Ok(())
}

fn cmd_list_runs() -> Result<()> {
    let runs = storage::list_runs()?;

    if runs.is_empty() {
        println!("No evaluation runs found.");
        println!("Run directory: {}", storage::runs_dir().display());
        return Ok(());
    }

    println!("Previous evaluation runs:");
    println!();

    for run_id in runs {
        // Try to load the run to get more info
        match storage::load_run(&run_id) {
            Ok(metrics) => {
                let status = if metrics.completed_at.is_some() {
                    "completed"
                } else {
                    "in-progress"
                };
                println!(
                    "  {} - {} on {} [{}]",
                    run_id,
                    metrics.mode.name(),
                    metrics.taskset_name,
                    status
                );
            }
            Err(_) => {
                // Metrics file might not exist yet (run in progress or failed)
                println!("  {} - (no metrics available)", run_id);
            }
        }
    }

    Ok(())
}

fn cmd_compare(run_ids: &[String]) -> Result<()> {
    if run_ids.is_empty() {
        println!("No run IDs provided for comparison.");
        println!("Usage: scud-eval compare <run_id1> <run_id2> ...");
        return Ok(());
    }

    let report = comparison::compare_runs(run_ids)?;

    if report.runs.is_empty() {
        println!("No valid runs found for comparison.");
        return Ok(());
    }

    print!("{}", comparison::format_comparison_table(&report));

    Ok(())
}

fn cmd_report(run_id: &str) -> Result<()> {
    let metrics = storage::load_run(run_id)?;
    print!("{}", comparison::generate_report(&metrics));
    Ok(())
}
