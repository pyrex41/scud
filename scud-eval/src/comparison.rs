use crate::metrics::EvalRunMetrics;
use crate::storage;

pub struct ComparisonReport {
    pub runs: Vec<EvalRunMetrics>,
}

/// Compare multiple evaluation runs
pub fn compare_runs(run_ids: &[String]) -> anyhow::Result<ComparisonReport> {
    let mut runs = Vec::new();
    for id in run_ids {
        match storage::load_run(id) {
            Ok(metrics) => runs.push(metrics),
            Err(_) => {
                eprintln!("Warning: Run '{}' not found or has no metrics", id);
            }
        }
    }
    Ok(ComparisonReport { runs })
}

/// Format comparison as ASCII table
pub fn format_comparison_table(report: &ComparisonReport) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{:<40} {:>12} {:>8} {:>10} {:>12}\n",
        "Run ID", "Duration", "Success", "Tokens", "Cost"
    ));
    output.push_str(&"-".repeat(86));
    output.push('\n');

    // Rows
    for run in &report.runs {
        let success_rate = if run.tasks_succeeded + run.tasks_failed > 0 {
            format!("{}/{}", run.tasks_succeeded, run.tasks_succeeded + run.tasks_failed)
        } else {
            "N/A".to_string()
        };

        let total_tokens = run
            .total_tokens_input
            .unwrap_or(0)
            .saturating_add(run.total_tokens_output.unwrap_or(0));
        let cost = run
            .total_estimated_cost_usd
            .map(|c| format!("${:.2}", c))
            .unwrap_or_else(|| "-".to_string());

        let duration = run
            .total_duration_secs
            .map(|d| format!("{:.0}", d))
            .unwrap_or_else(|| "-".to_string());

        output.push_str(&format!(
            "{:<40} {:>10}s {:>8} {:>10} {:>12}\n",
            truncate(&run.run_id, 38),
            duration,
            success_rate,
            total_tokens,
            cost
        ));
    }

    output
}

/// Generate detailed markdown report for a single run
pub fn generate_report(metrics: &EvalRunMetrics) -> String {
    let mut report = String::new();

    report.push_str(&format!("# Evaluation Report: {}\n\n", metrics.run_id));
    report.push_str(&format!("**Mode:** {}\n", metrics.mode.name()));
    report.push_str(&format!("**Taskset:** {}\n", metrics.taskset_name));
    if let Some(duration) = metrics.total_duration_secs {
        report.push_str(&format!("**Duration:** {:.1}s\n", duration));
    }
    report.push_str(&format!(
        "**Success Rate:** {}/{}\n\n",
        metrics.tasks_succeeded,
        metrics.tasks_succeeded + metrics.tasks_failed
    ));

    report.push_str("## Task Breakdown\n\n");
    report.push_str("| Task | Duration | Success | Lines | Tokens |\n");
    report.push_str("|------|----------|---------|-------|--------|\n");

    for task in &metrics.task_metrics {
        let lines = format!(
            "+{} -{}",
            task.lines_added.unwrap_or(0),
            task.lines_removed.unwrap_or(0)
        );
        let duration = task
            .duration_secs
            .map(|d| format!("{:.0}", d))
            .unwrap_or_else(|| "-".to_string());
        let tokens = task
            .tokens_input
            .unwrap_or(0)
            .saturating_add(task.tokens_output.unwrap_or(0));
        report.push_str(&format!(
            "| {} | {}s | {} | {} | {} |\n",
            task.task_id,
            duration,
            if task.success { "Y" } else { "N" },
            lines,
            tokens
        ));
    }

    report.push_str("\n## Cost Summary\n\n");
    if let Some(input) = metrics.total_tokens_input {
        report.push_str(&format!("- Input tokens: {}\n", input));
    }
    if let Some(output) = metrics.total_tokens_output {
        report.push_str(&format!("- Output tokens: {}\n", output));
    }
    if let Some(cost) = metrics.total_estimated_cost_usd {
        report.push_str(&format!("- Estimated cost: ${:.2}\n", cost));
    }

    report
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
