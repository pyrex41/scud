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
    /// Default model (used when no tier specified)
    pub model: String,
    /// Smart provider for validation/analysis tasks
    #[serde(default = "default_smart_provider")]
    pub smart_provider: String,
    /// Smart model for validation/analysis tasks (large context)
    #[serde(default = "default_smart_model")]
    pub smart_model: String,
    /// Fast provider for generation tasks
    #[serde(default = "default_fast_provider")]
    pub fast_provider: String,
    /// Fast model for generation tasks
    #[serde(default = "default_fast_model")]
    pub fast_model: String,
    #[serde(default)]
    pub max_tokens: u32,
}

fn default_smart_provider() -> String {
    "claude-cli".to_string()
}

fn default_smart_model() -> String {
    "opus".to_string()
}

fn default_fast_provider() -> String {
    "xai".to_string()
}

fn default_fast_model() -> String {
    "grok-code-fast-1".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            llm: LLMConfig {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                smart_provider: "claude-cli".to_string(),
                smart_model: "opus".to_string(),
                fast_provider: "xai".to_string(),
                fast_model: "grok-code-fast-1".to_string(),
                max_tokens: 16000,
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
        Self::api_key_env_var_for_provider(&self.llm.provider)
    }

    pub fn api_key_env_var_for_provider(provider: &str) -> &str {
        match provider {
            "anthropic" => "ANTHROPIC_API_KEY",
            "xai" => "XAI_API_KEY",
            "openai" => "OPENAI_API_KEY",
            "openrouter" => "OPENROUTER_API_KEY",
            "claude-cli" => "NONE", // Claude CLI doesn't need API key
            "codex" => "NONE",      // Codex CLI doesn't need API key
            _ => "API_KEY",
        }
    }

    pub fn requires_api_key(&self) -> bool {
        let providers = [&self.llm.provider, &self.llm.smart_provider, &self.llm.fast_provider];
        providers.iter().any(|p| !matches!(p.as_str(), "claude-cli" | "codex"))
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
            "xai" => "grok-code-fast-1",
            "anthropic" => "claude-sonnet-4-5-20250929",
            "openai" => "o3-mini",
            "openrouter" => "anthropic/claude-sonnet-4.5",
            "claude-cli" => "sonnet", // Claude CLI model names: sonnet, opus, haiku
            "codex" => "gpt-5.1",     // Codex CLI default model
            _ => "grok-code-fast-1",
        }
    }

    /// Get suggested models for a provider (for display in init)
    pub fn suggested_models_for_provider(provider: &str) -> Vec<&str> {
        match provider {
            "xai" => vec![
                "grok-code-fast-1",
                "grok-4-1-fast-reasoning",
                "grok-4-1-fast",
                "grok-3-fast",
            ],
            "anthropic" => vec![
                "claude-sonnet-4-5-20250929",
                "claude-opus-4-5-20251101",
                "claude-haiku-4-5-20251001",
                "claude-opus-4-1-20250805",
            ],
            "openai" => vec![
                "gpt-5.2-high",
                "gpt-5.1",
                "gpt-5.1-mini",
                "o3-mini",
                "o3",
                "o4-mini",
                "gpt-4.1",
            ],
            "openrouter" => vec![
                "anthropic/claude-sonnet-4.5",
                "anthropic/claude-opus-4.5",
                "openai/o3-mini",
                "openai/gpt-4.1",
                "xai/grok-4-1-fast-reasoning",
            ],
            "claude-cli" => vec![
                "opus",   // Claude Opus 4.5 - smart/reasoning
                "sonnet", // Claude Sonnet - fast/capable
                "haiku",  // Claude Haiku - fastest
            ],
            "codex" => vec![
                "gpt-5.2-high", // Smart/reasoning model
                "gpt-5.1",      // Capable model
                "gpt-5.1-mini", // Fast model
                "o3",           // Reasoning model
                "o3-mini",      // Fast reasoning
            ],
            _ => vec![],
        }
    }

    /// Get the smart provider (for validation/analysis tasks with large context)
    pub fn smart_provider(&self) -> &str {
        &self.llm.smart_provider
    }

    /// Get the smart model (for validation/analysis tasks with large context)
    pub fn smart_model(&self) -> &str {
        &self.llm.smart_model
    }

    /// Get the fast provider (for generation tasks)
    pub fn fast_provider(&self) -> &str {
        &self.llm.fast_provider
    }

    /// Get the fast model (for generation tasks)
    pub fn fast_model(&self) -> &str {
        &self.llm.fast_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.llm.provider, "claude-cli");
        assert_eq!(config.llm.model, "sonnet");
        assert_eq!(config.llm.smart_provider, "claude-cli");
        assert_eq!(config.llm.smart_model, "opus");
        assert_eq!(config.llm.fast_provider, "xai");
        assert_eq!(config.llm.fast_model, "grok-code-fast-1");
        assert_eq!(config.llm.max_tokens, 16000);
    }

    #[test]
    fn test_model_tiers() {
        let config = Config::default();
        assert_eq!(config.smart_provider(), "claude-cli");
        assert_eq!(config.smart_model(), "opus");
        assert_eq!(config.fast_provider(), "xai");
        assert_eq!(config.fast_model(), "grok-code-fast-1");
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

        config.llm.provider = "claude-cli".to_string();
        config.llm.smart_provider = "claude-cli".to_string();
        config.llm.fast_provider = "claude-cli".to_string();
        assert!(!config.requires_api_key());
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
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                smart_provider: "claude-cli".to_string(),
                smart_model: "opus".to_string(),
                fast_provider: "xai".to_string(),
                fast_model: "haiku".to_string(),
                max_tokens: 8192,
            },
        };

        config.save(&config_path).unwrap();
        assert!(config_path.exists());

        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.llm.provider, "claude-cli");
        assert_eq!(loaded.llm.model, "sonnet");
        assert_eq!(loaded.llm.smart_provider, "claude-cli");
        assert_eq!(loaded.llm.smart_model, "opus");
        assert_eq!(loaded.llm.fast_provider, "xai");
        assert_eq!(loaded.llm.fast_model, "haiku");
        assert_eq!(loaded.llm.max_tokens, 8192);
    }

    #[test]
    fn test_default_models() {
        assert_eq!(
            Config::default_model_for_provider("xai"),
            "grok-code-fast-1"
        );
        assert_eq!(
            Config::default_model_for_provider("anthropic"),
            "claude-sonnet-4-5-20250929"
        );
        assert_eq!(Config::default_model_for_provider("openai"), "o3-mini");
        assert_eq!(Config::default_model_for_provider("claude-cli"), "sonnet");
    }

    #[test]
    fn test_load_config_without_model_tiers() {
        // Test backward compatibility - loading a config without smart/fast models
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Write a config without smart_model and fast_model
        std::fs::write(
            &config_path,
            r#"[llm]
provider = "xai"
model = "grok-code-fast-1"
max_tokens = 4096
"#,
        )
        .unwrap();

        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.llm.provider, "xai");
        assert_eq!(loaded.llm.model, "grok-code-fast-1");
        // Should use defaults for missing fields
        assert_eq!(loaded.llm.smart_provider, "claude-cli");
        assert_eq!(loaded.llm.smart_model, "opus");
        assert_eq!(loaded.llm.fast_provider, "xai");
        assert_eq!(loaded.llm.fast_model, "grok-code-fast-1");
    }
}
