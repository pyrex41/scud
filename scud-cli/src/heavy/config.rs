//! Configuration for Heavy mode.
//!
//! Supports CLI args with optional overrides from `.scud/heavy.toml`.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Runtime configuration for a Heavy execution.
pub struct HeavyConfig {
    /// The user's query to reason about.
    pub query: String,
    /// LLM provider (default: "xai").
    pub provider: String,
    /// Model to use (default: "grok-4-0709").
    pub model: String,
    /// Optional stronger model for Captain synthesis calls.
    pub captain_model: Option<String>,
    /// Max agents to activate (None = let Captain decide).
    pub max_agents: Option<usize>,
    /// Number of debate rounds (default: 0).
    pub debate_rounds: usize,
    /// Max agentic tool turns per agent (default: 10).
    pub max_turns: usize,
    /// Show intermediate agent outputs and tool calls.
    pub verbose: bool,
    /// Output structured JSON.
    pub json_output: bool,
}

impl Default for HeavyConfig {
    fn default() -> Self {
        Self {
            query: String::new(),
            provider: "xai".to_string(),
            model: "grok-4-0709".to_string(),
            captain_model: None,
            max_agents: None,
            debate_rounds: 0,
            max_turns: 10,
            verbose: false,
            json_output: false,
        }
    }
}

impl HeavyConfig {
    /// Apply overrides from a `.scud/heavy.toml` file if it exists.
    pub fn apply_toml_defaults(&mut self, project_dir: &Path) -> Result<()> {
        let toml_path = project_dir.join(".scud").join("heavy.toml");
        if !toml_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&toml_path)?;
        let file: HeavyToml = toml::from_str(&content)?;

        if let Some(defaults) = file.defaults {
            // Only apply TOML defaults for fields not explicitly set by CLI
            if self.provider == "xai" {
                if let Some(p) = defaults.provider {
                    self.provider = p;
                }
            }
            if self.model == "grok-4-0709" {
                if let Some(m) = defaults.model {
                    self.model = m;
                }
            }
            if self.captain_model.is_none() {
                self.captain_model = defaults.captain_model;
            }
            if self.max_turns == 10 {
                if let Some(t) = defaults.max_turns {
                    self.max_turns = t;
                }
            }
        }

        Ok(())
    }

    /// Get tool overrides from TOML config for a specific agent.
    pub fn agent_tool_override(project_dir: &Path, agent_name: &str) -> Option<Vec<String>> {
        let toml_path = project_dir.join(".scud").join("heavy.toml");
        if !toml_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&toml_path).ok()?;
        let file: HeavyToml = toml::from_str(&content).ok()?;

        let agents = file.agents?;
        let agent_config = agents.get(&agent_name.to_lowercase())?;
        Some(agent_config.tools.clone())
    }
}

/// Schema for `.scud/heavy.toml`.
#[derive(Deserialize, Default)]
struct HeavyToml {
    defaults: Option<HeavyDefaults>,
    agents: Option<HashMap<String, AgentTomlConfig>>,
}

#[derive(Deserialize, Default)]
struct HeavyDefaults {
    provider: Option<String>,
    model: Option<String>,
    captain_model: Option<String>,
    max_turns: Option<usize>,
}

#[derive(Deserialize)]
struct AgentTomlConfig {
    tools: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HeavyConfig::default();
        assert_eq!(config.provider, "xai");
        assert_eq!(config.model, "grok-4-0709");
        assert_eq!(config.max_turns, 10);
        assert_eq!(config.debate_rounds, 0);
        assert!(!config.verbose);
        assert!(!config.json_output);
    }

    #[test]
    fn test_toml_missing_is_ok() {
        let mut config = HeavyConfig::default();
        let result = config.apply_toml_defaults(Path::new("/nonexistent"));
        assert!(result.is_ok());
    }
}
