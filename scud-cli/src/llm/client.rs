use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<Content>,
}

#[derive(Debug, Deserialize)]
struct Content {
    text: String,
}

pub struct LLMClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl LLMClient {
    pub fn new() -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable not set")?;

        let model =
            env::var("SCUD_MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        Ok(LLMClient {
            api_key,
            model,
            client: reqwest::Client::new(),
        })
    }

    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
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
