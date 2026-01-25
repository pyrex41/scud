//! HTTP client for OpenCode Server
//!
//! Provides async HTTP operations for creating sessions, sending messages,
//! and managing agent lifecycle via the OpenCode Server REST API.

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

use super::types::*;

/// HTTP client for OpenCode Server API
pub struct OpenCodeClient {
    base_url: String,
    client: Client,
}

impl OpenCodeClient {
    /// Create a new client connecting to the given base URL
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min timeout for long operations
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Create with default localhost URL
    pub fn localhost(port: u16) -> Self {
        Self::new(&format!("http://127.0.0.1:{}", port))
    }

    /// Check if server is ready
    pub async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await;

        match response {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Get server info
    pub async fn server_info(&self) -> Result<ServerInfo> {
        let response = self
            .client
            .get(format!("{}/", self.base_url))
            .send()
            .await
            .context("Failed to get server info")?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
                details: None,
            });
            anyhow::bail!("Server error: {}", error.error);
        }

        response
            .json()
            .await
            .context("Failed to parse server info")
    }

    /// Create a new session
    pub async fn create_session(&self, title: &str) -> Result<Session> {
        let request = CreateSessionRequest {
            title: title.to_string(),
            system_prompt: None,
        };

        let response = self
            .client
            .post(format!("{}/session", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to create session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create session ({}): {}", status, error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse session response")
    }

    /// Send a message/prompt to a session
    pub async fn send_message(
        &self,
        session_id: &str,
        text: &str,
        model: Option<(&str, &str)>, // (provider_id, model_id)
    ) -> Result<()> {
        let request = MessageRequest {
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            model: model.map(|(provider, model_id)| ModelSpec {
                provider_id: provider.to_string(),
                model_id: model_id.to_string(),
            }),
        };

        let response = self
            .client
            .post(format!("{}/session/{}/message", self.base_url, session_id))
            .json(&request)
            .send()
            .await
            .context("Failed to send message")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send message ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// Get session status
    pub async fn get_session_status(&self, session_id: &str) -> Result<SessionStatus> {
        let response = self
            .client
            .get(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("Failed to get session status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get session ({}): {}", status, error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse session status")
    }

    /// Abort/cancel a running session
    pub async fn abort_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/session/{}/abort", self.base_url, session_id))
            .send()
            .await
            .context("Failed to abort session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to abort session ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/session/{}", self.base_url, session_id))
            .send()
            .await
            .context("Failed to delete session")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to delete session ({}): {}", status, error_text);
        }

        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<Session>> {
        let response = self
            .client
            .get(format!("{}/session", self.base_url))
            .send()
            .await
            .context("Failed to list sessions")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to list sessions ({}): {}", status, error_text);
        }

        response
            .json()
            .await
            .context("Failed to parse sessions list")
    }

    /// Get the SSE event stream URL
    pub fn event_stream_url(&self) -> String {
        format!("{}/event", self.base_url)
    }

    /// Get base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenCodeClient::localhost(4096);
        assert_eq!(client.base_url(), "http://127.0.0.1:4096");
    }

    #[test]
    fn test_client_creation_with_trailing_slash() {
        let client = OpenCodeClient::new("http://localhost:4096/");
        assert_eq!(client.base_url(), "http://localhost:4096");
    }

    #[test]
    fn test_event_stream_url() {
        let client = OpenCodeClient::new("http://localhost:4096");
        assert_eq!(client.event_stream_url(), "http://localhost:4096/event");
    }

    #[test]
    fn test_localhost_constructor() {
        let client = OpenCodeClient::localhost(8080);
        assert_eq!(client.base_url(), "http://127.0.0.1:8080");
    }
}
