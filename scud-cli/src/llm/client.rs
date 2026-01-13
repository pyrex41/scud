use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

use crate::config::Config;
use crate::storage::Storage;

// Anthropic API structures
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

// OpenAI-compatible API structures (used by xAI, OpenAI, OpenRouter)
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenAIMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessageResponse,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessageResponse {
    content: String,
}

pub struct LLMClient {
    config: Config,
    api_key: String,
    client: reqwest::Client,
}

/// Info about which model is being used for display purposes
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub tier: &'static str, // "fast" or "smart"
    pub provider: String,
    pub model: String,
}

impl std::fmt::Display for ModelInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} model: {}/{}", self.tier, self.provider, self.model)
    }
}

impl LLMClient {
    pub fn new() -> Result<Self> {
        let storage = Storage::new(None);
        let config = storage.load_config()?;

        let api_key = if config.requires_api_key() {
            env::var(config.api_key_env_var()).with_context(|| {
                format!("{} environment variable not set", config.api_key_env_var())
            })?
        } else {
            String::new() // Claude CLI doesn't need API key
        };

        Ok(LLMClient {
            config,
            api_key,
            client: reqwest::Client::new(),
        })
    }

    pub fn new_with_project_root(project_root: PathBuf) -> Result<Self> {
        let storage = Storage::new(Some(project_root));
        let config = storage.load_config()?;

        let api_key = if config.requires_api_key() {
            env::var(config.api_key_env_var()).with_context(|| {
                format!("{} environment variable not set", config.api_key_env_var())
            })?
        } else {
            String::new() // Claude CLI doesn't need API key
        };

        Ok(LLMClient {
            config,
            api_key,
            client: reqwest::Client::new(),
        })
    }

    /// Get info about the smart model that will be used
    pub fn smart_model_info(&self, model_override: Option<&str>) -> ModelInfo {
        ModelInfo {
            tier: "smart",
            provider: self.config.smart_provider().to_string(),
            model: model_override
                .unwrap_or(self.config.smart_model())
                .to_string(),
        }
    }

    /// Get info about the fast model that will be used
    pub fn fast_model_info(&self, model_override: Option<&str>) -> ModelInfo {
        ModelInfo {
            tier: "fast",
            provider: self.config.fast_provider().to_string(),
            model: model_override
                .unwrap_or(self.config.fast_model())
                .to_string(),
        }
    }

    pub async fn complete(&self, prompt: &str) -> Result<String> {
        self.complete_with_model(prompt, None, None).await
    }

    /// Complete using the smart model (for validation/analysis tasks with large context)
    /// Use user override if provided, otherwise fall back to configured smart_model
    pub async fn complete_smart(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        let model = model_override.unwrap_or(self.config.smart_model());
        let provider = self.config.smart_provider();
        self.complete_with_model(prompt, Some(model), Some(provider))
            .await
    }

    /// Complete using the fast model (for generation tasks)
    /// Use user override if provided, otherwise fall back to configured fast_model
    pub async fn complete_fast(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        let model = model_override.unwrap_or(self.config.fast_model());
        let provider = self.config.fast_provider();
        self.complete_with_model(prompt, Some(model), Some(provider))
            .await
    }

    pub async fn complete_with_model(
        &self,
        prompt: &str,
        model_override: Option<&str>,
        provider_override: Option<&str>,
    ) -> Result<String> {
        let provider = provider_override.unwrap_or(&self.config.llm.provider);
        match provider.as_ref() {
            "claude-cli" => self.complete_claude_cli(prompt, model_override).await,
            "codex" => self.complete_codex_cli(prompt, model_override).await,
            "anthropic" => {
                self.complete_anthropic_with_model(prompt, model_override)
                    .await
            }
            "xai" | "openai" | "openrouter" => {
                self.complete_openai_compatible_with_model(prompt, model_override)
                    .await
            }
            _ => anyhow::bail!("Unsupported provider: {}", self.config.llm.provider),
        }
    }

    async fn complete_anthropic_with_model(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        let model = model_override.unwrap_or(&self.config.llm.model);
        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: self.config.llm.max_tokens,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post(self.config.api_endpoint())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic API response")?;

        Ok(api_response
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default())
    }

    async fn complete_openai_compatible_with_model(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        let model = model_override.unwrap_or(&self.config.llm.model);
        let request = OpenAIRequest {
            model: model.to_string(),
            max_tokens: self.config.llm.max_tokens,
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let mut request_builder = self
            .client
            .post(self.config.api_endpoint())
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json");

        // OpenRouter requires additional headers
        if self.config.llm.provider == "openrouter" {
            request_builder = request_builder
                .header("HTTP-Referer", "https://github.com/scud-cli")
                .header("X-Title", "SCUD Task Master");
        }

        let response = request_builder
            .json(&request)
            .send()
            .await
            .with_context(|| {
                format!("Failed to send request to {} API", self.config.llm.provider)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "{} API error ({}): {}",
                self.config.llm.provider,
                status,
                error_text
            );
        }

        let api_response: OpenAIResponse = response.json().await.with_context(|| {
            format!("Failed to parse {} API response", self.config.llm.provider)
        })?;

        Ok(api_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    pub async fn complete_json<T>(&self, prompt: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.complete_json_with_model(prompt, None).await
    }

    /// Complete JSON using the smart model (for validation/analysis tasks)
    pub async fn complete_json_smart<T>(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response_text = self.complete_smart(prompt, model_override).await?;
        Self::parse_json_response(&response_text)
    }

    /// Complete JSON using the fast model (for generation tasks)
    pub async fn complete_json_fast<T>(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response_text = self.complete_fast(prompt, model_override).await?;
        Self::parse_json_response(&response_text)
    }

    pub async fn complete_json_with_model<T>(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response_text = self
            .complete_with_model(prompt, model_override, None)
            .await?;
        Self::parse_json_response(&response_text)
    }

    fn parse_json_response<T>(response_text: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        // Try to find JSON in the response (LLM might include markdown or explanations)
        let json_str = Self::extract_json(response_text);

        serde_json::from_str(json_str).with_context(|| {
            // Provide helpful error context
            let preview = if json_str.len() > 500 {
                format!("{}...", &json_str[..500])
            } else {
                json_str.to_string()
            };
            format!(
                "Failed to parse JSON from LLM response. Response preview:\n{}",
                preview
            )
        })
    }

    /// Extract JSON from LLM response, handling markdown code blocks and extra text
    fn extract_json(response: &str) -> &str {
        // First, try to extract from markdown code blocks
        if let Some(start) = response.find("```json") {
            let content_start = start + 7; // Skip "```json"
            if let Some(end) = response[content_start..].find("```") {
                return response[content_start..content_start + end].trim();
            }
        }

        // Try plain code blocks
        if let Some(start) = response.find("```") {
            let content_start = start + 3;
            // Skip language identifier if present (e.g., "```\n")
            let content_start = response[content_start..]
                .find('\n')
                .map(|i| content_start + i + 1)
                .unwrap_or(content_start);
            if let Some(end) = response[content_start..].find("```") {
                return response[content_start..content_start + end].trim();
            }
        }

        // Try to find array JSON
        if let Some(start) = response.find('[') {
            if let Some(end) = response.rfind(']') {
                if end > start {
                    return &response[start..=end];
                }
            }
        }

        // Try to find object JSON
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                if end > start {
                    return &response[start..=end];
                }
            }
        }

        response.trim()
    }

    async fn complete_claude_cli(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        let model = model_override.unwrap_or(&self.config.llm.model);

        // Build the claude command
        let mut cmd = Command::new("claude");
        cmd.arg("-p") // Print mode (headless)
            .arg("--output-format")
            .arg("json")
            .arg("--model")
            .arg(model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let mut child = cmd.spawn().context("Failed to spawn 'claude' command. Make sure Claude Code is installed and 'claude' is in your PATH")?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .context("Failed to write prompt to claude stdin")?;
            drop(stdin); // Close stdin
        }

        // Wait for completion
        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for claude command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Claude CLI error: {}", stderr);
        }

        // Parse JSON output
        let stdout =
            String::from_utf8(output.stdout).context("Claude CLI output is not valid UTF-8")?;

        #[derive(Deserialize)]
        struct ClaudeCliResponse {
            result: String,
        }

        let response: ClaudeCliResponse =
            serde_json::from_str(&stdout).context("Failed to parse Claude CLI JSON response")?;

        Ok(response.result)
    }

    async fn complete_codex_cli(
        &self,
        prompt: &str,
        model_override: Option<&str>,
    ) -> Result<String> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        let model = model_override.unwrap_or(&self.config.llm.model);

        // Build the codex command
        // Codex CLI uses similar headless mode to Claude Code
        let mut cmd = Command::new("codex");
        cmd.arg("-p") // Prompt mode (headless/non-interactive)
            .arg("--model")
            .arg(model)
            .arg("--output-format")
            .arg("json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let mut child = cmd.spawn().context("Failed to spawn 'codex' command. Make sure OpenAI Codex CLI is installed and 'codex' is in your PATH")?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .context("Failed to write prompt to codex stdin")?;
            drop(stdin); // Close stdin
        }

        // Wait for completion
        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for codex command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Codex CLI error: {}", stderr);
        }

        // Parse JSON output
        let stdout =
            String::from_utf8(output.stdout).context("Codex CLI output is not valid UTF-8")?;

        // Codex outputs JSON with a result field similar to Claude CLI
        #[derive(Deserialize)]
        struct CodexCliResponse {
            result: String,
        }

        let response: CodexCliResponse =
            serde_json::from_str(&stdout).context("Failed to parse Codex CLI JSON response")?;

        Ok(response.result)
    }
}
