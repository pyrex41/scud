//! Extension runner for tools and agent spawning
//!
//! Provides:
//! - Tool execution via registered functions
//! - Async agent spawning without tmux
//! - Bounded concurrent execution (mapWithConcurrencyLimit pattern)

use crate::extensions::types::ToolFn;
use futures::stream::{self, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agents::AgentDef;
use crate::commands::spawn::terminal::{find_harness_binary, Harness};

// ============================================================================
// Tool Execution
// ============================================================================

/// Extension runner for executing extension-provided tools
pub struct ExtensionRunner {
    registered_tools: HashMap<String, ToolFn>,
}

impl ExtensionRunner {
    /// Create a new extension runner
    pub fn new() -> Self {
        ExtensionRunner {
            registered_tools: HashMap::new(),
        }
    }

    /// Register a tool function with a given name
    pub fn register_tool(&mut self, name: String, tool_fn: ToolFn) {
        self.registered_tools.insert(name, tool_fn);
    }

    /// Execute a registered tool by name with the given arguments
    pub fn execute_tool(&self, name: &str, args: &[Value]) -> Result<Value, ExtensionRunnerError> {
        let tool_fn = self
            .registered_tools
            .get(name)
            .ok_or_else(|| ExtensionRunnerError::ToolNotFound(name.to_string()))?;

        tool_fn(args).map_err(ExtensionRunnerError::ExecutionError)
    }

    /// Check if a tool is registered
    pub fn has_tool(&self, name: &str) -> bool {
        self.registered_tools.contains_key(name)
    }

    /// Get list of registered tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.registered_tools.keys().cloned().collect()
    }

    /// Handle a tool call event
    ///
    /// This method processes a tool call request, executing the registered tool
    /// and returning the result. It provides a unified entry point for tool
    /// call handling from various sources (LLM responses, IPC, etc.)
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to call
    /// * `arguments` - JSON arguments to pass to the tool
    ///
    /// # Returns
    /// Result containing the tool's response or an error
    pub fn on_tool_call(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<ToolCallResult, ExtensionRunnerError> {
        // Convert arguments to array format expected by ToolFn
        let args = match arguments {
            Value::Array(arr) => arr,
            Value::Object(_) => vec![arguments],
            Value::Null => vec![],
            other => vec![other],
        };

        let result = self.execute_tool(tool_name, &args)?;

        Ok(ToolCallResult {
            tool_name: tool_name.to_string(),
            output: result,
            success: true,
        })
    }
}

/// Result from a tool call execution
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    /// Name of the tool that was called
    pub tool_name: String,
    /// Output from the tool (JSON value)
    pub output: Value,
    /// Whether the tool executed successfully
    pub success: bool,
}

impl Default for ExtensionRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for extension runner operations
#[derive(Debug, thiserror::Error)]
pub enum ExtensionRunnerError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution error: {0}")]
    ExecutionError(Box<dyn std::error::Error + Send + Sync>),
}

// ============================================================================
// Agent Spawning
// ============================================================================

/// Result from an agent execution
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Task ID this agent was working on
    pub task_id: String,
    /// Whether the agent completed successfully
    pub success: bool,
    /// Exit code from the process
    pub exit_code: Option<i32>,
    /// Captured output (if any)
    pub output: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Event emitted during agent execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent process started
    Started { task_id: String },
    /// Agent produced output
    Output { task_id: String, line: String },
    /// Agent completed
    Completed { result: AgentResult },
    /// Agent failed to start
    SpawnFailed { task_id: String, error: String },
}

/// Configuration for spawning an agent
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Task ID being worked on
    pub task_id: String,
    /// Prompt to send to the agent
    pub prompt: String,
    /// Working directory
    pub working_dir: PathBuf,
    /// Harness to use
    pub harness: Harness,
    /// Model to use (if specified)
    pub model: Option<String>,
}

/// Spawn a single agent as an async subprocess
///
/// Returns a handle to the spawned task that resolves to AgentResult.
pub async fn spawn_agent(
    config: SpawnConfig,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<tokio::task::JoinHandle<AgentResult>, anyhow::Error> {
    let binary_path = find_harness_binary(config.harness)?;
    let task_id = config.task_id.clone();

    // Build command arguments based on harness
    let mut cmd = match config.harness {
        Harness::Claude => {
            let mut c = Command::new(binary_path);
            c.arg(&config.prompt);
            c.arg("--dangerously-skip-permissions");
            if let Some(ref model) = config.model {
                c.arg("--model").arg(model);
            }
            c
        }
        Harness::OpenCode => {
            let mut c = Command::new(binary_path);
            c.arg("run");
            c.arg("--variant").arg("minimal");
            if let Some(ref model) = config.model {
                c.arg("--model").arg(model);
            }
            c.arg(&config.prompt);
            c
        }
        Harness::Cursor => {
            let mut c = Command::new(binary_path);
            c.arg("-p");
            if let Some(ref model) = config.model {
                c.arg("--model").arg(model);
            }
            c.arg(&config.prompt);
            c
        }
        Harness::Rho => {
            let mut c = Command::new(binary_path);
            c.arg("-p").arg(&config.prompt);
            c.arg("-C").arg(&config.working_dir);
            if let Some(ref model) = config.model {
                c.arg("--model").arg(model);
            }
            c
        }
        #[cfg(feature = "direct-api")]
        Harness::DirectApi => {
            let mut c = Command::new(binary_path);
            c.arg("agent-exec");
            c.arg("--prompt").arg(&config.prompt);
            if let Some(ref model) = config.model {
                c.arg("--model").arg(model);
            }
            c
        }
    };

    // Set working directory and environment
    cmd.current_dir(&config.working_dir);
    cmd.env("SCUD_TASK_ID", &config.task_id);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let start_time = std::time::Instant::now();

    // Spawn the process
    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!(
            "Failed to spawn {} for task {}: {}",
            config.harness.name(),
            config.task_id,
            e
        )
    })?;

    // Notify that agent started
    let _ = event_tx
        .send(AgentEvent::Started {
            task_id: task_id.clone(),
        })
        .await;

    // Capture stdout in a background task
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let event_tx_clone = event_tx.clone();
    let task_id_clone = task_id.clone();

    let handle = tokio::spawn(async move {
        let mut output_buffer = String::new();

        // Read stdout
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                output_buffer.push_str(&line);
                output_buffer.push('\n');
                let _ = event_tx_clone
                    .send(AgentEvent::Output {
                        task_id: task_id_clone.clone(),
                        line: line.clone(),
                    })
                    .await;
            }
        }

        // Read stderr
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                output_buffer.push_str("[stderr] ");
                output_buffer.push_str(&line);
                output_buffer.push('\n');
            }
        }

        // Wait for process to complete
        let status = child.wait().await;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        let (success, exit_code) = match status {
            Ok(s) => (s.success(), s.code()),
            Err(_) => (false, None),
        };

        let result = AgentResult {
            task_id: task_id_clone.clone(),
            success,
            exit_code,
            output: output_buffer,
            duration_ms,
        };

        let _ = event_tx_clone
            .send(AgentEvent::Completed {
                result: result.clone(),
            })
            .await;

        result
    });

    Ok(handle)
}

/// Load and resolve agent configuration for a task
pub fn load_agent_config(
    agent_type: Option<&str>,
    default_harness: Harness,
    default_model: Option<&str>,
    working_dir: &Path,
) -> (Harness, Option<String>) {
    if let Some(agent_name) = agent_type {
        if let Some(agent_def) = AgentDef::try_load(agent_name, working_dir) {
            let harness = agent_def.harness().unwrap_or(default_harness);
            let model = agent_def
                .model()
                .map(String::from)
                .or_else(|| default_model.map(String::from));
            return (harness, model);
        }
    }

    // Fall back to defaults
    (default_harness, default_model.map(String::from))
}

/// Runner for managing multiple concurrent agent executions
pub struct AgentRunner {
    /// Event sender for broadcasting agent events
    event_tx: mpsc::Sender<AgentEvent>,
    /// Event receiver for consuming agent events
    event_rx: mpsc::Receiver<AgentEvent>,
    /// Active agent handles
    handles: Vec<tokio::task::JoinHandle<AgentResult>>,
}

impl AgentRunner {
    /// Create a new runner with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (event_tx, event_rx) = mpsc::channel(capacity);
        Self {
            event_tx,
            event_rx,
            handles: Vec::new(),
        }
    }

    /// Get a clone of the event sender for spawning agents
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Spawn an agent and track its handle
    pub async fn spawn(&mut self, config: SpawnConfig) -> anyhow::Result<()> {
        let handle = spawn_agent(config, self.event_tx.clone()).await?;
        self.handles.push(handle);
        Ok(())
    }

    /// Receive the next event (blocks until available)
    pub async fn recv_event(&mut self) -> Option<AgentEvent> {
        self.event_rx.recv().await
    }

    /// Try to receive an event without blocking
    pub fn try_recv_event(&mut self) -> Option<AgentEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Wait for all spawned agents to complete
    pub async fn wait_all(&mut self) -> Vec<AgentResult> {
        let handles = std::mem::take(&mut self.handles);
        let mut results = Vec::new();

        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// Check how many agents are still running
    pub fn active_count(&self) -> usize {
        self.handles.iter().filter(|h| !h.is_finished()).count()
    }
}

// ============================================================================
// Concurrent Execution Utilities
// ============================================================================

/// Execute async operations with bounded concurrency (mapWithConcurrencyLimit pattern)
///
/// This utility applies a limit to how many async operations run simultaneously,
/// preventing resource exhaustion when processing large batches.
///
/// # Type Parameters
/// * `T` - Input item type
/// * `F` - Async function/closure that processes each item
/// * `Fut` - Future returned by F
/// * `R` - Result type of each operation
///
/// # Arguments
/// * `items` - Iterator of items to process
/// * `concurrency` - Maximum number of concurrent operations
/// * `f` - Async function to apply to each item
///
/// # Returns
/// Vector of results in completion order (not input order)
///
/// # Example
/// ```ignore
/// let results = map_with_concurrency_limit(
///     task_ids,
///     5,  // max 5 concurrent
///     |id| async move { process_task(id).await }
/// ).await;
/// ```
pub async fn map_with_concurrency_limit<T, F, Fut, R>(
    items: impl IntoIterator<Item = T>,
    concurrency: usize,
    f: F,
) -> Vec<R>
where
    F: Fn(T) -> Fut,
    Fut: Future<Output = R>,
{
    stream::iter(items)
        .map(f)
        .buffer_unordered(concurrency)
        .collect()
        .await
}

/// Execute async operations with bounded concurrency, preserving input order
///
/// Similar to `map_with_concurrency_limit` but results are returned in
/// the same order as the input items.
///
/// # Arguments
/// * `items` - Iterator of items to process
/// * `concurrency` - Maximum number of concurrent operations
/// * `f` - Async function to apply to each item
///
/// # Returns
/// Vector of results in input order
pub async fn map_with_concurrency_limit_ordered<T, F, Fut, R>(
    items: impl IntoIterator<Item = T>,
    concurrency: usize,
    f: F,
) -> Vec<R>
where
    F: Fn(T) -> Fut,
    Fut: Future<Output = R>,
{
    stream::iter(items)
        .map(f)
        .buffered(concurrency)
        .collect()
        .await
}

/// Spawn multiple agents with bounded concurrency
///
/// Unlike spawning all agents at once, this limits how many agent processes
/// run simultaneously to prevent system resource exhaustion.
///
/// # Arguments
/// * `configs` - Iterator of spawn configurations
/// * `concurrency` - Maximum number of concurrent agent processes
/// * `event_tx` - Channel for receiving agent events
///
/// # Returns
/// Vector of agent results after all agents complete
pub async fn spawn_agents_with_limit(
    configs: impl IntoIterator<Item = SpawnConfig>,
    concurrency: usize,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Vec<Result<AgentResult, anyhow::Error>> {
    let configs: Vec<_> = configs.into_iter().collect();

    map_with_concurrency_limit(configs, concurrency, |config| {
        let tx = event_tx.clone();
        async move {
            match spawn_agent(config, tx).await {
                Ok(handle) => handle
                    .await
                    .map_err(|e| anyhow::anyhow!("Join error: {}", e)),
                Err(e) => Err(e),
            }
        }
    })
    .await
}

/// Configuration for bounded concurrent spawning
#[derive(Debug, Clone)]
pub struct ConcurrentSpawnConfig {
    /// Maximum concurrent agents
    pub max_concurrent: usize,
    /// Timeout per agent (milliseconds, 0 = no timeout)
    pub timeout_ms: u64,
    /// Whether to fail fast on first error
    pub fail_fast: bool,
}

impl Default for ConcurrentSpawnConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            timeout_ms: 0,
            fail_fast: false,
        }
    }
}

/// Result from a concurrent spawn operation
#[derive(Debug)]
pub struct ConcurrentSpawnResult {
    /// Successful agent results
    pub successes: Vec<AgentResult>,
    /// Failed spawns with task IDs and errors
    pub failures: Vec<(String, String)>,
    /// Whether all agents succeeded
    pub all_succeeded: bool,
}

/// Spawn multiple agents with advanced concurrency control
///
/// Provides more control over concurrent spawning including timeouts,
/// fail-fast behavior, and detailed result tracking.
///
/// # Arguments
/// * `configs` - Vector of spawn configurations
/// * `spawn_config` - Concurrency control settings
/// * `event_tx` - Channel for receiving agent events
///
/// # Returns
/// ConcurrentSpawnResult with successes and failures
pub async fn spawn_agents_concurrent(
    configs: Vec<SpawnConfig>,
    spawn_config: ConcurrentSpawnConfig,
    event_tx: mpsc::Sender<AgentEvent>,
) -> ConcurrentSpawnResult {
    let mut successes = Vec::new();
    let mut failures = Vec::new();

    let results = if spawn_config.timeout_ms > 0 {
        // With timeout
        let timeout_duration = std::time::Duration::from_millis(spawn_config.timeout_ms);

        map_with_concurrency_limit(configs, spawn_config.max_concurrent, |config| {
            let tx = event_tx.clone();
            let task_id = config.task_id.clone();
            async move {
                let result = tokio::time::timeout(timeout_duration, async {
                    match spawn_agent(config, tx).await {
                        Ok(handle) => handle
                            .await
                            .map_err(|e| anyhow::anyhow!("Join error: {}", e)),
                        Err(e) => Err(e),
                    }
                })
                .await;

                match result {
                    Ok(Ok(agent_result)) => Ok(agent_result),
                    Ok(Err(e)) => Err((task_id, e.to_string())),
                    Err(_) => Err((task_id, "Timeout".to_string())),
                }
            }
        })
        .await
    } else {
        // Without timeout
        map_with_concurrency_limit(configs, spawn_config.max_concurrent, |config| {
            let tx = event_tx.clone();
            let task_id = config.task_id.clone();
            async move {
                match spawn_agent(config, tx).await {
                    Ok(handle) => handle
                        .await
                        .map_err(|e| (task_id, format!("Join error: {}", e))),
                    Err(e) => Err((task_id, e.to_string())),
                }
            }
        })
        .await
    };

    for result in results {
        match result {
            Ok(agent_result) => successes.push(agent_result),
            Err((task_id, error)) => failures.push((task_id, error)),
        }
    }

    let all_succeeded = failures.is_empty();

    ConcurrentSpawnResult {
        successes,
        failures,
        all_succeeded,
    }
}

/// Subagent spawn helper for use within extensions
///
/// Spawns a subagent for a specific task and returns its result.
/// This is the primary interface for extensions that need to spawn
/// child agents.
///
/// # Arguments
/// * `task_id` - Identifier for the spawned task
/// * `prompt` - Prompt to send to the agent
/// * `working_dir` - Working directory for the agent
/// * `harness` - Which AI harness to use
/// * `model` - Optional model override
///
/// # Returns
/// AgentResult after the subagent completes
pub async fn spawn_subagent(
    task_id: String,
    prompt: String,
    working_dir: PathBuf,
    harness: Harness,
    model: Option<String>,
) -> Result<AgentResult, anyhow::Error> {
    // Create a channel just for this agent
    let (tx, _rx) = mpsc::channel(10);

    let config = SpawnConfig {
        task_id,
        prompt,
        working_dir,
        harness,
        model,
    };

    let handle = spawn_agent(config, tx).await?;
    handle
        .await
        .map_err(|e| anyhow::anyhow!("Subagent join error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_runner_new() {
        let runner = ExtensionRunner::new();
        assert!(runner.list_tools().is_empty());
    }

    #[test]
    fn test_agent_result_debug() {
        let result = AgentResult {
            task_id: "test:1".to_string(),
            success: true,
            exit_code: Some(0),
            output: "test output".to_string(),
            duration_ms: 1000,
        };

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.task_id, "test:1");
    }

    #[test]
    fn test_spawn_config_debug() {
        let config = SpawnConfig {
            task_id: "test:1".to_string(),
            prompt: "do something".to_string(),
            working_dir: PathBuf::from("/tmp"),
            harness: Harness::Claude,
            model: Some("opus".to_string()),
        };

        assert_eq!(config.task_id, "test:1");
        assert_eq!(config.harness, Harness::Claude);
    }

    #[tokio::test]
    async fn test_agent_runner_new() {
        let runner = AgentRunner::new(100);
        assert_eq!(runner.active_count(), 0);
    }

    #[test]
    fn test_tool_call_result() {
        let result = ToolCallResult {
            tool_name: "my_tool".to_string(),
            output: serde_json::json!({"key": "value"}),
            success: true,
        };

        assert_eq!(result.tool_name, "my_tool");
        assert!(result.success);
        assert_eq!(result.output["key"], "value");
    }

    #[test]
    fn test_on_tool_call_not_found() {
        let runner = ExtensionRunner::new();
        let result = runner.on_tool_call("nonexistent", serde_json::json!({}));

        assert!(result.is_err());
        match result {
            Err(ExtensionRunnerError::ToolNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected ToolNotFound error"),
        }
    }

    #[test]
    fn test_on_tool_call_with_registered_tool() {
        let mut runner = ExtensionRunner::new();

        // Register a simple tool that echoes input
        fn echo_tool(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
            Ok(args.first().cloned().unwrap_or(Value::Null))
        }

        runner.register_tool("echo".to_string(), echo_tool);

        // Call with object argument
        let result = runner
            .on_tool_call("echo", serde_json::json!({"test": 123}))
            .unwrap();

        assert_eq!(result.tool_name, "echo");
        assert!(result.success);
        assert_eq!(result.output["test"], 123);
    }

    #[test]
    fn test_on_tool_call_argument_conversion() {
        let mut runner = ExtensionRunner::new();

        // Tool that returns the number of args received
        fn count_args(args: &[Value]) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
            Ok(serde_json::json!(args.len()))
        }

        runner.register_tool("count".to_string(), count_args);

        // Array input - passed directly
        let result = runner
            .on_tool_call("count", serde_json::json!([1, 2, 3]))
            .unwrap();
        assert_eq!(result.output, 3);

        // Object input - wrapped in array
        let result = runner
            .on_tool_call("count", serde_json::json!({"a": 1}))
            .unwrap();
        assert_eq!(result.output, 1);

        // Null input - empty array
        let result = runner.on_tool_call("count", Value::Null).unwrap();
        assert_eq!(result.output, 0);

        // Scalar input - wrapped in array
        let result = runner.on_tool_call("count", serde_json::json!(42)).unwrap();
        assert_eq!(result.output, 1);
    }

    #[tokio::test]
    async fn test_map_with_concurrency_limit() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let items: Vec<i32> = (0..10).collect();
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let results = map_with_concurrency_limit(items, 3, |n| {
            let counter = Arc::clone(&counter);
            let max_concurrent = Arc::clone(&max_concurrent);
            async move {
                // Increment counter
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

                // Update max concurrent if higher
                let mut max = max_concurrent.load(Ordering::SeqCst);
                while current > max {
                    match max_concurrent.compare_exchange_weak(
                        max,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(new_max) => max = new_max,
                    }
                }

                // Simulate some work
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                // Decrement counter
                counter.fetch_sub(1, Ordering::SeqCst);

                n * 2
            }
        })
        .await;

        // All items processed
        assert_eq!(results.len(), 10);

        // Results contain doubled values (order may vary due to buffer_unordered)
        let mut sorted: Vec<i32> = results;
        sorted.sort();
        assert_eq!(sorted, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);

        // Max concurrent should be <= 3
        assert!(max_concurrent.load(Ordering::SeqCst) <= 3);
    }

    #[tokio::test]
    async fn test_map_with_concurrency_limit_ordered() {
        let items: Vec<i32> = vec![1, 2, 3, 4, 5];

        let results = map_with_concurrency_limit_ordered(items, 2, |n| async move {
            // Add some variance in timing
            tokio::time::sleep(std::time::Duration::from_millis((5 - n) as u64 * 5)).await;
            n * 10
        })
        .await;

        // Results should be in input order despite different completion times
        assert_eq!(results, vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn test_concurrent_spawn_config_default() {
        let config = ConcurrentSpawnConfig::default();

        assert_eq!(config.max_concurrent, 5);
        assert_eq!(config.timeout_ms, 0);
        assert!(!config.fail_fast);
    }

    #[test]
    fn test_concurrent_spawn_result() {
        let result = ConcurrentSpawnResult {
            successes: vec![AgentResult {
                task_id: "1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "done".to_string(),
                duration_ms: 100,
            }],
            failures: vec![],
            all_succeeded: true,
        };

        assert!(result.all_succeeded);
        assert_eq!(result.successes.len(), 1);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_concurrent_spawn_result_with_failures() {
        let result = ConcurrentSpawnResult {
            successes: vec![],
            failures: vec![("task1".to_string(), "error msg".to_string())],
            all_succeeded: false,
        };

        assert!(!result.all_succeeded);
        assert!(result.successes.is_empty());
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].0, "task1");
    }
}
