use anyhow::Result;
use std::path::Path;

/// Capture full tmux pane output for a task window
pub fn capture_agent_output(session_name: &str, task_id: &str) -> Result<String> {
    let window_name = format!("task-{}", task_id);

    let output = std::process::Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            &format!("{}:{}", session_name, window_name),
            "-p",
            "-S",
            "-", // From start of history
        ])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Save captured output to run directory
pub fn save_agent_output(run_dir: &Path, task_id: &str, output: &str) -> Result<()> {
    let logs_dir = run_dir.join("agent_logs");
    std::fs::create_dir_all(&logs_dir)?;
    std::fs::write(logs_dir.join(format!("{}.log", task_id)), output)?;
    Ok(())
}
