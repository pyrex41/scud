//! OAuth token reader for Claude Code's macOS Keychain credentials.
//!
//! Reads OAuth tokens stored by Claude Code in the macOS Keychain,
//! enabling direct Anthropic API calls using subscription billing (Pro/Max).

use anyhow::{Context, Result};
use serde::Deserialize;

/// OAuth credentials stored by Claude Code
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeOAuthCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeychainData {
    claude_ai_oauth: Option<ClaudeOAuthCredentials>,
}

/// API credential type resolved from available sources
#[derive(Debug)]
pub enum ApiCredential {
    /// Bearer token from Claude Code OAuth (subscription billing)
    OAuth(String),
    /// Standard x-api-key (API billing)
    ApiKey(String),
}

/// Read OAuth credentials from Claude Code's macOS Keychain entry.
///
/// Returns `Ok(None)` if no credentials are stored (e.g. Claude Code
/// not installed or user not logged in).
pub fn read_claude_oauth() -> Result<Option<ClaudeOAuthCredentials>> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .context("Failed to read from macOS Keychain")?;

    if !output.status.success() {
        return Ok(None);
    }

    let json_str =
        String::from_utf8(output.stdout).context("Keychain data is not valid UTF-8")?;
    let data: KeychainData =
        serde_json::from_str(json_str.trim()).context("Failed to parse keychain JSON")?;

    Ok(data.claude_ai_oauth)
}

/// Check if an OAuth token is still valid (with 5-minute buffer).
pub fn is_token_valid(creds: &ClaudeOAuthCredentials) -> bool {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    creds.expires_at > now_ms + 300_000
}

/// Resolve the best available API credential for Anthropic.
///
/// Priority: OAuth token (subscription) > ANTHROPIC_API_KEY env var.
pub fn resolve_anthropic_credential() -> Result<ApiCredential> {
    // Try OAuth first
    if let Ok(Some(creds)) = read_claude_oauth() {
        if is_token_valid(&creds) {
            return Ok(ApiCredential::OAuth(creds.access_token));
        }
        tracing::warn!("Claude Code OAuth token expired, falling back to API key");
    }

    // Fall back to API key
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        return Ok(ApiCredential::ApiKey(key));
    }

    anyhow::bail!(
        "No Anthropic credentials available. Either:\n\
         - Log in to Claude Code (`claude` CLI) for OAuth, or\n\
         - Set ANTHROPIC_API_KEY environment variable"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_token_valid_future_expiry() {
        let future_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000; // 1 hour from now

        let creds = ClaudeOAuthCredentials {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            expires_at: future_ms,
        };

        assert!(is_token_valid(&creds));
    }

    #[test]
    fn test_is_token_valid_past_expiry() {
        let creds = ClaudeOAuthCredentials {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            expires_at: 1000, // Way in the past
        };

        assert!(!is_token_valid(&creds));
    }

    #[test]
    fn test_is_token_valid_within_buffer() {
        // Token expires in 2 minutes - within 5 minute buffer, should be invalid
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let creds = ClaudeOAuthCredentials {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            expires_at: now_ms + 120_000, // 2 minutes from now
        };

        assert!(!is_token_valid(&creds));
    }

    #[test]
    fn test_keychain_data_deserialization() {
        let json = r#"{"claudeAiOauth": {"accessToken": "sk-ant-oat01-test", "refreshToken": "refresh-123", "expiresAt": 9999999999999}}"#;
        let data: KeychainData = serde_json::from_str(json).unwrap();
        let creds = data.claude_ai_oauth.unwrap();
        assert_eq!(creds.access_token, "sk-ant-oat01-test");
        assert_eq!(creds.refresh_token, "refresh-123");
        assert_eq!(creds.expires_at, 9999999999999);
    }

    #[test]
    fn test_keychain_data_missing_oauth() {
        let json = r#"{}"#;
        let data: KeychainData = serde_json::from_str(json).unwrap();
        assert!(data.claude_ai_oauth.is_none());
    }
}
