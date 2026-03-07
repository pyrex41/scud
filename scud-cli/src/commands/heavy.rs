//! CLI handler for `scud heavy` — multi-agent Heavy reasoning mode.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::backend::direct::DirectApiBackend;
use crate::heavy::{self, HeavyConfig, HeavyEvent};

/// Run the Heavy command from CLI args.
pub async fn run(
    query: String,
    provider: String,
    model: String,
    captain_model: Option<String>,
    max_agents: Option<usize>,
    debate_rounds: usize,
    verbose: bool,
    json: bool,
    query_file: Option<PathBuf>,
) -> Result<()> {
    let query = if let Some(f) = query_file {
        std::fs::read_to_string(&f)?
    } else {
        query
    };

    if query.trim().is_empty() {
        anyhow::bail!("Query is required. Provide a query string or use --query-file.");
    }

    let mut config = HeavyConfig {
        query,
        provider,
        model,
        captain_model,
        max_agents,
        debate_rounds,
        max_turns: 10,
        verbose,
        json_output: json,
    };

    // Apply any .scud/heavy.toml defaults
    let working_dir = std::env::current_dir()?;
    config.apply_toml_defaults(&working_dir)?;

    let backend = Arc::new(DirectApiBackend::new()) as Arc<dyn crate::backend::AgentBackend>;

    let (event_tx, event_rx) = mpsc::channel(1000);

    let config_json = config.json_output;
    let config_verbose = config.verbose;

    // Spawn display task
    let display_handle = tokio::spawn(display_events(event_rx, config_verbose, config_json));

    // Run the pipeline
    let start = Instant::now();
    let result = heavy::run_heavy(config, backend, event_tx).await;

    // Wait for display to finish
    let _ = display_handle.await;

    match result {
        Ok(result) => {
            if config_json {
                // JSON output already printed in display_events
            } else {
                let elapsed = start.elapsed();
                eprintln!(
                    "\n{} agents, {:.1}s total",
                    result.agents_activated.len(),
                    elapsed.as_secs_f64()
                );
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Display events as they stream in.
async fn display_events(
    mut rx: mpsc::Receiver<HeavyEvent>,
    verbose: bool,
    json_output: bool,
) {
    let mut agent_states: HashMap<String, AgentDisplayState> = HashMap::new();

    while let Some(event) = rx.recv().await {
        match event {
            HeavyEvent::RoutingStarted => {
                if !json_output {
                    eprintln!("\x1b[1m[Captain]\x1b[0m      Analyzing query... selecting specialists");
                }
            }
            HeavyEvent::RoutingComplete { agents } => {
                if !json_output {
                    let specialist_count = agents.iter().filter(|a| {
                        !["Captain", "Harper", "Benjamin", "Lucas"].contains(&a.as_str())
                    }).count();
                    let specialist_names: Vec<&str> = agents
                        .iter()
                        .filter(|a| !["Captain"].contains(&a.as_str()))
                        .map(|s| s.as_str())
                        .collect();
                    eprintln!(
                        "\x1b[1m[Captain]\x1b[0m      Activated: {} ({} of 16)\n",
                        specialist_names.join(", "),
                        agents.len()
                    );
                    let _ = specialist_count; // used in the count above
                }
            }
            HeavyEvent::AgentStarted { name, role } => {
                agent_states.insert(name.clone(), AgentDisplayState::default());
                if verbose && !json_output {
                    eprintln!("\x1b[2m[{}]\x1b[0m     Started ({})", name, role);
                }
            }
            HeavyEvent::AgentEvent { name, inner } => {
                if verbose && !json_output {
                    match &inner {
                        crate::backend::AgentEvent::ToolCallStart { name: tool_name, .. } => {
                            eprintln!("\x1b[2m[{}]\x1b[0m       {}...", name, tool_name);
                        }
                        crate::backend::AgentEvent::TextDelta(t) => {
                            if let Some(state) = agent_states.get_mut(&name) {
                                state.text_len += t.len();
                            }
                        }
                        _ => {}
                    }
                }
            }
            HeavyEvent::AgentCompleted { name, duration } => {
                if !json_output {
                    let is_core = ["Harper", "Benjamin", "Lucas"].contains(&name.as_str());
                    if is_core {
                        eprintln!(
                            "\x1b[1m[{}]\x1b[0m{}Done ({:.1}s)",
                            name,
                            padding(&name),
                            duration.as_secs_f64()
                        );
                    } else {
                        eprintln!(
                            "\x1b[2m[{}]\x1b[0m{}Done ({:.1}s)",
                            name,
                            padding(&name),
                            duration.as_secs_f64()
                        );
                    }
                }
            }
            HeavyEvent::SynthesisStarted => {
                if !json_output {
                    eprintln!("\nSynthesizing perspectives...");
                    eprintln!("{}", "━".repeat(40));
                }
            }
            HeavyEvent::SynthesisDelta(text) => {
                if !json_output {
                    print!("{}", text);
                }
            }
            HeavyEvent::DebateRound { round } => {
                if !json_output {
                    eprintln!("\n\x1b[1mDebate round {}\x1b[0m", round);
                }
            }
            HeavyEvent::Complete(result) => {
                if json_output {
                    if let Ok(json) = serde_json::to_string_pretty(&result) {
                        println!("{}", json);
                    }
                } else {
                    // Final answer was already streamed via SynthesisDelta
                    println!();
                }
            }
        }
    }
}

/// Generate padding to align output columns.
fn padding(name: &str) -> &'static str {
    match name.len() {
        ..=4 => "          ",
        5 => "         ",
        6 => "        ",
        7 => "       ",
        8 => "      ",
        9 => "     ",
        10 => "    ",
        _ => "   ",
    }
}

#[derive(Default)]
struct AgentDisplayState {
    text_len: usize,
}
