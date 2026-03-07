//! Heavy Mode — 16-agent multi-agent reasoning ensemble.
//!
//! Implements a Grok Heavy-style architecture where a Captain coordinator
//! routes queries to domain-specialized agents, collects their parallel
//! outputs, and synthesizes a unified answer.
//!
//! # Architecture
//!
//! 1. **Routing**: Captain analyzes the query and selects relevant specialists
//! 2. **Parallel Execution**: Selected agents run concurrently via `AgentBackend`
//! 3. **Synthesis**: Captain combines all agent outputs into a final answer
//! 4. **Debate** (optional): Agents critique the synthesis, Captain re-synthesizes

pub mod agents;
pub mod config;

pub use config::HeavyConfig;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::backend::{AgentBackend, AgentEvent, AgentRequest, AgentResult, TokenUsage};

/// Events emitted during Heavy execution for live display.
#[derive(Debug, Clone)]
pub enum HeavyEvent {
    RoutingStarted,
    RoutingComplete { agents: Vec<String> },
    AgentStarted { name: String, role: String },
    AgentEvent { name: String, inner: AgentEvent },
    AgentCompleted { name: String, duration: Duration },
    SynthesisStarted,
    SynthesisDelta(String),
    DebateRound { round: usize },
    Complete(HeavyResult),
}

/// Final result of a Heavy execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeavyResult {
    pub final_text: String,
    pub agents_activated: Vec<String>,
    pub agent_outputs: HashMap<String, AgentResult>,
    pub debate_critiques: Vec<HashMap<String, String>>,
    pub total_usage: Option<TokenUsage>,
}

/// Run the full Heavy pipeline.
pub async fn run_heavy(
    config: HeavyConfig,
    backend: Arc<dyn AgentBackend>,
    event_tx: mpsc::Sender<HeavyEvent>,
) -> Result<HeavyResult> {
    let working_dir = std::env::current_dir()?;

    // ── Phase 1: Routing ────────────────────────────────────────────
    let _ = event_tx.send(HeavyEvent::RoutingStarted).await;

    let selected_agents = route_query(&config, &backend, &working_dir).await?;

    let agent_names: Vec<String> = selected_agents.iter().map(|r| r.name.to_string()).collect();
    let _ = event_tx
        .send(HeavyEvent::RoutingComplete {
            agents: agent_names.clone(),
        })
        .await;

    // ── Phase 2: Parallel Execution ─────────────────────────────────
    let mut agent_outputs: HashMap<String, AgentResult> = HashMap::new();
    let mut join_set = tokio::task::JoinSet::new();

    for role in &selected_agents {
        // Skip Captain in execution phase (Captain only routes and synthesizes)
        if role.name == "Captain" {
            continue;
        }

        let _ = event_tx
            .send(HeavyEvent::AgentStarted {
                name: role.name.to_string(),
                role: role.domain.to_string(),
            })
            .await;

        let backend = Arc::clone(&backend);
        let query = config.query.clone();
        let name = role.name.to_string();
        let system_prompt = role.system_prompt.to_string();
        let model = config.model.clone();
        let provider = config.provider.clone();
        let max_turns = config.max_turns;
        let working_dir = working_dir.clone();
        let event_tx = event_tx.clone();

        join_set.spawn(async move {
            let start = Instant::now();

            let req = AgentRequest {
                prompt: query,
                system_prompt: Some(system_prompt),
                working_dir,
                model: Some(model),
                provider: Some(provider),
                max_turns: Some(max_turns),
                timeout: Some(Duration::from_secs(300)),
                reasoning_effort: None,
            };

            let result = match backend.execute(req).await {
                Ok(handle) => {
                    let agent_name = name.clone();
                    let event_tx = event_tx.clone();

                    // Drain events, forwarding to the main event channel
                    let mut events_rx = handle.events;
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();

                    while let Some(event) = events_rx.recv().await {
                        match &event {
                            AgentEvent::TextDelta(t) => text_parts.push(t.clone()),
                            AgentEvent::TextComplete(t) => {
                                text_parts.clear();
                                text_parts.push(t.clone());
                            }
                            AgentEvent::Complete(result) => {
                                let _ = event_tx
                                    .send(HeavyEvent::AgentCompleted {
                                        name: agent_name.clone(),
                                        duration: start.elapsed(),
                                    })
                                    .await;
                                return (name, Ok(result.clone()));
                            }
                            AgentEvent::ToolCallStart { id, name: tname } => {
                                tool_calls.push(crate::backend::ToolCallRecord {
                                    id: id.clone(),
                                    name: tname.clone(),
                                    output: String::new(),
                                });
                            }
                            AgentEvent::ToolCallEnd { id, output } => {
                                if let Some(record) =
                                    tool_calls.iter_mut().find(|r| r.id == *id)
                                {
                                    record.output = output.clone();
                                }
                            }
                            _ => {}
                        }

                        let _ = event_tx
                            .send(HeavyEvent::AgentEvent {
                                name: agent_name.clone(),
                                inner: event,
                            })
                            .await;
                    }

                    // Stream ended without Complete event
                    Ok(AgentResult {
                        text: text_parts.join(""),
                        status: crate::backend::AgentStatus::Completed,
                        tool_calls,
                        usage: None,
                    })
                }
                Err(e) => Err(e),
            };

            let _ = event_tx
                .send(HeavyEvent::AgentCompleted {
                    name: name.clone(),
                    duration: start.elapsed(),
                })
                .await;

            (name, result)
        });
    }

    // Collect all results
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((name, Ok(agent_result))) => {
                agent_outputs.insert(name, agent_result);
            }
            Ok((name, Err(e))) => {
                eprintln!("[{}] Error: {}", name, e);
                agent_outputs.insert(
                    name,
                    AgentResult {
                        text: format!("Error: {}", e),
                        status: crate::backend::AgentStatus::Failed(e.to_string()),
                        tool_calls: vec![],
                        usage: None,
                    },
                );
            }
            Err(e) => {
                eprintln!("Join error: {}", e);
            }
        }
    }

    // ── Phase 3: Synthesis ──────────────────────────────────────────
    let _ = event_tx.send(HeavyEvent::SynthesisStarted).await;

    let synthesis_outputs: Vec<(String, String, String)> = selected_agents
        .iter()
        .filter(|r| r.name != "Captain")
        .filter_map(|r| {
            agent_outputs
                .get(r.name)
                .map(|result| (r.name.to_string(), r.domain.to_string(), result.text.clone()))
        })
        .collect();

    let synthesis_prompt = agents::synthesis_prompt(&config.query, &synthesis_outputs);
    let captain_model = config
        .captain_model
        .as_deref()
        .unwrap_or(&config.model)
        .to_string();

    let synthesis_text = run_captain_call(
        &backend,
        &synthesis_prompt,
        &captain_model,
        &config.provider,
        &working_dir,
        &event_tx,
    )
    .await
    .context("Captain synthesis failed")?;

    // ── Phase 4: Debate (optional) ──────────────────────────────────
    let mut debate_critiques = Vec::new();
    let mut final_text = synthesis_text.clone();

    for round in 0..config.debate_rounds {
        let _ = event_tx
            .send(HeavyEvent::DebateRound { round: round + 1 })
            .await;

        // Collect critiques from all active non-Captain agents
        let critique_prompt = agents::critique_prompt(&config.query, &final_text);
        let mut round_critiques = HashMap::new();
        let mut critique_join_set = tokio::task::JoinSet::new();

        for role in &selected_agents {
            if role.name == "Captain" {
                continue;
            }

            let backend = Arc::clone(&backend);
            let name = role.name.to_string();
            let system_prompt = role.system_prompt.to_string();
            let model = config.model.clone();
            let provider = config.provider.clone();
            let critique_prompt = critique_prompt.clone();
            let working_dir = working_dir.clone();

            critique_join_set.spawn(async move {
                let req = AgentRequest {
                    prompt: critique_prompt,
                    system_prompt: Some(system_prompt),
                    working_dir,
                    model: Some(model),
                    provider: Some(provider),
                    max_turns: Some(1), // Critiques don't need tools
                    timeout: Some(Duration::from_secs(120)),
                    reasoning_effort: None,
                };

                let result = match backend.execute(req).await {
                    Ok(handle) => handle.result().await,
                    Err(e) => Err(e),
                };
                (name, result)
            });
        }

        while let Some(result) = critique_join_set.join_next().await {
            if let Ok((name, Ok(result))) = result {
                if !result.text.is_empty() {
                    round_critiques.insert(name, result.text);
                }
            }
        }

        // Captain re-synthesizes
        let critiques_vec: Vec<(String, String)> = round_critiques
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let resynth_prompt =
            agents::resynthesis_prompt(&config.query, &final_text, &critiques_vec);

        final_text = run_captain_call(
            &backend,
            &resynth_prompt,
            &captain_model,
            &config.provider,
            &working_dir,
            &event_tx,
        )
        .await
        .context("Captain re-synthesis failed")?;

        debate_critiques.push(round_critiques);
    }

    let result = HeavyResult {
        final_text,
        agents_activated: agent_names,
        agent_outputs,
        debate_critiques,
        total_usage: None,
    };

    let _ = event_tx.send(HeavyEvent::Complete(result.clone())).await;
    Ok(result)
}

/// Phase 1: Ask Captain to select which agents to activate.
async fn route_query(
    config: &HeavyConfig,
    backend: &Arc<dyn AgentBackend>,
    working_dir: &std::path::Path,
) -> Result<Vec<&'static agents::AgentRole>> {
    let routing_prompt = agents::routing_prompt(&config.query);

    let req = AgentRequest {
        prompt: routing_prompt,
        system_prompt: None,
        working_dir: working_dir.to_path_buf(),
        model: Some(
            config
                .captain_model
                .as_deref()
                .unwrap_or(&config.model)
                .to_string(),
        ),
        provider: Some(config.provider.clone()),
        max_turns: Some(1),
        timeout: Some(Duration::from_secs(60)),
        reasoning_effort: None,
    };

    let handle = backend.execute(req).await?;
    let result = handle.result().await?;

    // Parse the JSON array of agent names from Captain's response
    let selected = parse_agent_selection(&result.text)?;

    // Apply max_agents cap
    let mut roles: Vec<&'static agents::AgentRole> = Vec::new();

    // Always include core roles
    for role in agents::core_roles() {
        roles.push(role);
    }

    // Add selected specialists
    for name in &selected {
        if let Some(role) = agents::role_by_name(name) {
            if !role.is_core && !roles.iter().any(|r| r.name == role.name) {
                roles.push(role);
            }
        }
    }

    // Apply cap
    if let Some(max) = config.max_agents {
        roles.truncate(max);
    }

    Ok(roles)
}

/// Parse Captain's JSON response into a list of agent names.
fn parse_agent_selection(text: &str) -> Result<Vec<String>> {
    // Find JSON array in the response (Captain might include extra text)
    let trimmed = text.trim();

    // Try to find a JSON array
    let json_str = if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let names: Vec<String> = serde_json::from_str(json_str)
        .context("Failed to parse Captain's agent selection as JSON array")?;

    Ok(names)
}

/// Run a Captain-only LLM call (routing or synthesis).
async fn run_captain_call(
    backend: &Arc<dyn AgentBackend>,
    prompt: &str,
    model: &str,
    provider: &str,
    working_dir: &std::path::Path,
    event_tx: &mpsc::Sender<HeavyEvent>,
) -> Result<String> {
    let captain = agents::role_by_name("Captain").expect("Captain role must exist");

    let req = AgentRequest {
        prompt: prompt.to_string(),
        system_prompt: Some(captain.system_prompt.to_string()),
        working_dir: working_dir.to_path_buf(),
        model: Some(model.to_string()),
        provider: Some(provider.to_string()),
        max_turns: Some(1),
        timeout: Some(Duration::from_secs(120)),
        reasoning_effort: None,
    };

    let handle = backend.execute(req).await?;
    let mut events = handle.events;
    let mut text_parts = Vec::new();

    while let Some(event) = events.recv().await {
        match &event {
            AgentEvent::TextDelta(t) => {
                text_parts.push(t.clone());
                let _ = event_tx
                    .send(HeavyEvent::SynthesisDelta(t.clone()))
                    .await;
            }
            AgentEvent::TextComplete(t) => {
                text_parts.clear();
                text_parts.push(t.clone());
            }
            AgentEvent::Complete(result) => {
                return Ok(result.text.clone());
            }
            _ => {}
        }
    }

    Ok(text_parts.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_selection_clean() {
        let result =
            parse_agent_selection(r#"["Captain", "Harper", "Benjamin", "Lucas", "Curie"]"#)
                .unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], "Captain");
    }

    #[test]
    fn test_parse_agent_selection_with_surrounding_text() {
        let result = parse_agent_selection(
            r#"Based on the query, I'll activate: ["Captain", "Harper", "Benjamin", "Lucas"] Done."#,
        )
        .unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_parse_agent_selection_invalid() {
        let result = parse_agent_selection("not json at all");
        assert!(result.is_err());
    }
}
