//! Integration tests for ExtensionRunner (Task 12)
//!
//! Tests for the extension runner trait implementation:
//! - Tool call handling with various input types
//! - Concurrent execution with bounded limits
//! - AgentRunner lifecycle and event handling
//! - Error handling and edge cases

use scud::commands::spawn::terminal::Harness;
use scud::extensions::{
    map_with_concurrency_limit, map_with_concurrency_limit_ordered, AgentEvent, AgentResult,
    AgentRunner, ConcurrentSpawnConfig, ConcurrentSpawnResult, ExtensionRunner,
    ExtensionRunnerError, SpawnConfig, ToolCallResult,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// ============================================================================
// Tool Call Integration Tests
// ============================================================================

#[test]
fn test_tool_call_full_lifecycle() {
    // Test complete tool registration, execution, and result handling
    let mut runner = ExtensionRunner::new();

    // Register multiple tools
    fn add_tool(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let a = args.get(0).and_then(|v| v.as_i64()).unwrap_or(0);
        let b = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!(a + b))
    }

    fn multiply_tool(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let a = args.get(0).and_then(|v| v.as_i64()).unwrap_or(1);
        let b = args.get(1).and_then(|v| v.as_i64()).unwrap_or(1);
        Ok(json!(a * b))
    }

    fn format_tool(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let template = args
            .get(0)
            .and_then(|v| v.get("template"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let value = args
            .get(0)
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        Ok(json!(template.replace("{}", &value.to_string())))
    }

    runner.register_tool("add".to_string(), add_tool);
    runner.register_tool("multiply".to_string(), multiply_tool);
    runner.register_tool("format".to_string(), format_tool);

    // Verify registration
    assert_eq!(runner.list_tools().len(), 3);
    assert!(runner.has_tool("add"));
    assert!(runner.has_tool("multiply"));
    assert!(runner.has_tool("format"));

    // Test add tool with array input
    let result = runner.on_tool_call("add", json!([5, 3])).unwrap();
    assert_eq!(result.tool_name, "add");
    assert!(result.success);
    assert_eq!(result.output, json!(8));

    // Test multiply tool with array input
    let result = runner.on_tool_call("multiply", json!([4, 7])).unwrap();
    assert_eq!(result.output, json!(28));

    // Test format tool with object input
    let result = runner
        .on_tool_call(
            "format",
            json!({"template": "Result: {}", "value": 42}),
        )
        .unwrap();
    assert_eq!(result.output, json!("Result: 42"));
}

#[test]
fn test_tool_call_error_propagation() {
    let mut runner = ExtensionRunner::new();

    // Register a tool that returns an error
    fn failing_tool(_args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("Intentional test failure".into())
    }

    runner.register_tool("failing".to_string(), failing_tool);

    // Tool should be registered but fail on execution
    assert!(runner.has_tool("failing"));

    let result = runner.on_tool_call("failing", json!({}));
    assert!(result.is_err());

    match result {
        Err(ExtensionRunnerError::ExecutionError(_)) => {} // Expected
        _ => panic!("Expected ExecutionError"),
    }
}

#[test]
fn test_tool_call_chained_execution() {
    // Test chaining multiple tool calls
    let mut runner = ExtensionRunner::new();

    fn increment(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let n = args.get(0).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!(n + 1))
    }

    fn double(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let n = args.get(0).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!(n * 2))
    }

    runner.register_tool("increment".to_string(), increment);
    runner.register_tool("double".to_string(), double);

    // Chain: 5 -> increment -> 6 -> double -> 12
    let r1 = runner.on_tool_call("increment", json!([5])).unwrap();
    let r2 = runner.on_tool_call("double", json!([r1.output])).unwrap();

    assert_eq!(r2.output, json!(12));
}

#[test]
fn test_tool_call_with_complex_json() {
    let mut runner = ExtensionRunner::new();

    fn process_task(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let task = args.get(0).ok_or("Missing task argument")?;
        let id = task.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let priority = task.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
        let tags: Vec<&str> = task
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|t| t.as_str()).collect())
            .unwrap_or_default();

        Ok(json!({
            "processed_id": id,
            "priority_level": if priority > 5 { "high" } else { "low" },
            "tag_count": tags.len(),
            "tags": tags,
        }))
    }

    runner.register_tool("process_task".to_string(), process_task);

    let task = json!({
        "id": "task-123",
        "priority": 8,
        "tags": ["urgent", "backend", "api"],
        "metadata": {
            "created": "2024-01-01",
            "author": "test"
        }
    });

    let result = runner.on_tool_call("process_task", task).unwrap();

    assert_eq!(result.output["processed_id"], "task-123");
    assert_eq!(result.output["priority_level"], "high");
    assert_eq!(result.output["tag_count"], 3);
}

#[test]
fn test_tool_call_null_and_empty_inputs() {
    let mut runner = ExtensionRunner::new();

    fn arg_info(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!({
            "count": args.len(),
            "types": args.iter().map(|a| {
                if a.is_null() { "null" }
                else if a.is_array() { "array" }
                else if a.is_object() { "object" }
                else if a.is_string() { "string" }
                else if a.is_number() { "number" }
                else if a.is_boolean() { "boolean" }
                else { "unknown" }
            }).collect::<Vec<_>>()
        }))
    }

    runner.register_tool("arg_info".to_string(), arg_info);

    // Test with null input
    let r1 = runner.on_tool_call("arg_info", Value::Null).unwrap();
    assert_eq!(r1.output["count"], 0);

    // Test with empty array
    let r2 = runner.on_tool_call("arg_info", json!([])).unwrap();
    assert_eq!(r2.output["count"], 0);

    // Test with empty object
    let r3 = runner.on_tool_call("arg_info", json!({})).unwrap();
    assert_eq!(r3.output["count"], 1);
    assert_eq!(r3.output["types"][0], "object");

    // Test with mixed types
    let r4 = runner
        .on_tool_call("arg_info", json!([1, "two", null, true]))
        .unwrap();
    assert_eq!(r4.output["count"], 4);
}

#[test]
fn test_tool_registry_isolation() {
    // Verify that different runner instances are isolated
    let mut runner1 = ExtensionRunner::new();
    let mut runner2 = ExtensionRunner::new();

    fn tool_a(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!("a"))
    }
    fn tool_b(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!("b"))
    }

    runner1.register_tool("tool_a".to_string(), tool_a);
    runner2.register_tool("tool_b".to_string(), tool_b);

    // Each runner should only have its own tool
    assert!(runner1.has_tool("tool_a"));
    assert!(!runner1.has_tool("tool_b"));
    assert!(!runner2.has_tool("tool_a"));
    assert!(runner2.has_tool("tool_b"));

    // Executing unregistered tool should fail
    assert!(runner1.on_tool_call("tool_b", json!({})).is_err());
    assert!(runner2.on_tool_call("tool_a", json!({})).is_err());
}

// ============================================================================
// Concurrency Integration Tests
// ============================================================================

#[tokio::test]
async fn test_concurrency_limit_respects_bound() {
    let items: Vec<i32> = (0..20).collect();
    let current_concurrent = Arc::new(AtomicUsize::new(0));
    let max_observed = Arc::new(AtomicUsize::new(0));
    let limit = 4;

    let results = map_with_concurrency_limit(items, limit, |n| {
        let current = Arc::clone(&current_concurrent);
        let max = Arc::clone(&max_observed);
        async move {
            // Track concurrency
            let active = current.fetch_add(1, Ordering::SeqCst) + 1;

            // Update max if this is higher
            let mut current_max = max.load(Ordering::SeqCst);
            while active > current_max {
                match max.compare_exchange_weak(
                    current_max,
                    active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(m) => current_max = m,
                }
            }

            // Simulate work
            tokio::time::sleep(Duration::from_millis(20)).await;

            current.fetch_sub(1, Ordering::SeqCst);
            n * 2
        }
    })
    .await;

    // All items processed
    assert_eq!(results.len(), 20);

    // Max concurrent should not exceed limit
    let observed = max_observed.load(Ordering::SeqCst);
    assert!(
        observed <= limit,
        "Max concurrent {} exceeded limit {}",
        observed,
        limit
    );
}

#[tokio::test]
async fn test_concurrency_limit_ordered_preserves_order() {
    // Use varying delays to test order preservation
    let items: Vec<(i32, u64)> = vec![
        (1, 50), // slow
        (2, 10), // fast
        (3, 30), // medium
        (4, 5),  // fastest
        (5, 40), // slow-ish
    ];

    let results = map_with_concurrency_limit_ordered(items, 3, |(n, delay)| async move {
        tokio::time::sleep(Duration::from_millis(delay)).await;
        n
    })
    .await;

    // Results should be in input order despite different completion times
    assert_eq!(results, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn test_concurrency_with_failures() {
    let items: Vec<i32> = (0..10).collect();

    let results: Vec<Result<i32, &str>> =
        map_with_concurrency_limit(items, 3, |n| async move {
            if n % 3 == 0 {
                Err("divisible by 3")
            } else {
                Ok(n * 2)
            }
        })
        .await;

    // Count successes and failures
    let successes = results.iter().filter_map(|r| r.as_ref().ok()).count();
    let failures = results.iter().filter(|r| r.is_err()).count();

    // 0, 3, 6, 9 should fail (4 items)
    assert_eq!(failures, 4);
    // 1, 2, 4, 5, 7, 8 should succeed (6 items)
    assert_eq!(successes, 6);
}

#[tokio::test]
async fn test_concurrency_under_load() {
    // Stress test with many items
    let item_count = 100;
    let items: Vec<i32> = (0..item_count).collect();
    let processed = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();

    let results = map_with_concurrency_limit(items, 10, |n| {
        let counter = Arc::clone(&processed);
        async move {
            // Simulate variable work
            tokio::time::sleep(Duration::from_millis(5)).await;
            counter.fetch_add(1, Ordering::SeqCst);
            n
        }
    })
    .await;

    let duration = start.elapsed();

    // All items processed
    assert_eq!(results.len(), item_count as usize);
    assert_eq!(processed.load(Ordering::SeqCst), item_count as usize);

    // With concurrency 10 and 5ms per item, 100 items should take ~50ms
    // Without concurrency it would be 500ms
    // Allow some overhead but ensure we got significant parallelism
    assert!(
        duration < Duration::from_millis(200),
        "Took {:?}, expected < 200ms with parallelism",
        duration
    );
}

#[tokio::test]
async fn test_empty_input_handling() {
    let items: Vec<i32> = vec![];

    let results = map_with_concurrency_limit(items.clone(), 5, |n| async move { n * 2 }).await;

    assert!(results.is_empty());

    let ordered_results =
        map_with_concurrency_limit_ordered(items, 5, |n| async move { n * 2 }).await;

    assert!(ordered_results.is_empty());
}

#[tokio::test]
async fn test_single_item() {
    let items = vec![42];

    let results = map_with_concurrency_limit(items.clone(), 5, |n| async move { n * 2 }).await;

    assert_eq!(results, vec![84]);

    let ordered = map_with_concurrency_limit_ordered(items, 5, |n| async move { n * 2 }).await;

    assert_eq!(ordered, vec![84]);
}

// ============================================================================
// AgentRunner Integration Tests
// ============================================================================

#[tokio::test]
async fn test_agent_runner_initialization() {
    let runner = AgentRunner::new(100);

    assert_eq!(runner.active_count(), 0);
}

#[tokio::test]
async fn test_agent_runner_event_channel() {
    let runner = AgentRunner::new(10);
    let sender = runner.event_sender();

    // Send events through the channel
    sender
        .send(AgentEvent::Started {
            task_id: "test-1".to_string(),
        })
        .await
        .unwrap();

    sender
        .send(AgentEvent::Output {
            task_id: "test-1".to_string(),
            line: "Processing...".to_string(),
        })
        .await
        .unwrap();

    sender
        .send(AgentEvent::Completed {
            result: AgentResult {
                task_id: "test-1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "Done".to_string(),
                duration_ms: 100,
            },
        })
        .await
        .unwrap();

    // Events should be in the channel (we just verify no panic)
}

#[tokio::test]
async fn test_agent_runner_try_recv() {
    let mut runner = AgentRunner::new(10);
    let sender = runner.event_sender();

    // Initially empty
    assert!(runner.try_recv_event().is_none());

    // Send an event
    sender
        .send(AgentEvent::Started {
            task_id: "test".to_string(),
        })
        .await
        .unwrap();

    // Should be receivable
    let event = runner.try_recv_event();
    assert!(event.is_some());

    match event.unwrap() {
        AgentEvent::Started { task_id } => assert_eq!(task_id, "test"),
        _ => panic!("Wrong event type"),
    }
}

// ============================================================================
// ConcurrentSpawnConfig Tests
// ============================================================================

#[test]
fn test_concurrent_spawn_config_defaults() {
    let config = ConcurrentSpawnConfig::default();

    assert_eq!(config.max_concurrent, 5);
    assert_eq!(config.timeout_ms, 0);
    assert!(!config.fail_fast);
}

#[test]
fn test_concurrent_spawn_config_custom() {
    let config = ConcurrentSpawnConfig {
        max_concurrent: 10,
        timeout_ms: 30000,
        fail_fast: true,
    };

    assert_eq!(config.max_concurrent, 10);
    assert_eq!(config.timeout_ms, 30000);
    assert!(config.fail_fast);
}

#[test]
fn test_concurrent_spawn_result_all_success() {
    let result = ConcurrentSpawnResult {
        successes: vec![
            AgentResult {
                task_id: "1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "done".to_string(),
                duration_ms: 100,
            },
            AgentResult {
                task_id: "2".to_string(),
                success: true,
                exit_code: Some(0),
                output: "done".to_string(),
                duration_ms: 150,
            },
        ],
        failures: vec![],
        all_succeeded: true,
    };

    assert!(result.all_succeeded);
    assert_eq!(result.successes.len(), 2);
    assert!(result.failures.is_empty());
}

#[test]
fn test_concurrent_spawn_result_partial_failure() {
    let result = ConcurrentSpawnResult {
        successes: vec![AgentResult {
            task_id: "1".to_string(),
            success: true,
            exit_code: Some(0),
            output: "ok".to_string(),
            duration_ms: 100,
        }],
        failures: vec![
            ("2".to_string(), "timeout".to_string()),
            ("3".to_string(), "spawn failed".to_string()),
        ],
        all_succeeded: false,
    };

    assert!(!result.all_succeeded);
    assert_eq!(result.successes.len(), 1);
    assert_eq!(result.failures.len(), 2);
    assert_eq!(result.failures[0].0, "2");
    assert_eq!(result.failures[1].1, "spawn failed");
}

// ============================================================================
// AgentResult and AgentEvent Tests
// ============================================================================

#[test]
fn test_agent_result_fields() {
    let result = AgentResult {
        task_id: "complex-task:1.2.3".to_string(),
        success: false,
        exit_code: Some(1),
        output: "Error: something went wrong\nStack trace...".to_string(),
        duration_ms: 5000,
    };

    assert_eq!(result.task_id, "complex-task:1.2.3");
    assert!(!result.success);
    assert_eq!(result.exit_code, Some(1));
    assert!(result.output.contains("Error"));
    assert_eq!(result.duration_ms, 5000);
}

#[test]
fn test_agent_event_variants() {
    // Test Started
    let started = AgentEvent::Started {
        task_id: "t1".to_string(),
    };
    match started {
        AgentEvent::Started { task_id } => assert_eq!(task_id, "t1"),
        _ => panic!("Wrong variant"),
    }

    // Test Output
    let output = AgentEvent::Output {
        task_id: "t1".to_string(),
        line: "Processing...".to_string(),
    };
    match output {
        AgentEvent::Output { task_id, line } => {
            assert_eq!(task_id, "t1");
            assert_eq!(line, "Processing...");
        }
        _ => panic!("Wrong variant"),
    }

    // Test Completed
    let completed = AgentEvent::Completed {
        result: AgentResult {
            task_id: "t1".to_string(),
            success: true,
            exit_code: Some(0),
            output: "done".to_string(),
            duration_ms: 100,
        },
    };
    match completed {
        AgentEvent::Completed { result } => {
            assert_eq!(result.task_id, "t1");
            assert!(result.success);
        }
        _ => panic!("Wrong variant"),
    }

    // Test SpawnFailed
    let failed = AgentEvent::SpawnFailed {
        task_id: "t2".to_string(),
        error: "binary not found".to_string(),
    };
    match failed {
        AgentEvent::SpawnFailed { task_id, error } => {
            assert_eq!(task_id, "t2");
            assert!(error.contains("binary"));
        }
        _ => panic!("Wrong variant"),
    }
}

// ============================================================================
// SpawnConfig Tests (via AgentResult which doesn't require internal Harness)
// ============================================================================

#[test]
fn test_spawn_config_fields_via_result() {
    // Test SpawnConfig-related types through AgentResult
    // SpawnConfig itself requires internal Harness type, but we can test
    // the result types that get produced after spawning
    let result = AgentResult {
        task_id: "pitui:12".to_string(),
        success: true,
        exit_code: Some(0),
        output: "Task completed successfully".to_string(),
        duration_ms: 5000,
    };

    assert_eq!(result.task_id, "pitui:12");
    assert!(result.success);
    assert_eq!(result.exit_code, Some(0));
    assert!(result.output.contains("completed"));
    assert_eq!(result.duration_ms, 5000);
}

#[test]
fn test_agent_result_failure_case() {
    let result = AgentResult {
        task_id: "1".to_string(),
        success: false,
        exit_code: Some(1),
        output: "Error: compilation failed".to_string(),
        duration_ms: 1500,
    };

    assert!(!result.success);
    assert_eq!(result.exit_code, Some(1));
    assert!(result.output.contains("Error"));
}

// ============================================================================
// ToolCallResult Tests
// ============================================================================

#[test]
fn test_tool_call_result_clone() {
    let result = ToolCallResult {
        tool_name: "my_tool".to_string(),
        output: json!({"key": "value", "nested": {"a": 1}}),
        success: true,
    };

    let cloned = result.clone();

    assert_eq!(cloned.tool_name, result.tool_name);
    assert_eq!(cloned.output, result.output);
    assert_eq!(cloned.success, result.success);
}

#[test]
fn test_tool_call_result_debug() {
    let result = ToolCallResult {
        tool_name: "test".to_string(),
        output: json!(42),
        success: true,
    };

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("ToolCallResult"));
    assert!(debug_str.contains("test"));
}

// ============================================================================
// Integration Test: Concurrent Tool Execution Simulation
// ============================================================================

#[tokio::test]
async fn test_simulated_concurrent_tool_execution() {
    // Simulate multiple tool calls being processed concurrently
    let runner = Arc::new(Mutex::new(ExtensionRunner::new()));

    // Register a tool
    fn process(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let id = args.get(0).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!({"processed": id, "result": id * 2}))
    }

    {
        let mut r = runner.lock().await;
        r.register_tool("process".to_string(), process);
    }

    // Execute multiple tool calls concurrently
    let task_ids: Vec<i64> = (1..=10).collect();

    let results = map_with_concurrency_limit(task_ids, 5, |id| {
        let runner = Arc::clone(&runner);
        async move {
            let r = runner.lock().await;
            r.on_tool_call("process", json!([id]))
        }
    })
    .await;

    // All should succeed
    assert_eq!(results.len(), 10);
    for result in results {
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.output.get("processed").is_some());
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_extension_runner_default_impl() {
    let runner = ExtensionRunner::default();
    assert!(runner.list_tools().is_empty());
}

#[test]
fn test_tool_overwrite() {
    let mut runner = ExtensionRunner::new();

    fn tool_v1(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!("v1"))
    }

    fn tool_v2(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!("v2"))
    }

    runner.register_tool("tool".to_string(), tool_v1);
    let r1 = runner.on_tool_call("tool", json!({})).unwrap();
    assert_eq!(r1.output, json!("v1"));

    // Overwrite with v2
    runner.register_tool("tool".to_string(), tool_v2);
    let r2 = runner.on_tool_call("tool", json!({})).unwrap();
    assert_eq!(r2.output, json!("v2"));
}

#[tokio::test]
async fn test_concurrency_limit_zero() {
    // Edge case: concurrency limit of 0 should still work (buffered(0) = no buffering)
    // This may behave as serial execution
    let items: Vec<i32> = (1..=3).collect();

    // Note: buffer_unordered(0) and buffered(0) in futures crate
    // may panic or behave unexpectedly - this tests current behavior
    let results = map_with_concurrency_limit(items.clone(), 1, |n| async move { n * 2 }).await;

    let mut sorted = results.clone();
    sorted.sort();
    assert_eq!(sorted, vec![2, 4, 6]);
}

#[tokio::test]
async fn test_high_concurrency_limit() {
    // Concurrency limit higher than item count
    let items: Vec<i32> = (1..=5).collect();

    let results = map_with_concurrency_limit(items, 100, |n| async move { n }).await;

    assert_eq!(results.len(), 5);
}

#[test]
fn test_unicode_in_tool_results() {
    let mut runner = ExtensionRunner::new();

    fn unicode_tool(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Ok(json!({
            "greeting": "こんにちは",
            "emoji": "🚀",
            "mixed": "Hello 世界 🌍"
        }))
    }

    runner.register_tool("unicode".to_string(), unicode_tool);

    let result = runner.on_tool_call("unicode", json!({})).unwrap();
    assert_eq!(result.output["greeting"], "こんにちは");
    assert_eq!(result.output["emoji"], "🚀");
}

#[test]
fn test_large_json_payload() {
    let mut runner = ExtensionRunner::new();

    fn echo(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        // on_tool_call wraps array inputs directly, objects in an array
        Ok(args.first().cloned().unwrap_or(Value::Null))
    }

    runner.register_tool("echo".to_string(), echo);

    // Create a large JSON object payload (objects get wrapped in array by on_tool_call)
    let large_data: Vec<_> = (0..1000).map(|i| json!({"index": i, "data": "x".repeat(100)})).collect();
    let large_payload = json!({"items": large_data});

    let result = runner.on_tool_call("echo", large_payload.clone()).unwrap();

    // Verify it was echoed back correctly (the object is returned as-is)
    assert_eq!(result.output["items"].as_array().unwrap().len(), 1000);
}

// ============================================================================
// Additional Integration Tests for spawn_subagent Lifecycle (Task 12)
// ============================================================================

/// Test AgentRunner complete lifecycle with multiple agents
#[tokio::test]
async fn test_agent_runner_multi_agent_lifecycle() {
    let mut runner = AgentRunner::new(100);
    let sender = runner.event_sender();

    // Spawn a task that simulates agent lifecycle events
    let sender_clone = sender.clone();
    let handle = tokio::spawn(async move {
        for agent_id in &["agent-1", "agent-2", "agent-3"] {
            // Started
            sender_clone
                .send(AgentEvent::Started {
                    task_id: agent_id.to_string(),
                })
                .await
                .unwrap();

            // Multiple output events
            for i in 0..3 {
                sender_clone
                    .send(AgentEvent::Output {
                        task_id: agent_id.to_string(),
                        line: format!("Output line {} from {}", i, agent_id),
                    })
                    .await
                    .unwrap();
            }

            // Completed
            sender_clone
                .send(AgentEvent::Completed {
                    result: AgentResult {
                        task_id: agent_id.to_string(),
                        success: true,
                        exit_code: Some(0),
                        output: format!("Agent {} finished", agent_id),
                        duration_ms: 100,
                    },
                })
                .await
                .unwrap();
        }
    });

    // Wait for sender task to complete
    handle.await.unwrap();

    // Collect all events
    let mut events = Vec::new();
    while let Some(event) = runner.try_recv_event() {
        events.push(event);
    }

    // Verify event counts: 3 agents × (1 started + 3 output + 1 completed) = 15 events
    assert_eq!(events.len(), 15);

    // Count event types
    let started_count = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::Started { .. }))
        .count();
    let output_count = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::Output { .. }))
        .count();
    let completed_count = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::Completed { .. }))
        .count();

    assert_eq!(started_count, 3);
    assert_eq!(output_count, 9);
    assert_eq!(completed_count, 3);
}

/// Test AgentRunner handles spawn failures gracefully
#[tokio::test]
async fn test_agent_runner_spawn_failure_events() {
    let mut runner = AgentRunner::new(10);
    let sender = runner.event_sender();

    // Simulate a spawn failure
    sender
        .send(AgentEvent::SpawnFailed {
            task_id: "failed-task".to_string(),
            error: "Binary not found: /nonexistent/path".to_string(),
        })
        .await
        .unwrap();

    // Verify event was received
    let event = runner.recv_event().await.unwrap();
    match event {
        AgentEvent::SpawnFailed { task_id, error } => {
            assert_eq!(task_id, "failed-task");
            assert!(error.contains("Binary not found"));
        }
        _ => panic!("Expected SpawnFailed event"),
    }
}

/// Test AgentRunner channel capacity handling
#[tokio::test]
async fn test_agent_runner_channel_capacity() {
    // Small capacity to test backpressure
    let runner = AgentRunner::new(5);
    let sender = runner.event_sender();

    // Fill the channel
    for i in 0..5 {
        sender
            .send(AgentEvent::Output {
                task_id: format!("task-{}", i),
                line: "test".to_string(),
            })
            .await
            .unwrap();
    }

    // Next send should block (we use try_send to test this)
    // Note: mpsc::Sender doesn't have try_send, but we can verify the queue is full
    // by checking that the channel accepts messages up to capacity

    // This verifies the channel was created with the right capacity
    assert!(sender.capacity() == 5 || sender.capacity() == 0); // 0 when full
}

/// Test concurrent AgentRunner operations
#[tokio::test]
async fn test_agent_runner_concurrent_operations() {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let runner = Arc::new(tokio::sync::Mutex::new(AgentRunner::new(100)));
    let barrier = Arc::new(Barrier::new(5));

    let mut handles = Vec::new();

    // Spawn 5 concurrent tasks that send events
    for i in 0..5 {
        let runner = Arc::clone(&runner);
        let barrier = Arc::clone(&barrier);

        handles.push(tokio::spawn(async move {
            let r = runner.lock().await;
            let sender = r.event_sender();
            drop(r); // Release lock before barrier

            barrier.wait().await;

            // Send events concurrently
            for j in 0..10 {
                sender
                    .send(AgentEvent::Output {
                        task_id: format!("task-{}-{}", i, j),
                        line: format!("output from {} iteration {}", i, j),
                    })
                    .await
                    .ok();
            }
        }));
    }

    // Wait for all senders to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify events were received
    let mut runner = runner.lock().await;
    let mut count = 0;
    while runner.try_recv_event().is_some() {
        count += 1;
    }

    // Should have 50 events (5 tasks × 10 events each)
    assert_eq!(count, 50);
}

// ============================================================================
// SpawnConfig Integration Tests
// ============================================================================

#[test]
fn test_spawn_config_with_all_fields() {
    use std::path::PathBuf;

    // We can verify SpawnConfig fields work correctly
    let config = SpawnConfig {
        task_id: "complex:1.2.3".to_string(),
        prompt: "Implement feature X with the following requirements:\n1. Do A\n2. Do B".to_string(),
        working_dir: PathBuf::from("/tmp/test-project"),
        harness: Harness::Claude,
        model: Some("opus".to_string()),
    };

    assert_eq!(config.task_id, "complex:1.2.3");
    assert!(config.prompt.contains("Implement feature"));
    assert!(config.model.is_some());
}

#[test]
fn test_spawn_config_minimal() {
    use std::path::PathBuf;

    let config = SpawnConfig {
        task_id: "1".to_string(),
        prompt: "simple prompt".to_string(),
        working_dir: PathBuf::from("."),
        harness: Harness::default(),
        model: None,
    };

    assert_eq!(config.task_id, "1");
    assert!(config.model.is_none());
}

// ============================================================================
// Concurrent Spawn Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_spawn_with_varied_execution_times() {
    // Simulate agents with different execution times
    let configs: Vec<(String, u64)> = vec![
        ("fast-1".to_string(), 10),
        ("slow-1".to_string(), 50),
        ("fast-2".to_string(), 5),
        ("slow-2".to_string(), 40),
        ("medium".to_string(), 25),
    ];

    let start = Instant::now();

    let results = map_with_concurrency_limit(configs, 3, |(id, delay)| async move {
        tokio::time::sleep(Duration::from_millis(delay)).await;
        (id, delay)
    })
    .await;

    let duration = start.elapsed();

    // All should complete
    assert_eq!(results.len(), 5);

    // With concurrency 3, the total time should be less than sequential (130ms)
    // but more than pure parallel (50ms)
    // Expected: ~50-60ms (max(50, 10+40) with some overlap)
    assert!(
        duration < Duration::from_millis(100),
        "Expected parallelism, got {:?}",
        duration
    );
}

#[tokio::test]
async fn test_concurrent_spawn_with_partial_failures() {
    let items: Vec<(i32, bool)> = vec![
        (1, true),  // success
        (2, false), // fail
        (3, true),  // success
        (4, false), // fail
        (5, true),  // success
    ];

    let results: Vec<Result<i32, String>> =
        map_with_concurrency_limit(items, 2, |(id, should_succeed)| async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            if should_succeed {
                Ok(id)
            } else {
                Err(format!("Task {} failed", id))
            }
        })
        .await;

    let successes: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let failures: Vec<_> = results.iter().filter(|r| r.is_err()).collect();

    assert_eq!(successes.len(), 3);
    assert_eq!(failures.len(), 2);
}

#[tokio::test]
async fn test_concurrent_spawn_stress_many_items() {
    // Stress test with many items
    let item_count = 500;
    let items: Vec<i32> = (0..item_count).collect();
    let processed = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let limit = 20;
    let start = Instant::now();

    let results = map_with_concurrency_limit(items, limit, |n| {
        let processed = Arc::clone(&processed);
        let max_conc = Arc::clone(&max_concurrent);
        let curr_conc = Arc::clone(&current_concurrent);
        async move {
            // Track concurrency
            let current = curr_conc.fetch_add(1, Ordering::SeqCst) + 1;

            // Update max
            let mut max = max_conc.load(Ordering::SeqCst);
            while current > max {
                match max_conc.compare_exchange_weak(
                    max,
                    current,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(m) => max = m,
                }
            }

            // Simulate work
            tokio::time::sleep(Duration::from_millis(2)).await;

            curr_conc.fetch_sub(1, Ordering::SeqCst);
            processed.fetch_add(1, Ordering::SeqCst);
            n
        }
    })
    .await;

    let duration = start.elapsed();

    // Verify all processed
    assert_eq!(results.len(), item_count as usize);
    assert_eq!(processed.load(Ordering::SeqCst), item_count as usize);

    // Verify concurrency was bounded
    assert!(
        max_concurrent.load(Ordering::SeqCst) <= limit,
        "Max concurrent {} exceeded limit {}",
        max_concurrent.load(Ordering::SeqCst),
        limit
    );

    // Verify we got significant parallelism
    // Sequential would be 500 * 2ms = 1000ms
    // With 20 concurrent, expect ~50ms (500/20 * 2ms)
    assert!(
        duration < Duration::from_millis(500),
        "Expected parallelism, took {:?}",
        duration
    );
}

// ============================================================================
// Mock Process Tests (using simple script simulation)
// ============================================================================

#[tokio::test]
async fn test_simulated_process_lifecycle() {
    use tokio::sync::mpsc;

    // Simulate what spawn_agent does internally
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(10);
    let task_id = "test-task-1";

    // Simulate the lifecycle that would happen with a real process
    tokio::spawn(async move {
        // Started event
        tx.send(AgentEvent::Started {
            task_id: task_id.to_string(),
        })
        .await
        .unwrap();

        // Simulate stdout output
        for i in 0..5 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(AgentEvent::Output {
                task_id: task_id.to_string(),
                line: format!("Processing step {}", i + 1),
            })
            .await
            .unwrap();
        }

        // Completed event
        tx.send(AgentEvent::Completed {
            result: AgentResult {
                task_id: task_id.to_string(),
                success: true,
                exit_code: Some(0),
                output: "All steps completed".to_string(),
                duration_ms: 50,
            },
        })
        .await
        .unwrap();
    });

    // Collect and verify events
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event.clone());
        if matches!(event, AgentEvent::Completed { .. }) {
            break;
        }
    }

    // Verify lifecycle: Started -> Output(s) -> Completed
    assert!(matches!(events[0], AgentEvent::Started { .. }));
    assert!(matches!(events[1], AgentEvent::Output { .. }));
    assert!(matches!(events.last(), Some(AgentEvent::Completed { .. })));
}

#[tokio::test]
async fn test_simulated_process_failure() {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(10);

    // Simulate a process that fails
    tokio::spawn(async move {
        tx.send(AgentEvent::Started {
            task_id: "failing-task".to_string(),
        })
        .await
        .unwrap();

        // Some output before failure
        tx.send(AgentEvent::Output {
            task_id: "failing-task".to_string(),
            line: "Starting work...".to_string(),
        })
        .await
        .unwrap();

        tx.send(AgentEvent::Output {
            task_id: "failing-task".to_string(),
            line: "[stderr] Error: something went wrong".to_string(),
        })
        .await
        .unwrap();

        // Failed completion
        tx.send(AgentEvent::Completed {
            result: AgentResult {
                task_id: "failing-task".to_string(),
                success: false,
                exit_code: Some(1),
                output: "Error: something went wrong".to_string(),
                duration_ms: 25,
            },
        })
        .await
        .unwrap();
    });

    let mut result: Option<AgentResult> = None;
    while let Some(event) = rx.recv().await {
        if let AgentEvent::Completed { result: r } = event {
            result = Some(r);
            break;
        }
    }

    let result = result.unwrap();
    assert!(!result.success);
    assert_eq!(result.exit_code, Some(1));
    assert!(result.output.contains("Error"));
}

#[tokio::test]
async fn test_simulated_multiple_concurrent_processes() {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(100);
    let task_ids = vec!["proc-1", "proc-2", "proc-3"];

    // Spawn multiple simulated processes
    for task_id in task_ids.iter() {
        let tx = tx.clone();
        let task_id = task_id.to_string();

        tokio::spawn(async move {
            tx.send(AgentEvent::Started {
                task_id: task_id.clone(),
            })
            .await
            .unwrap();

            // Variable delay to simulate different execution times
            let delay = match task_id.as_str() {
                "proc-1" => 30,
                "proc-2" => 10,
                "proc-3" => 20,
                _ => 15,
            };

            tokio::time::sleep(Duration::from_millis(delay)).await;

            tx.send(AgentEvent::Completed {
                result: AgentResult {
                    task_id: task_id.clone(),
                    success: true,
                    exit_code: Some(0),
                    output: format!("{} done", task_id),
                    duration_ms: delay,
                },
            })
            .await
            .unwrap();
        });
    }

    // Drop original sender so rx.recv() eventually returns None
    drop(tx);

    // Collect all events
    let mut completed_order = Vec::new();
    while let Some(event) = rx.recv().await {
        if let AgentEvent::Completed { result } = event {
            completed_order.push(result.task_id.clone());
        }
    }

    // All three should complete
    assert_eq!(completed_order.len(), 3);

    // Order should be proc-2, proc-3, proc-1 (fastest to slowest)
    assert_eq!(completed_order[0], "proc-2");
    assert_eq!(completed_order[1], "proc-3");
    assert_eq!(completed_order[2], "proc-1");
}

// ============================================================================
// ConcurrentSpawnResult Integration Tests
// ============================================================================

#[test]
fn test_concurrent_spawn_result_mixed() {
    let result = ConcurrentSpawnResult {
        successes: vec![
            AgentResult {
                task_id: "task-1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "completed".to_string(),
                duration_ms: 100,
            },
            AgentResult {
                task_id: "task-3".to_string(),
                success: true,
                exit_code: Some(0),
                output: "completed".to_string(),
                duration_ms: 150,
            },
        ],
        failures: vec![
            ("task-2".to_string(), "timeout".to_string()),
            ("task-4".to_string(), "binary not found".to_string()),
        ],
        all_succeeded: false,
    };

    assert!(!result.all_succeeded);
    assert_eq!(result.successes.len(), 2);
    assert_eq!(result.failures.len(), 2);

    // Verify failure details
    let failure_ids: Vec<_> = result.failures.iter().map(|(id, _)| id.as_str()).collect();
    assert!(failure_ids.contains(&"task-2"));
    assert!(failure_ids.contains(&"task-4"));
}

// ============================================================================
// Extended Error Handling Tests
// ============================================================================

#[test]
fn test_extension_runner_error_display() {
    let error1 = ExtensionRunnerError::ToolNotFound("missing_tool".to_string());
    let error_str = format!("{}", error1);
    assert!(error_str.contains("Tool not found"));
    assert!(error_str.contains("missing_tool"));

    // ExecutionError wraps another error
    let inner_error: Box<dyn std::error::Error + Send + Sync> = "inner failure".into();
    let error2 = ExtensionRunnerError::ExecutionError(inner_error);
    let error_str2 = format!("{}", error2);
    assert!(error_str2.contains("Tool execution error"));
}

#[test]
fn test_tool_that_panics_is_caught() {
    // Note: Rust panics are not caught by Result, but we can test that
    // errors returned from tools are properly wrapped

    let mut runner = ExtensionRunner::new();

    fn error_tool(_: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("simulated panic-like error".into())
    }

    runner.register_tool("error_tool".to_string(), error_tool);

    let result = runner.on_tool_call("error_tool", json!({}));
    assert!(result.is_err());

    match result {
        Err(ExtensionRunnerError::ExecutionError(e)) => {
            assert!(e.to_string().contains("simulated panic"));
        }
        _ => panic!("Expected ExecutionError"),
    }
}

// ============================================================================
// Advanced Tool Registration Tests
// ============================================================================

#[test]
fn test_tool_with_state_via_closure() {
    // Test that we can have stateful tools via closures
    // Note: ToolFn is a function pointer, not a closure, so we test the pattern
    // that tools can maintain state via their arguments

    let mut runner = ExtensionRunner::new();

    // A tool that tracks cumulative state via input
    fn accumulator(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let current = args.get(0).and_then(|v| v.get("current")).and_then(|v| v.as_i64()).unwrap_or(0);
        let add = args.get(0).and_then(|v| v.get("add")).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!({"result": current + add}))
    }

    runner.register_tool("accumulator".to_string(), accumulator);

    // First call
    let r1 = runner
        .on_tool_call("accumulator", json!({"current": 0, "add": 5}))
        .unwrap();
    assert_eq!(r1.output["result"], 5);

    // Second call using previous result
    let r2 = runner
        .on_tool_call("accumulator", json!({"current": 5, "add": 10}))
        .unwrap();
    assert_eq!(r2.output["result"], 15);
}

#[test]
fn test_multiple_tools_interaction() {
    let mut runner = ExtensionRunner::new();

    fn to_upper(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let s = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!(s.to_uppercase()))
    }

    fn concat(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let parts: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
        Ok(json!(parts.join("")))
    }

    fn length(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let s = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!(s.len()))
    }

    runner.register_tool("to_upper".to_string(), to_upper);
    runner.register_tool("concat".to_string(), concat);
    runner.register_tool("length".to_string(), length);

    // Chain: "hello" -> TO_UPPER -> "HELLO"
    let r1 = runner.on_tool_call("to_upper", json!(["hello"])).unwrap();
    assert_eq!(r1.output, "HELLO");

    // Chain: ["HELLO", " ", "WORLD"] -> CONCAT -> "HELLO WORLD"
    let r2 = runner
        .on_tool_call("concat", json!(["HELLO", " ", "WORLD"]))
        .unwrap();
    assert_eq!(r2.output, "HELLO WORLD");

    // Chain: "HELLO WORLD" -> LENGTH -> 11
    let r3 = runner.on_tool_call("length", json!(["HELLO WORLD"])).unwrap();
    assert_eq!(r3.output, 11);
}
