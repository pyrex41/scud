//! AI-Powered Command Tests (US-10 to US-14)
//!
//! Tests for AI-powered features:
//! - US-10: PRD to Tasks Conversion (parse)
//! - US-11: Task Complexity Analysis (analyze-complexity)
//! - US-12: Automatic Task Expansion (expand)
//! - US-13: Full Pipeline Generation (generate)
//! - US-14: Cross-Phase Dependency Analysis (reanalyze-deps)
//!
//! These tests use MockLLMClient by default. Run with `--features real-llm`
//! to test with actual API calls.

use crate::e2e::fixtures::{TestProject, SAMPLE_PRD};
use crate::e2e::mock_llm::{
    sample_complexity_response, sample_expand_response, sample_parse_response, MockLLMClient,
    MockLLMResponse,
};

// US-10: PRD to Tasks Conversion

#[test]
fn test_us10_mock_parse_response_valid_json() {
    let response = sample_parse_response();

    // Verify it's valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Sample parse response should be valid JSON");

    // Should be an array
    assert!(parsed.is_array());
    let tasks = parsed.as_array().unwrap();
    assert!(!tasks.is_empty());

    // Each task should have required fields
    for task in tasks {
        assert!(task.get("id").is_some());
        assert!(task.get("title").is_some());
        assert!(task.get("description").is_some());
    }
}

#[test]
fn test_us10_mock_client_returns_parse_response() {
    let client = MockLLMClient::new().with_parse_response(&sample_parse_response());

    let result = client.call("Please parse this PRD document", None);
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.contains("User Registration"));
    assert!(response.contains("Login System"));
}

#[test]
fn test_us10_prd_fixture_content() {
    // Verify sample PRD has expected sections
    assert!(SAMPLE_PRD.contains("Product Requirements Document"));
    assert!(SAMPLE_PRD.contains("User Registration"));
    assert!(SAMPLE_PRD.contains("Login System"));
    assert!(SAMPLE_PRD.contains("Password Reset"));
}

// US-11: Task Complexity Analysis

#[test]
fn test_us11_mock_complexity_response_valid_json() {
    let response = sample_complexity_response();

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Sample complexity response should be valid JSON");

    assert!(parsed.is_array());
    let analyses = parsed.as_array().unwrap();

    for analysis in analyses {
        assert!(analysis.get("id").is_some());
        assert!(analysis.get("complexity").is_some());
        assert!(analysis.get("reason").is_some());
    }
}

#[test]
fn test_us11_mock_client_returns_complexity_response() {
    let client = MockLLMClient::new().with_complexity_response(&sample_complexity_response());

    let result = client.call("Analyze complexity of these tasks", None);
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.contains("complexity"));
}

#[test]
fn test_us11_complexity_project_has_scores() {
    let project = TestProject::complexity_project();
    let phase = project.storage.load_active_group().unwrap();

    // Verify tasks have complexity scores
    let t1 = phase.get_task("1").unwrap();
    let t2 = phase.get_task("2").unwrap();
    let t3 = phase.get_task("3").unwrap();

    assert_eq!(t1.complexity, 3); // Simple
    assert_eq!(t2.complexity, 8); // Medium
    assert_eq!(t3.complexity, 13); // Complex
}

// US-12: Automatic Task Expansion

#[test]
fn test_us12_mock_expand_response_valid_json() {
    let response = sample_expand_response();

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Sample expand response should be valid JSON");

    assert!(parsed.is_array());
    let subtasks = parsed.as_array().unwrap();

    // Should have subtasks with hierarchical IDs
    for subtask in subtasks {
        let id = subtask.get("id").unwrap().as_str().unwrap();
        assert!(
            id.contains('.'),
            "Subtask IDs should be hierarchical (e.g., 1.1)"
        );
    }
}

#[test]
fn test_us12_mock_client_returns_expand_response() {
    let client = MockLLMClient::new().with_expand_response(&sample_expand_response());

    let result = client.call("Expand this task into subtasks", None);
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.contains("1.1"));
    assert!(response.contains("1.2"));
}

#[test]
fn test_us12_high_complexity_tasks_identified() {
    let project = TestProject::complexity_project();
    let phase = project.storage.load_active_group().unwrap();

    // Find tasks with complexity >= 8 (should be expanded)
    let complex_tasks: Vec<_> = phase.tasks.iter().filter(|t| t.complexity >= 8).collect();

    assert_eq!(complex_tasks.len(), 2); // Tasks 2 and 3
}

// US-13: Full Pipeline Generation

#[test]
fn test_us13_mock_pipeline_all_responses_configured() {
    let client = MockLLMClient::new()
        .with_parse_response(&sample_parse_response())
        .with_complexity_response(&sample_complexity_response())
        .with_expand_response(&sample_expand_response());

    // Each stage should work
    assert!(client.call("parse PRD", None).is_ok());
    assert!(client.call("analyze complexity", None).is_ok());
    assert!(client.call("expand task", None).is_ok());
}

#[test]
fn test_us13_mock_client_records_pipeline_calls() {
    let client = MockLLMClient::new()
        .with_parse_response(&sample_parse_response())
        .with_complexity_response(&sample_complexity_response())
        .with_expand_response(&sample_expand_response());

    // Simulate pipeline
    client.call("parse this PRD", Some("gpt-4")).unwrap();
    client.call("analyze complexity", Some("gpt-4")).unwrap();
    client.call("expand task 1", Some("gpt-4")).unwrap();
    client.call("expand task 2", Some("gpt-4")).unwrap();

    // Verify calls recorded
    assert_eq!(client.call_count(), 4);

    let parse_calls = client.calls_matching("parse");
    assert_eq!(parse_calls.len(), 1);

    let expand_calls = client.calls_matching("expand");
    assert_eq!(expand_calls.len(), 2);
}

// US-14: Cross-Phase Dependency Analysis

#[test]
fn test_us14_multi_phase_deps_structure() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    // Find all cross-phase dependencies
    let mut cross_deps: Vec<String> = Vec::new();
    for (_name, phase) in &tasks {
        for task in &phase.tasks {
            for dep in &task.dependencies {
                if dep.contains(':') {
                    cross_deps.push(dep.clone());
                }
            }
        }
    }

    assert!(
        !cross_deps.is_empty(),
        "Should have cross-phase dependencies"
    );
}

#[test]
fn test_us14_cross_phase_dep_format() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    let phase2 = tasks.get("phase2").unwrap();
    let task = phase2.get_task("1").unwrap();

    // Verify format is "phase:task_id"
    let cross_dep = task
        .dependencies
        .iter()
        .find(|d| d.contains(':'))
        .expect("Should have a cross-phase dependency");

    let parts: Vec<_> = cross_dep.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "phase1"); // Source phase
    assert_eq!(parts[1], "1"); // Source task ID
}

// Error handling tests

#[test]
fn test_mock_client_error_handling() {
    let client = MockLLMClient::new().with_default_response(MockLLMResponse::Error(
        "API rate limit exceeded".to_string(),
    ));

    let result = client.call("any prompt", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("rate limit"));
}

#[test]
fn test_mock_client_timeout_handling() {
    let client = MockLLMClient::new().with_default_response(MockLLMResponse::Timeout);

    let result = client.call("any prompt", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("timed out"));
}

#[test]
fn test_mock_client_pattern_matching_order() {
    // Note: HashMap iteration order is not guaranteed, so specific pattern
    // matching happens based on which pattern matches first during iteration.
    // This test verifies that pattern matching works, not priority.
    let client = MockLLMClient::new()
        .when_prompt_contains(
            "specific_unique",
            MockLLMResponse::Success("Specific".to_string()),
        )
        .when_prompt_contains("general", MockLLMResponse::Success("General".to_string()));

    // Each pattern should match its specific prompt
    assert_eq!(
        client.call("specific_unique query", None).unwrap(),
        "Specific"
    );
    assert_eq!(client.call("general query", None).unwrap(), "General");
}
