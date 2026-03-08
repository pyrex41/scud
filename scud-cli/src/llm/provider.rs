//! Multi-provider abstraction for the direct API agentic runner.
//!
//! Supports Anthropic Messages API and OpenAI-compatible Chat Completions APIs
//! (OpenAI, xAI, OpenRouter, OpenCode Zen). Handles endpoint routing, auth,
//! model normalization, and request/response format conversion.

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::oauth::{self, ApiCredential};
use super::tools::ToolDefinition;

const CLAUDE_CODE_VERSION: &str = "2.1.2";

// ─── Provider-agnostic types ────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AgentRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub enum AgentContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: AgentRole,
    pub content: Vec<AgentContentBlock>,
}

pub struct AgentResponse {
    pub content: Vec<AgentContentBlock>,
    pub done: bool,
}

// ─── Credential types ───────────────────────────────────────────────

pub enum ProviderCredential {
    Anthropic(ApiCredential),
    BearerToken(String),
}

// ─── Provider enum ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AgentProvider {
    Anthropic,
    OpenAI,
    Xai,
    OpenRouter,
    OpenCodeZen,
}

impl AgentProvider {
    pub fn from_provider_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAI),
            "xai" => Ok(Self::Xai),
            "openrouter" => Ok(Self::OpenRouter),
            "opencode-zen" | "opencode" | "zen" => Ok(Self::OpenCodeZen),
            _ => anyhow::bail!(
                "Unknown provider: '{}'. Supported: anthropic, openai, xai, openrouter, opencode-zen",
                s
            ),
        }
    }

    pub fn endpoint(&self) -> &str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
            Self::OpenAI => "https://api.openai.com/v1/chat/completions",
            Self::Xai => "https://api.x.ai/v1/chat/completions",
            Self::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            Self::OpenCodeZen => "https://api.opencode.ai/v1/chat/completions",
        }
    }

    pub fn resolve_credential(&self) -> Result<ProviderCredential> {
        match self {
            Self::Anthropic => {
                let cred = oauth::resolve_anthropic_credential()?;
                Ok(ProviderCredential::Anthropic(cred))
            }
            Self::OpenAI => {
                let key = std::env::var("OPENAI_API_KEY")
                    .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;
                Ok(ProviderCredential::BearerToken(key))
            }
            Self::Xai => {
                let key = std::env::var("XAI_API_KEY")
                    .map_err(|_| anyhow::anyhow!("XAI_API_KEY environment variable not set"))?;
                Ok(ProviderCredential::BearerToken(key))
            }
            Self::OpenRouter => {
                let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
                    anyhow::anyhow!("OPENROUTER_API_KEY environment variable not set")
                })?;
                Ok(ProviderCredential::BearerToken(key))
            }
            Self::OpenCodeZen => {
                let key = std::env::var("OPENCODE_API_KEY").map_err(|_| {
                    anyhow::anyhow!("OPENCODE_API_KEY environment variable not set")
                })?;
                Ok(ProviderCredential::BearerToken(key))
            }
        }
    }

    /// Strip provider prefix for native APIs, keep for aggregators.
    pub fn normalize_model<'a>(&self, model: &'a str) -> &'a str {
        match self {
            Self::Anthropic => model.strip_prefix("anthropic/").unwrap_or(model),
            Self::OpenAI => model.strip_prefix("openai/").unwrap_or(model),
            Self::Xai => model.strip_prefix("xai/").unwrap_or(model),
            // Aggregators keep prefix as-is (they need it for routing)
            Self::OpenRouter | Self::OpenCodeZen => model,
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::Anthropic => "claude-sonnet-4-5-20250929",
            Self::OpenAI => "o3-mini",
            Self::Xai => "grok-code-fast-1",
            Self::OpenRouter => "anthropic/claude-sonnet-4.5",
            Self::OpenCodeZen => "anthropic/claude-sonnet-4.5",
        }
    }

    /// Format tool definitions for this provider's wire format.
    pub fn format_tool_definitions(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        match self {
            Self::Anthropic => tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect(),
            _ => {
                // OpenAI-compatible format
                tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.input_schema,
                            }
                        })
                    })
                    .collect()
            }
        }
    }

    /// Send a request and parse the response into provider-agnostic types.
    pub async fn send_request(
        &self,
        client: &Client,
        credential: &ProviderCredential,
        model: &str,
        max_tokens: u32,
        system_prompt: Option<&str>,
        messages: &[AgentMessage],
        tools: &[Value],
    ) -> Result<AgentResponse> {
        match self {
            Self::Anthropic => {
                send_anthropic_request(
                    client,
                    credential,
                    model,
                    max_tokens,
                    system_prompt,
                    messages,
                    tools,
                )
                .await
            }
            _ => {
                send_openai_request(
                    client,
                    self,
                    credential,
                    model,
                    max_tokens,
                    system_prompt,
                    messages,
                    tools,
                )
                .await
            }
        }
    }
}

impl std::fmt::Display for AgentProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anthropic => write!(f, "Anthropic"),
            Self::OpenAI => write!(f, "OpenAI"),
            Self::Xai => write!(f, "xAI"),
            Self::OpenRouter => write!(f, "OpenRouter"),
            Self::OpenCodeZen => write!(f, "OpenCode Zen"),
        }
    }
}

// ─── Anthropic wire format ──────────────────────────────────────────

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

async fn send_anthropic_request(
    client: &Client,
    credential: &ProviderCredential,
    model: &str,
    max_tokens: u32,
    system_prompt: Option<&str>,
    messages: &[AgentMessage],
    tools: &[Value],
) -> Result<AgentResponse> {
    let anthropic_cred = match credential {
        ProviderCredential::Anthropic(c) => c,
        _ => anyhow::bail!("Anthropic provider requires Anthropic credential"),
    };

    // Build system blocks (OAuth identity prefix + custom prompt)
    let mut system_blocks: Vec<Value> = Vec::new();
    if matches!(anthropic_cred, ApiCredential::OAuth(_)) {
        system_blocks.push(json!({
            "type": "text",
            "text": "You are Claude Code, Anthropic's official CLI for Claude."
        }));
    }
    if let Some(prompt) = system_prompt {
        system_blocks.push(json!({"type": "text", "text": prompt}));
    }

    // Convert messages to Anthropic format
    let api_messages: Vec<Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                AgentRole::User => "user",
                AgentRole::Assistant => "assistant",
            };
            let content: Vec<Value> = m
                .content
                .iter()
                .map(|b| match b {
                    AgentContentBlock::Text { text } => json!({"type": "text", "text": text}),
                    AgentContentBlock::ToolUse { id, name, input } => json!({
                        "type": "tool_use", "id": id, "name": name, "input": input
                    }),
                    AgentContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let mut block = json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                        });
                        if *is_error {
                            block["is_error"] = json!(true);
                        }
                        block
                    }
                })
                .collect();
            json!({"role": role, "content": content})
        })
        .collect();

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system_blocks,
        "messages": api_messages,
        "tools": tools,
    });

    let mut req = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");

    match anthropic_cred {
        ApiCredential::OAuth(token) => {
            req = req
                .bearer_auth(token)
                .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
                .header(
                    "user-agent",
                    format!("claude-cli/{} (external, cli)", CLAUDE_CODE_VERSION),
                )
                .header("x-app", "cli");
        }
        ApiCredential::ApiKey(key) => {
            req = req.header("x-api-key", key);
        }
    }

    let response = req.json(&body).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error ({}): {}", status, text);
    }

    let resp: AnthropicResponse = response.json().await?;

    let mut has_tool_use = false;
    let content: Vec<AgentContentBlock> = resp
        .content
        .into_iter()
        .map(|b| match b {
            AnthropicContentBlock::Text { text } => AgentContentBlock::Text { text },
            AnthropicContentBlock::ToolUse { id, name, input } => {
                has_tool_use = true;
                AgentContentBlock::ToolUse { id, name, input }
            }
        })
        .collect();

    Ok(AgentResponse {
        content,
        done: !has_tool_use,
    })
}

// ─── OpenAI wire format ─────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenAICompletionResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIAssistantMsg,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIAssistantMsg {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Deserialize)]
struct OpenAIToolCall {
    id: String,
    function: OpenAIToolCallFunction,
}

#[derive(Deserialize)]
struct OpenAIToolCallFunction {
    name: String,
    arguments: String,
}

/// Convert provider-agnostic messages to OpenAI Chat Completions format.
fn convert_messages_to_openai(
    system_prompt: Option<&str>,
    messages: &[AgentMessage],
) -> Vec<Value> {
    let mut out = Vec::new();

    if let Some(prompt) = system_prompt {
        out.push(json!({"role": "system", "content": prompt}));
    }

    for msg in messages {
        match msg.role {
            AgentRole::User => {
                let has_tool_results = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, AgentContentBlock::ToolResult { .. }));

                if has_tool_results {
                    // Each tool result becomes a separate "tool" message
                    for block in &msg.content {
                        if let AgentContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } = block
                        {
                            out.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                    }
                } else {
                    // Regular user message - concatenate text blocks
                    let text: String = msg
                        .content
                        .iter()
                        .filter_map(|b| {
                            if let AgentContentBlock::Text { text } = b {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push(json!({"role": "user", "content": text}));
                }
            }
            AgentRole::Assistant => {
                let mut text_parts: Vec<&str> = Vec::new();
                let mut tool_calls: Vec<Value> = Vec::new();

                for block in &msg.content {
                    match block {
                        AgentContentBlock::Text { text } => text_parts.push(text),
                        AgentContentBlock::ToolUse { id, name, input } => {
                            tool_calls.push(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": serde_json::to_string(input).unwrap_or_default(),
                                }
                            }));
                        }
                        _ => {}
                    }
                }

                let mut msg_json = json!({"role": "assistant"});
                if text_parts.is_empty() {
                    msg_json["content"] = Value::Null;
                } else {
                    msg_json["content"] = json!(text_parts.join(""));
                }
                if !tool_calls.is_empty() {
                    msg_json["tool_calls"] = json!(tool_calls);
                }
                out.push(msg_json);
            }
        }
    }

    out
}

async fn send_openai_request(
    client: &Client,
    provider: &AgentProvider,
    credential: &ProviderCredential,
    model: &str,
    max_tokens: u32,
    system_prompt: Option<&str>,
    messages: &[AgentMessage],
    tools: &[Value],
) -> Result<AgentResponse> {
    let token = match credential {
        ProviderCredential::BearerToken(t) => t,
        _ => anyhow::bail!("OpenAI-compatible provider requires bearer token credential"),
    };

    let openai_messages = convert_messages_to_openai(system_prompt, messages);

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": openai_messages,
        "tools": tools,
    });

    let mut req = client
        .post(provider.endpoint())
        .header("content-type", "application/json")
        .bearer_auth(token);

    // OpenRouter extra headers
    if *provider == AgentProvider::OpenRouter {
        req = req
            .header("HTTP-Referer", "https://github.com/scud-cli")
            .header("X-Title", "SCUD Task Master");
    }

    let response = req.json(&body).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("{} API error ({}): {}", provider, status, text);
    }

    let resp: OpenAICompletionResponse = response.json().await?;

    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No choices in {} API response", provider))?;

    let mut content = Vec::new();
    let mut has_tool_use = false;

    if let Some(text) = choice.message.content {
        if !text.is_empty() {
            content.push(AgentContentBlock::Text { text });
        }
    }

    if let Some(tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            has_tool_use = true;
            let input: Value =
                serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
            content.push(AgentContentBlock::ToolUse {
                id: tc.id,
                name: tc.function.name,
                input,
            });
        }
    }

    Ok(AgentResponse {
        content,
        done: !has_tool_use,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_provider_str() {
        assert_eq!(
            AgentProvider::from_provider_str("anthropic").unwrap(),
            AgentProvider::Anthropic
        );
        assert_eq!(
            AgentProvider::from_provider_str("openai").unwrap(),
            AgentProvider::OpenAI
        );
        assert_eq!(
            AgentProvider::from_provider_str("xai").unwrap(),
            AgentProvider::Xai
        );
        assert_eq!(
            AgentProvider::from_provider_str("openrouter").unwrap(),
            AgentProvider::OpenRouter
        );
        assert_eq!(
            AgentProvider::from_provider_str("opencode-zen").unwrap(),
            AgentProvider::OpenCodeZen
        );
        assert_eq!(
            AgentProvider::from_provider_str("opencode").unwrap(),
            AgentProvider::OpenCodeZen
        );
        assert_eq!(
            AgentProvider::from_provider_str("zen").unwrap(),
            AgentProvider::OpenCodeZen
        );
        // Case insensitive
        assert_eq!(
            AgentProvider::from_provider_str("ANTHROPIC").unwrap(),
            AgentProvider::Anthropic
        );
        assert_eq!(
            AgentProvider::from_provider_str("Xai").unwrap(),
            AgentProvider::Xai
        );
        // Unknown
        assert!(AgentProvider::from_provider_str("unknown").is_err());
    }

    #[test]
    fn test_normalize_model() {
        // Native APIs strip prefix
        assert_eq!(
            AgentProvider::Anthropic.normalize_model("anthropic/claude-sonnet-4.5"),
            "claude-sonnet-4.5"
        );
        assert_eq!(
            AgentProvider::Anthropic.normalize_model("claude-sonnet-4.5"),
            "claude-sonnet-4.5"
        );
        assert_eq!(
            AgentProvider::Xai.normalize_model("xai/grok-code-fast-1"),
            "grok-code-fast-1"
        );
        assert_eq!(
            AgentProvider::Xai.normalize_model("grok-code-fast-1"),
            "grok-code-fast-1"
        );
        assert_eq!(
            AgentProvider::OpenAI.normalize_model("openai/gpt-4"),
            "gpt-4"
        );

        // Aggregators keep prefix
        assert_eq!(
            AgentProvider::OpenRouter.normalize_model("anthropic/claude-sonnet-4.5"),
            "anthropic/claude-sonnet-4.5"
        );
        assert_eq!(
            AgentProvider::OpenCodeZen.normalize_model("anthropic/claude-sonnet-4.5"),
            "anthropic/claude-sonnet-4.5"
        );
    }

    #[test]
    fn test_format_tool_definitions_anthropic() {
        let tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }];

        let formatted = AgentProvider::Anthropic.format_tool_definitions(&tools);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["name"], "Read");
        assert_eq!(formatted[0]["description"], "Read a file");
        assert!(formatted[0].get("input_schema").is_some());
        // Should NOT have function wrapper
        assert!(formatted[0].get("type").is_none());
        assert!(formatted[0].get("function").is_none());
    }

    #[test]
    fn test_format_tool_definitions_openai() {
        let tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }];

        for provider in [
            AgentProvider::OpenAI,
            AgentProvider::Xai,
            AgentProvider::OpenRouter,
            AgentProvider::OpenCodeZen,
        ] {
            let formatted = provider.format_tool_definitions(&tools);
            assert_eq!(formatted.len(), 1);
            assert_eq!(formatted[0]["type"], "function");
            assert_eq!(formatted[0]["function"]["name"], "Read");
            assert_eq!(formatted[0]["function"]["description"], "Read a file");
            assert!(formatted[0]["function"]["parameters"].is_object());
            // Should NOT have Anthropic-style fields at top level
            assert!(formatted[0].get("name").is_none());
            assert!(formatted[0].get("input_schema").is_none());
        }
    }

    #[test]
    fn test_convert_messages_to_openai_basic() {
        let messages = vec![AgentMessage {
            role: AgentRole::User,
            content: vec![AgentContentBlock::Text {
                text: "Hello".to_string(),
            }],
        }];

        let openai_msgs = convert_messages_to_openai(Some("System prompt"), &messages);
        assert_eq!(openai_msgs.len(), 2);
        assert_eq!(openai_msgs[0]["role"], "system");
        assert_eq!(openai_msgs[0]["content"], "System prompt");
        assert_eq!(openai_msgs[1]["role"], "user");
        assert_eq!(openai_msgs[1]["content"], "Hello");
    }

    #[test]
    fn test_convert_messages_to_openai_no_system() {
        let messages = vec![AgentMessage {
            role: AgentRole::User,
            content: vec![AgentContentBlock::Text {
                text: "Hi".to_string(),
            }],
        }];

        let openai_msgs = convert_messages_to_openai(None, &messages);
        assert_eq!(openai_msgs.len(), 1);
        assert_eq!(openai_msgs[0]["role"], "user");
    }

    #[test]
    fn test_convert_messages_to_openai_tool_use_cycle() {
        let messages = vec![
            AgentMessage {
                role: AgentRole::User,
                content: vec![AgentContentBlock::Text {
                    text: "Read file".to_string(),
                }],
            },
            AgentMessage {
                role: AgentRole::Assistant,
                content: vec![
                    AgentContentBlock::Text {
                        text: "I'll read that.".to_string(),
                    },
                    AgentContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "Read".to_string(),
                        input: json!({"file_path": "/src/main.rs"}),
                    },
                ],
            },
            AgentMessage {
                role: AgentRole::User,
                content: vec![AgentContentBlock::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: "fn main() {}".to_string(),
                    is_error: false,
                }],
            },
        ];

        let openai_msgs = convert_messages_to_openai(None, &messages);
        assert_eq!(openai_msgs.len(), 3);

        // User message
        assert_eq!(openai_msgs[0]["role"], "user");
        assert_eq!(openai_msgs[0]["content"], "Read file");

        // Assistant with tool calls
        assert_eq!(openai_msgs[1]["role"], "assistant");
        assert_eq!(openai_msgs[1]["content"], "I'll read that.");
        let tool_calls = openai_msgs[1]["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_1");
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "Read");
        // arguments should be a JSON string
        let args: Value =
            serde_json::from_str(tool_calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args["file_path"], "/src/main.rs");

        // Tool result
        assert_eq!(openai_msgs[2]["role"], "tool");
        assert_eq!(openai_msgs[2]["tool_call_id"], "call_1");
        assert_eq!(openai_msgs[2]["content"], "fn main() {}");
    }

    #[test]
    fn test_convert_messages_to_openai_tool_only_assistant() {
        // Assistant message with only tool calls (no text)
        let messages = vec![AgentMessage {
            role: AgentRole::Assistant,
            content: vec![AgentContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "Bash".to_string(),
                input: json!({"command": "ls"}),
            }],
        }];

        let openai_msgs = convert_messages_to_openai(None, &messages);
        assert_eq!(openai_msgs.len(), 1);
        assert_eq!(openai_msgs[0]["role"], "assistant");
        assert!(openai_msgs[0]["content"].is_null());
        assert!(openai_msgs[0]["tool_calls"].is_array());
    }

    #[test]
    fn test_convert_messages_to_openai_multiple_tool_results() {
        // Multiple tool results expand into separate messages
        let messages = vec![AgentMessage {
            role: AgentRole::User,
            content: vec![
                AgentContentBlock::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: "result 1".to_string(),
                    is_error: false,
                },
                AgentContentBlock::ToolResult {
                    tool_use_id: "call_2".to_string(),
                    content: "result 2".to_string(),
                    is_error: true,
                },
            ],
        }];

        let openai_msgs = convert_messages_to_openai(None, &messages);
        assert_eq!(openai_msgs.len(), 2);
        assert_eq!(openai_msgs[0]["role"], "tool");
        assert_eq!(openai_msgs[0]["tool_call_id"], "call_1");
        assert_eq!(openai_msgs[1]["role"], "tool");
        assert_eq!(openai_msgs[1]["tool_call_id"], "call_2");
    }

    #[test]
    fn test_endpoint() {
        assert_eq!(
            AgentProvider::Anthropic.endpoint(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            AgentProvider::OpenAI.endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            AgentProvider::Xai.endpoint(),
            "https://api.x.ai/v1/chat/completions"
        );
        assert_eq!(
            AgentProvider::OpenRouter.endpoint(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(
            AgentProvider::OpenCodeZen.endpoint(),
            "https://api.opencode.ai/v1/chat/completions"
        );
    }

    #[test]
    fn test_default_model() {
        assert!(AgentProvider::Anthropic.default_model().contains("claude"));
        assert!(AgentProvider::Xai.default_model().contains("grok"));
        assert!(AgentProvider::OpenAI.default_model().contains("o3"));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", AgentProvider::Anthropic), "Anthropic");
        assert_eq!(format!("{}", AgentProvider::Xai), "xAI");
        assert_eq!(format!("{}", AgentProvider::OpenCodeZen), "OpenCode Zen");
    }
}
