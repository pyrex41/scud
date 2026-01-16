//! Mock LLM client for AI command testing
//!
//! Provides `MockLLMClient` for testing AI-powered commands without making
//! actual API calls. When the `real-llm` feature is enabled, tests can
//! optionally use real API calls for integration testing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents a call made to the mock LLM
#[derive(Debug, Clone)]
pub struct MockLLMCall {
    pub prompt: String,
    pub model: Option<String>,
    pub timestamp: std::time::Instant,
}

/// Mock response types for different AI operations
#[derive(Debug, Clone)]
pub enum MockLLMResponse {
    /// Successful response with content
    Success(String),
    /// Simulated error
    Error(String),
    /// Timeout simulation
    Timeout,
}

/// Mock LLM client that records calls and returns configured responses
#[derive(Clone)]
pub struct MockLLMClient {
    /// Responses keyed by prompt patterns (substring matching)
    responses: Arc<Mutex<HashMap<String, MockLLMResponse>>>,
    /// Default response when no pattern matches
    default_response: Arc<Mutex<MockLLMResponse>>,
    /// Log of all calls made
    calls: Arc<Mutex<Vec<MockLLMCall>>>,
}

impl MockLLMClient {
    /// Create a new mock client with a default success response
    pub fn new() -> Self {
        MockLLMClient {
            responses: Arc::new(Mutex::new(HashMap::new())),
            default_response: Arc::new(Mutex::new(MockLLMResponse::Success(
                "Mock response".to_string(),
            ))),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set the default response for unmatched prompts
    pub fn with_default_response(self, response: MockLLMResponse) -> Self {
        *self.default_response.lock().unwrap() = response;
        self
    }

    /// Configure a response for prompts containing the given pattern
    pub fn when_prompt_contains(self, pattern: &str, response: MockLLMResponse) -> Self {
        self.responses
            .lock()
            .unwrap()
            .insert(pattern.to_string(), response);
        self
    }

    /// Configure response for PRD parsing
    /// Returns a JSON array of tasks
    pub fn with_parse_response(self, tasks_json: &str) -> Self {
        self.when_prompt_contains("parse", MockLLMResponse::Success(tasks_json.to_string()))
    }

    /// Configure response for complexity analysis
    pub fn with_complexity_response(self, analysis_json: &str) -> Self {
        self.when_prompt_contains(
            "complexity",
            MockLLMResponse::Success(analysis_json.to_string()),
        )
    }

    /// Configure response for task expansion
    pub fn with_expand_response(self, subtasks_json: &str) -> Self {
        self.when_prompt_contains("expand", MockLLMResponse::Success(subtasks_json.to_string()))
    }

    /// Configure response for dependency analysis
    pub fn with_deps_response(self, deps_json: &str) -> Self {
        self.when_prompt_contains(
            "dependencies",
            MockLLMResponse::Success(deps_json.to_string()),
        )
    }

    /// Simulate an LLM call
    pub fn call(&self, prompt: &str, model: Option<&str>) -> Result<String, String> {
        // Record the call
        self.calls.lock().unwrap().push(MockLLMCall {
            prompt: prompt.to_string(),
            model: model.map(String::from),
            timestamp: std::time::Instant::now(),
        });

        // Find matching response
        let responses = self.responses.lock().unwrap();
        for (pattern, response) in responses.iter() {
            if prompt.to_lowercase().contains(&pattern.to_lowercase()) {
                return match response {
                    MockLLMResponse::Success(content) => Ok(content.clone()),
                    MockLLMResponse::Error(msg) => Err(msg.clone()),
                    MockLLMResponse::Timeout => Err("Request timed out".to_string()),
                };
            }
        }

        // Use default response
        match &*self.default_response.lock().unwrap() {
            MockLLMResponse::Success(content) => Ok(content.clone()),
            MockLLMResponse::Error(msg) => Err(msg.clone()),
            MockLLMResponse::Timeout => Err("Request timed out".to_string()),
        }
    }

    /// Get all recorded calls
    pub fn calls(&self) -> Vec<MockLLMCall> {
        self.calls.lock().unwrap().clone()
    }

    /// Get calls matching a prompt pattern
    pub fn calls_matching(&self, pattern: &str) -> Vec<MockLLMCall> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.prompt.to_lowercase().contains(&pattern.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Clear recorded calls
    pub fn clear_calls(&self) {
        self.calls.lock().unwrap().clear();
    }

    /// Check if any call was made
    pub fn was_called(&self) -> bool {
        !self.calls.lock().unwrap().is_empty()
    }

    /// Get the number of calls made
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Sample task list JSON response for parse tests
pub fn sample_parse_response() -> String {
    r#"[
        {
            "id": "1",
            "title": "User Registration",
            "description": "Implement user registration with email and password",
            "complexity": 5,
            "dependencies": []
        },
        {
            "id": "2",
            "title": "Login System",
            "description": "Implement login with session management",
            "complexity": 8,
            "dependencies": ["1"]
        },
        {
            "id": "3",
            "title": "Password Reset",
            "description": "Email-based password reset flow",
            "complexity": 5,
            "dependencies": ["1"]
        }
    ]"#
    .to_string()
}

/// Sample complexity analysis JSON response
pub fn sample_complexity_response() -> String {
    r#"[
        { "id": "1", "complexity": 5, "reason": "Standard registration flow" },
        { "id": "2", "complexity": 8, "reason": "Session management adds complexity" },
        { "id": "3", "complexity": 5, "reason": "Token handling with email integration" }
    ]"#
    .to_string()
}

/// Sample expansion JSON response for subtasks
pub fn sample_expand_response() -> String {
    r#"[
        {
            "id": "1.1",
            "title": "Create registration form",
            "description": "Build the UI form for user registration",
            "complexity": 3,
            "dependencies": []
        },
        {
            "id": "1.2",
            "title": "Validate input",
            "description": "Add email and password validation",
            "complexity": 2,
            "dependencies": ["1.1"]
        },
        {
            "id": "1.3",
            "title": "Store user in database",
            "description": "Save validated user data",
            "complexity": 2,
            "dependencies": ["1.2"]
        }
    ]"#
    .to_string()
}

/// Sample dependency analysis JSON response
pub fn sample_deps_response() -> String {
    r#"{
        "suggested_dependencies": [
            { "from": "phase2:1", "to": "phase1:3", "reason": "API needs auth complete" }
        ],
        "circular_risks": []
    }"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_client_default_response() {
        let client = MockLLMClient::new();
        let result = client.call("test prompt", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Mock response");
    }

    #[test]
    fn test_mock_client_custom_default() {
        let client = MockLLMClient::new()
            .with_default_response(MockLLMResponse::Success("Custom default".to_string()));
        let result = client.call("anything", None);
        assert_eq!(result.unwrap(), "Custom default");
    }

    #[test]
    fn test_mock_client_pattern_matching() {
        let client = MockLLMClient::new()
            .when_prompt_contains("parse", MockLLMResponse::Success("Parsed!".to_string()))
            .when_prompt_contains("expand", MockLLMResponse::Success("Expanded!".to_string()));

        assert_eq!(
            client.call("Please parse this PRD", None).unwrap(),
            "Parsed!"
        );
        assert_eq!(
            client.call("Expand this task", None).unwrap(),
            "Expanded!"
        );
        assert_eq!(
            client.call("Something else", None).unwrap(),
            "Mock response"
        );
    }

    #[test]
    fn test_mock_client_error_response() {
        let client = MockLLMClient::new()
            .with_default_response(MockLLMResponse::Error("API error".to_string()));

        let result = client.call("test", None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "API error");
    }

    #[test]
    fn test_mock_client_timeout_response() {
        let client =
            MockLLMClient::new().with_default_response(MockLLMResponse::Timeout);

        let result = client.call("test", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[test]
    fn test_mock_client_records_calls() {
        let client = MockLLMClient::new();

        client.call("first prompt", Some("gpt-4")).unwrap();
        client.call("second prompt", None).unwrap();

        let calls = client.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].prompt, "first prompt");
        assert_eq!(calls[0].model, Some("gpt-4".to_string()));
        assert_eq!(calls[1].prompt, "second prompt");
        assert_eq!(calls[1].model, None);
    }

    #[test]
    fn test_mock_client_calls_matching() {
        let client = MockLLMClient::new();

        client.call("parse this PRD", None).unwrap();
        client.call("expand task 1", None).unwrap();
        client.call("parse another PRD", None).unwrap();

        let parse_calls = client.calls_matching("parse");
        assert_eq!(parse_calls.len(), 2);
    }

    #[test]
    fn test_mock_client_clear_calls() {
        let client = MockLLMClient::new();

        client.call("test", None).unwrap();
        assert!(client.was_called());

        client.clear_calls();
        assert!(!client.was_called());
    }

    #[test]
    fn test_parse_response_helper() {
        let client = MockLLMClient::new().with_parse_response(&sample_parse_response());

        let result = client.call("parse this document", None).unwrap();
        assert!(result.contains("User Registration"));
        assert!(result.contains("Login System"));
    }

    #[test]
    fn test_expand_response_helper() {
        let client = MockLLMClient::new().with_expand_response(&sample_expand_response());

        let result = client.call("expand this task", None).unwrap();
        assert!(result.contains("Create registration form"));
        assert!(result.contains("1.1"));
    }

    #[test]
    fn test_case_insensitive_matching() {
        let client = MockLLMClient::new()
            .when_prompt_contains("PARSE", MockLLMResponse::Success("Found!".to_string()));

        assert_eq!(client.call("parse this", None).unwrap(), "Found!");
        assert_eq!(client.call("PARSE THIS", None).unwrap(), "Found!");
        assert_eq!(client.call("Parse This", None).unwrap(), "Found!");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let client = Arc::new(MockLLMClient::new());
        let mut handles = vec![];

        for i in 0..10 {
            let c = Arc::clone(&client);
            let handle = thread::spawn(move || {
                c.call(&format!("prompt {}", i), None).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(client.call_count(), 10);
    }
}
