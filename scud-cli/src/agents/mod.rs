//! Agent definitions for model routing
//!
//! Agent definitions specify which harness and model to use for a task,
//! along with an optional custom prompt template.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::commands::spawn::terminal::Harness;

/// Agent definition loaded from .scud/agents/<name>.toml
#[derive(Debug, Clone, Deserialize)]
pub struct AgentDef {
    pub agent: AgentMeta,
    pub model: ModelConfig,
    #[serde(default)]
    pub prompt: PromptConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Harness to use: "rho", "claude", "opencode", or "cursor"
    #[serde(default = "default_harness")]
    pub harness: String,
    /// Model name to pass to CLI (e.g., "sonnet", "opus", "grok-4")
    #[serde(default)]
    pub model: Option<String>,
}

fn default_harness() -> String {
    "rho".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PromptConfig {
    /// Inline prompt template (supports {task.title}, {task.description}, etc.)
    pub template: Option<String>,
    /// Path to prompt template file (relative to .scud/agents/)
    pub template_file: Option<String>,
}

impl AgentDef {
    /// Load agent definition from .scud/agents/<name>.toml
    pub fn load(name: &str, project_root: &Path) -> Result<Self> {
        let path = project_root
            .join(".scud")
            .join("agents")
            .join(format!("{}.toml", name));

        if !path.exists() {
            anyhow::bail!(
                "Agent definition '{}' not found at {}",
                name,
                path.display()
            );
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read agent file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse agent file: {}", path.display()))
    }

    /// Try to load agent definition, return None if not found.
    /// Falls back to embedded agent definitions if not found on disk.
    pub fn try_load(name: &str, project_root: &Path) -> Option<Self> {
        Self::load(name, project_root)
            .ok()
            .or_else(|| Self::load_embedded(name))
    }

    /// Load from embedded agent definitions (compiled into the binary)
    fn load_embedded(name: &str) -> Option<Self> {
        let content = match name {
            "builder" => Some(include_str!("../assets/spawn-agents/builder.toml")),
            "fast-builder" => Some(include_str!("../assets/spawn-agents/fast-builder.toml")),
            "reviewer" => Some(include_str!("../assets/spawn-agents/reviewer.toml")),
            "planner" => Some(include_str!("../assets/spawn-agents/planner.toml")),
            "researcher" => Some(include_str!("../assets/spawn-agents/researcher.toml")),
            "analyzer" => Some(include_str!("../assets/spawn-agents/analyzer.toml")),
            "repairer" => Some(include_str!("../assets/spawn-agents/repairer.toml")),
            "tester" => Some(include_str!("../assets/spawn-agents/tester.toml")),
            "outside-generalist" => Some(include_str!(
                "../assets/spawn-agents/outside-generalist.toml"
            )),
            _ => None,
        };
        content.and_then(|c| toml::from_str(c).ok())
    }

    /// Get the harness for this agent
    pub fn harness(&self) -> Result<Harness> {
        Harness::parse(&self.model.harness)
    }

    /// Get the model name for this agent (if specified)
    pub fn model(&self) -> Option<&str> {
        self.model.model.as_deref()
    }

    /// Get the prompt template (if specified)
    pub fn prompt_template(&self, project_root: &Path) -> Option<String> {
        // Try inline template first
        if let Some(ref template) = self.prompt.template {
            return Some(template.clone());
        }

        // Try template file
        if let Some(ref template_file) = self.prompt.template_file {
            let path = project_root
                .join(".scud")
                .join("agents")
                .join(template_file);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Some(content);
            }
        }

        None
    }

    /// Create a default agent (Rho with claude-sonnet, no custom prompt)
    pub fn default_builder() -> Self {
        AgentDef {
            agent: AgentMeta {
                name: "builder".to_string(),
                description: "Default code implementation agent".to_string(),
            },
            model: ModelConfig {
                harness: "rho".to_string(),
                model: Some("claude-sonnet".to_string()),
            },
            prompt: PromptConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_agent_definition() {
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_file = agents_dir.join("reviewer.toml");
        let content = r#"
[agent]
name = "reviewer"
description = "Code review agent"

[model]
harness = "claude"
model = "opus"
"#;
        std::fs::write(&agent_file, content).unwrap();

        let agent = AgentDef::load("reviewer", temp.path()).unwrap();
        assert_eq!(agent.agent.name, "reviewer");
        assert_eq!(agent.model.harness, "claude");
        assert_eq!(agent.model.model, Some("opus".to_string()));
    }

    #[test]
    fn test_agent_not_found() {
        let temp = TempDir::new().unwrap();
        let result = AgentDef::load("nonexistent", temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_default_builder() {
        let agent = AgentDef::default_builder();
        assert_eq!(agent.agent.name, "builder");
        assert_eq!(agent.model.harness, "rho");
        assert_eq!(agent.model.model, Some("claude-sonnet".to_string()));
    }

    #[test]
    fn test_try_load_returns_none_for_missing() {
        let temp = TempDir::new().unwrap();
        let result = AgentDef::try_load("nonexistent", temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_prompt_template_inline() {
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_file = agents_dir.join("custom.toml");
        let content = r#"
[agent]
name = "custom"

[model]
harness = "claude"

[prompt]
template = "You are a custom agent for {task.title}"
"#;
        std::fs::write(&agent_file, content).unwrap();

        let agent = AgentDef::load("custom", temp.path()).unwrap();
        let template = agent.prompt_template(temp.path());
        assert_eq!(
            template,
            Some("You are a custom agent for {task.title}".to_string())
        );
    }

    #[test]
    fn test_prompt_template_file() {
        let temp = TempDir::new().unwrap();
        let agents_dir = temp.path().join(".scud").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Create template file
        let template_content = "Custom template from file: {task.description}";
        std::fs::write(agents_dir.join("custom.prompt"), template_content).unwrap();

        let agent_file = agents_dir.join("file-agent.toml");
        let content = r#"
[agent]
name = "file-agent"

[model]
harness = "opencode"

[prompt]
template_file = "custom.prompt"
"#;
        std::fs::write(&agent_file, content).unwrap();

        let agent = AgentDef::load("file-agent", temp.path()).unwrap();
        let template = agent.prompt_template(temp.path());
        assert_eq!(template, Some(template_content.to_string()));
    }

    #[test]
    fn test_harness_parsing() {
        let agent = AgentDef::default_builder();
        let harness = agent.harness().unwrap();
        assert_eq!(harness, Harness::Rho);
    }
}
