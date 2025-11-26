use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LLMConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub max_tokens: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            llm: LLMConfig {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-5-20250929".to_string(),
                max_tokens: 4096,
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))
    }

    pub fn api_key_env_var(&self) -> &str {
        match self.llm.provider.as_str() {
            "anthropic" => "ANTHROPIC_API_KEY",
            "xai" => "XAI_API_KEY",
            "openai" => "OPENAI_API_KEY",
            "openrouter" => "OPENROUTER_API_KEY",
            "claude-cli" => "NONE", // Claude CLI doesn't need API key
            _ => "API_KEY",
        }
    }

    pub fn requires_api_key(&self) -> bool {
        self.llm.provider != "claude-cli"
    }

    pub fn api_endpoint(&self) -> &str {
        match self.llm.provider.as_str() {
            "anthropic" => "https://api.anthropic.com/v1/messages",
            "xai" => "https://api.x.ai/v1/chat/completions",
            "openai" => "https://api.openai.com/v1/chat/completions",
            "openrouter" => "https://openrouter.ai/api/v1/chat/completions",
            _ => "https://api.anthropic.com/v1/messages",
        }
    }

    pub fn default_model_for_provider(provider: &str) -> &str {
        match provider {
            "anthropic" => "claude-sonnet-4-5-20250929",
            "xai" => "grok-4-1-fast-reasoning",
            "openai" => "o3-mini",
            "openrouter" => "anthropic/claude-sonnet-4.5",
            "claude-cli" => "sonnet", // Claude CLI model names: sonnet, opus, haiku
            _ => "claude-sonnet-4-5-20250929",
        }
    }

    /// Get suggested models for a provider (for display in init)
    pub fn suggested_models_for_provider(provider: &str) -> Vec<&str> {
        match provider {
            "xai" => vec![
                "grok-4-1-fast-reasoning",
                "grok-4-1-fast",
                "grok-3-fast",
                "grok-3",
            ],
            "anthropic" => vec![
                "claude-sonnet-4-5-20250929",
                "claude-opus-4-5-20251101",
                "claude-haiku-4-5-20251001",
                "claude-opus-4-1-20250805",
            ],
            "openai" => vec![
                "o3-mini",
                "o3",
                "o4-mini",
                "gpt-4.1",
                "gpt-4o",
                "gpt-4o-mini",
            ],
            "openrouter" => vec![
                "anthropic/claude-sonnet-4.5",
                "anthropic/claude-opus-4.5",
                "openai/o3-mini",
                "openai/gpt-4.1",
                "xai/grok-4-1-fast-reasoning",
            ],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.llm.provider, "anthropic");
        assert_eq!(config.llm.model, "claude-sonnet-4-5-20250929");
        assert_eq!(config.llm.max_tokens, 4096);
    }

    #[test]
    fn test_api_key_env_vars() {
        let mut config = Config::default();

        config.llm.provider = "anthropic".to_string();
        assert_eq!(config.api_key_env_var(), "ANTHROPIC_API_KEY");

        config.llm.provider = "xai".to_string();
        assert_eq!(config.api_key_env_var(), "XAI_API_KEY");

        config.llm.provider = "openai".to_string();
        assert_eq!(config.api_key_env_var(), "OPENAI_API_KEY");
    }

    #[test]
    fn test_api_endpoints() {
        let mut config = Config::default();

        config.llm.provider = "anthropic".to_string();
        assert_eq!(
            config.api_endpoint(),
            "https://api.anthropic.com/v1/messages"
        );

        config.llm.provider = "xai".to_string();
        assert_eq!(
            config.api_endpoint(),
            "https://api.x.ai/v1/chat/completions"
        );

        config.llm.provider = "openai".to_string();
        assert_eq!(
            config.api_endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = Config {
            llm: LLMConfig {
                provider: "xai".to_string(),
                model: "grok-code-fast-1".to_string(),
                max_tokens: 8192,
            },
        };

        config.save(&config_path).unwrap();
        assert!(config_path.exists());

        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.llm.provider, "xai");
        assert_eq!(loaded.llm.model, "grok-code-fast-1");
        assert_eq!(loaded.llm.max_tokens, 8192);
    }

    #[test]
    fn test_default_models() {
        assert_eq!(
            Config::default_model_for_provider("anthropic"),
            "claude-sonnet-4-5-20250929"
        );
        assert_eq!(
            Config::default_model_for_provider("xai"),
            "grok-4-1-fast-reasoning"
        );
        assert_eq!(Config::default_model_for_provider("openai"), "o3-mini");
    }
}
