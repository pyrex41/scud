//! Execute an agent loop using direct Anthropic API calls.
//!
//! This command is designed to be spawned in tmux windows by ralph/swarm,
//! replacing `claude -p` with a direct API agentic loop. Can also be used
//! standalone for testing.

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::commands::spawn::headless::events::StreamEventKind;
use crate::llm::agent;

/// Run the direct API agent loop, printing events to stdout/stderr.
pub async fn run(
    prompt: Option<String>,
    prompt_file: Option<PathBuf>,
    model: Option<String>,
) -> Result<()> {
    let prompt = if let Some(f) = prompt_file {
        std::fs::read_to_string(&f)?
    } else if let Some(p) = prompt {
        p
    } else {
        anyhow::bail!("Either --prompt or --prompt-file is required");
    };

    let working_dir = std::env::current_dir()?;
    let (tx, mut rx) = mpsc::channel(1000);

    let agent_handle = tokio::spawn({
        let model = model.clone();
        let working_dir = working_dir.clone();
        async move {
            agent::run_agent_loop(&prompt, None, &working_dir, model.as_deref(), 16_000, tx).await
        }
    });

    // Print events to stdout (visible in tmux)
    while let Some(event) = rx.recv().await {
        match &event.kind {
            StreamEventKind::TextDelta { text } => print!("{}", text),
            StreamEventKind::ToolStart {
                tool_name,
                input_summary,
                ..
            } => {
                eprintln!("\n[{}] {}", tool_name, input_summary);
            }
            StreamEventKind::ToolResult {
                tool_name, success, ..
            } => {
                eprintln!(
                    "[{}] {}",
                    tool_name,
                    if *success { "ok" } else { "FAILED" }
                );
            }
            StreamEventKind::Error { message } => eprintln!("\nERROR: {}", message),
            StreamEventKind::Complete { success } => {
                if !*success {
                    eprintln!("\nAgent completed with errors");
                }
                break;
            }
            _ => {}
        }
    }

    agent_handle.await??;
    Ok(())
}
