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

impl LLMClient {
    pub fn new() -> Result<Self> {
        let storage = Storage::new(None);
        let config = storage.load_config()?;

        let api_key = env::var(config.api_key_env_var()).with_context(|| {
            format!("{} environment variable not set", config.api_key_env_var())
        })?;

        Ok(LLMClient {
            config,
            api_key,
            client: reqwest::Client::new(),
        })
    }

    pub fn new_with_project_root(project_root: PathBuf) -> Result<Self> {
        let storage = Storage::new(Some(project_root));
        let config = storage.load_config()?;

        let api_key = env::var(config.api_key_env_var()).with_context(|| {
            format!("{} environment variable not set", config.api_key_env_var())
        })?;

        Ok(LLMClient {
            config,
            api_key,
            client: reqwest::Client::new(),
        })
    }

    pub async fn complete(&self, prompt: &str) -> Result<String> {
        self.complete_with_model(prompt, None).await
    }

    pub async fn complete_with_model(&self, prompt: &str, model_override: Option<&str>) -> Result<String> {
        match self.config.llm.provider.as_str() {
            "anthropic" => self.complete_anthropic_with_model(prompt, model_override).await,
            "xai" | "openai" | "openrouter" => self.complete_openai_compatible_with_model(prompt, model_override).await,
            _ => anyhow::bail!("Unsupported provider: {}", self.config.llm.provider),
        }
    }

    async fn complete_anthropic(&self, prompt: &str) -> Result<String> {
        self.complete_anthropic_with_model(prompt, None).await
    }

    async fn complete_anthropic_with_model(&self, prompt: &str, model_override: Option<&str>) -> Result<String> {
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

    async fn complete_openai_compatible(&self, prompt: &str) -> Result<String> {
        self.complete_openai_compatible_with_model(prompt, None).await
    }

    async fn complete_openai_compatible_with_model(&self, prompt: &str, model_override: Option<&str>) -> Result<String> {
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
        let response_text = self.complete(prompt).await?;

        // Try to find JSON in the response (LLM might include markdown or explanations)
        let json_str = if let Some(start) = response_text.find('[') {
            if let Some(end) = response_text.rfind(']') {
                &response_text[start..=end]
            } else {
                &response_text
            }
        } else if let Some(start) = response_text.find('{') {
            if let Some(end) = response_text.rfind('}') {
                &response_text[start..=end]
            } else {
                &response_text
            }
        } else {
            &response_text
        };

        serde_json::from_str(json_str).context("Failed to parse JSON from LLM response")
    }
}
