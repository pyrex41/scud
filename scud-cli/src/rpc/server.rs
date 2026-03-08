//! JSON RPC server for IPC communication
//!
//! Handles stdin/stdout communication with JSON RPC protocol.

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::commands::spawn::terminal::Harness;
use crate::extensions::runner::{spawn_agent, AgentEvent, SpawnConfig};

use super::types::*;

/// RPC Server configuration
#[derive(Debug, Clone)]
pub struct RpcServerConfig {
    /// Default working directory for agents
    pub working_dir: PathBuf,
    /// Default harness to use
    pub default_harness: Harness,
    /// Default model to use
    pub default_model: Option<String>,
    /// Project root for task storage access
    pub project_root: Option<PathBuf>,
}

impl Default for RpcServerConfig {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            default_harness: Harness::default(),
            default_model: None,
            project_root: None,
        }
    }
}

/// JSON RPC Server for subagent communication
pub struct RpcServer {
    config: RpcServerConfig,
    /// Active agent task handles
    active_agents: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    /// Event channel for agent events
    event_tx: mpsc::Sender<AgentEvent>,
    event_rx: mpsc::Receiver<AgentEvent>,
}

impl RpcServer {
    /// Create a new RPC server with the given configuration
    pub fn new(config: RpcServerConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        Self {
            config,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            event_rx,
        }
    }

    /// Run the RPC server loop
    ///
    /// Reads JSON RPC requests from stdin and writes responses/events to stdout.
    /// This function blocks until a shutdown request is received or stdin closes.
    pub async fn run(&mut self) -> Result<()> {
        // Emit server ready notification
        self.emit_notification(RpcNotification::server_ready(env!("CARGO_PKG_VERSION")))?;

        let stdin = io::stdin();
        let reader = stdin.lock();

        // Spawn a task to forward agent events to stdout
        let event_rx = std::mem::replace(&mut self.event_rx, mpsc::channel(1).1);
        let event_forwarder = tokio::spawn(Self::forward_events(event_rx));

        // Read requests from stdin line by line
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error reading stdin: {}", e);
                    break;
                }
            };

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse the request
            let request: RpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    // Try to extract ID for error response
                    let id = Self::extract_id(&line).unwrap_or(RpcId::Null);
                    self.emit_response(RpcResponse::error(
                        id,
                        RpcError::parse_error(&e.to_string()),
                    ))?;
                    continue;
                }
            };

            // Validate JSON RPC version
            if request.jsonrpc != JSONRPC_VERSION {
                if let Some(id) = request.id.clone() {
                    self.emit_response(RpcResponse::error(
                        id,
                        RpcError::invalid_request("Invalid JSON-RPC version"),
                    ))?;
                }
                continue;
            }

            // Handle the request
            let should_shutdown = self.handle_request(request).await?;
            if should_shutdown {
                break;
            }
        }

        // Emit shutdown notification
        self.emit_notification(RpcNotification::server_shutdown())?;

        // Cancel the event forwarder
        event_forwarder.abort();

        Ok(())
    }

    /// Handle a single RPC request
    async fn handle_request(&mut self, request: RpcRequest) -> Result<bool> {
        let id = request.id.clone();
        let method = request.method.as_str();

        match method {
            "ping" => {
                if let Some(id) = id {
                    self.emit_response(RpcResponse::success(
                        id,
                        serde_json::json!({"pong": true}),
                    ))?;
                }
            }

            "shutdown" => {
                if let Some(id) = id {
                    self.emit_response(RpcResponse::success(
                        id,
                        serde_json::json!({"status": "shutting_down"}),
                    ))?;
                }
                return Ok(true);
            }

            "spawn" => {
                let result = self.handle_spawn(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "spawn_task" => {
                let result = self.handle_spawn_task(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "cancel" => {
                let result = self.handle_cancel(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "list_agents" => {
                let result = self.handle_list_agents().await;
                if let Some(id) = id {
                    self.emit_response(RpcResponse::success(id, result))?;
                }
            }

            "list_tasks" => {
                let result = self.handle_list_tasks(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "get_task" => {
                let result = self.handle_get_task(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "set_status" => {
                let result = self.handle_set_status(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            "next_task" => {
                let result = self.handle_next_task(request.params).await;
                if let Some(id) = id {
                    match result {
                        Ok(value) => self.emit_response(RpcResponse::success(id, value))?,
                        Err(e) => self.emit_response(RpcResponse::error(id, e))?,
                    }
                }
            }

            _ => {
                if let Some(id) = id {
                    self.emit_response(RpcResponse::error(id, RpcError::method_not_found(method)))?;
                }
            }
        }

        Ok(false)
    }

    /// Handle "spawn" method - spawn agent with custom prompt
    async fn handle_spawn(&self, params: Value) -> Result<Value, RpcError> {
        let params: SpawnParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

        // Parse harness
        let harness = if let Some(h) = params.harness {
            Harness::parse(&h).map_err(|e| RpcError::invalid_params(&e.to_string()))?
        } else {
            self.config.default_harness
        };

        // Determine working directory
        let working_dir = params
            .working_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config.working_dir.clone());

        let config = SpawnConfig {
            task_id: params.task_id.clone(),
            prompt: params.prompt,
            working_dir,
            harness,
            model: params.model.or_else(|| self.config.default_model.clone()),
        };

        // Spawn the agent
        let event_tx = self.event_tx.clone();
        let task_id = params.task_id.clone();

        match spawn_agent(config, event_tx).await {
            Ok(handle) => {
                // Track the agent handle
                let mut agents = self.active_agents.lock().await;
                agents.insert(
                    task_id.clone(),
                    tokio::spawn(async move {
                        let _ = handle.await;
                    }),
                );

                Ok(serde_json::json!({
                    "status": "spawned",
                    "task_id": task_id
                }))
            }
            Err(e) => Err(RpcError::spawn_failed(&e.to_string())),
        }
    }

    /// Handle "spawn_task" method - spawn agent for a task in the graph
    async fn handle_spawn_task(&self, params: Value) -> Result<Value, RpcError> {
        let params: SpawnTaskParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

        // Load task from storage
        let storage = crate::storage::Storage::new(self.config.project_root.clone());

        // Get the tag
        let tag = params
            .tag
            .or_else(|| storage.get_active_group().ok().flatten());
        let tag =
            tag.ok_or_else(|| RpcError::invalid_params("No tag specified and no active tag"))?;

        // Load the task group
        let group = storage
            .load_group(&tag)
            .map_err(|e: anyhow::Error| RpcError::task_not_found(&e.to_string()))?;

        // Find the task
        let task = group
            .tasks
            .iter()
            .find(|t| t.id == params.task_id)
            .ok_or_else(|| RpcError::task_not_found(&params.task_id))?;

        // Generate prompt for the task
        let prompt = crate::commands::spawn::agent::generate_prompt(task, &tag);

        // Parse harness (prefer params, then task's agent_type, then default)
        let harness = if let Some(h) = params.harness {
            Harness::parse(&h).map_err(|e| RpcError::invalid_params(&e.to_string()))?
        } else if let Some(agent_type) = &task.agent_type {
            // Try to load from agent definition
            if let Some(agent_def) =
                crate::agents::AgentDef::try_load(agent_type, &self.config.working_dir)
            {
                agent_def.harness().unwrap_or(self.config.default_harness)
            } else {
                self.config.default_harness
            }
        } else {
            self.config.default_harness
        };

        // Get model
        let model = params.model.or_else(|| {
            if let Some(agent_type) = &task.agent_type {
                if let Some(agent_def) =
                    crate::agents::AgentDef::try_load(agent_type, &self.config.working_dir)
                {
                    return agent_def.model().map(String::from);
                }
            }
            self.config.default_model.clone()
        });

        let config = SpawnConfig {
            task_id: params.task_id.clone(),
            prompt,
            working_dir: self.config.working_dir.clone(),
            harness,
            model,
        };

        // Spawn the agent
        let event_tx = self.event_tx.clone();
        let task_id = params.task_id.clone();

        match spawn_agent(config, event_tx).await {
            Ok(handle) => {
                // Track the agent handle
                let mut agents = self.active_agents.lock().await;
                agents.insert(
                    task_id.clone(),
                    tokio::spawn(async move {
                        let _ = handle.await;
                    }),
                );

                Ok(serde_json::json!({
                    "status": "spawned",
                    "task_id": task_id,
                    "tag": tag
                }))
            }
            Err(e) => Err(RpcError::spawn_failed(&e.to_string())),
        }
    }

    /// Handle "cancel" method - cancel a running agent
    async fn handle_cancel(&self, params: Value) -> Result<Value, RpcError> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::invalid_params("Missing task_id"))?;

        let mut agents = self.active_agents.lock().await;

        if let Some(handle) = agents.remove(task_id) {
            handle.abort();
            Ok(serde_json::json!({
                "status": "cancelled",
                "task_id": task_id
            }))
        } else {
            Err(RpcError::task_not_found(task_id))
        }
    }

    /// Handle "list_agents" method - list currently active agents
    async fn handle_list_agents(&self) -> Value {
        let agents = self.active_agents.lock().await;
        let agent_ids: Vec<&String> = agents.keys().collect();
        serde_json::json!({
            "agents": agent_ids,
            "count": agent_ids.len()
        })
    }

    /// Handle "list_tasks" method - list tasks from storage
    async fn handle_list_tasks(&self, params: Value) -> Result<Value, RpcError> {
        let params: ListTasksParams = serde_json::from_value(params).unwrap_or_default();

        let storage = crate::storage::Storage::new(self.config.project_root.clone());

        let tag = params
            .tag
            .or_else(|| storage.get_active_group().ok().flatten());
        let tag =
            tag.ok_or_else(|| RpcError::invalid_params("No tag specified and no active tag"))?;

        let group = storage
            .load_group(&tag)
            .map_err(|e: anyhow::Error| RpcError::internal_error(&e.to_string()))?;

        let tasks: Vec<Value> = group
            .tasks
            .iter()
            .filter(|t| {
                params
                    .status
                    .as_ref()
                    .map(|s| t.status.as_str().to_lowercase() == s.to_lowercase())
                    .unwrap_or(true)
            })
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "status": t.status.as_str(),
                    "complexity": t.complexity,
                    "dependencies": t.dependencies
                })
            })
            .collect();

        Ok(serde_json::json!({
            "tag": tag,
            "tasks": tasks,
            "count": tasks.len()
        }))
    }

    /// Handle "get_task" method - get a single task
    async fn handle_get_task(&self, params: Value) -> Result<Value, RpcError> {
        let params: GetTaskParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

        let storage = crate::storage::Storage::new(self.config.project_root.clone());

        let tag = params
            .tag
            .or_else(|| storage.get_active_group().ok().flatten());
        let tag =
            tag.ok_or_else(|| RpcError::invalid_params("No tag specified and no active tag"))?;

        let group = storage
            .load_group(&tag)
            .map_err(|e: anyhow::Error| RpcError::internal_error(&e.to_string()))?;

        let task = group
            .tasks
            .iter()
            .find(|t| t.id == params.task_id)
            .ok_or_else(|| RpcError::task_not_found(&params.task_id))?;

        Ok(serde_json::json!({
            "id": task.id,
            "title": task.title,
            "description": task.description,
            "status": task.status.as_str(),
            "complexity": task.complexity,
            "priority": format!("{:?}", task.priority).to_lowercase(),
            "dependencies": task.dependencies,
            "agent_type": task.agent_type,
            "assigned_to": task.assigned_to
        }))
    }

    /// Handle "set_status" method - update task status
    async fn handle_set_status(&self, params: Value) -> Result<Value, RpcError> {
        let params: SetStatusParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

        let storage = crate::storage::Storage::new(self.config.project_root.clone());

        let tag = params
            .tag
            .or_else(|| storage.get_active_group().ok().flatten());
        let tag =
            tag.ok_or_else(|| RpcError::invalid_params("No tag specified and no active tag"))?;

        let mut group = storage
            .load_group(&tag)
            .map_err(|e: anyhow::Error| RpcError::internal_error(&e.to_string()))?;

        // Find and update the task
        let task = group
            .tasks
            .iter_mut()
            .find(|t| t.id == params.task_id)
            .ok_or_else(|| RpcError::task_not_found(&params.task_id))?;

        // Parse status
        let new_status =
            crate::models::task::TaskStatus::from_str(&params.status).ok_or_else(|| {
                RpcError::invalid_params(&format!("Invalid status: {}", params.status))
            })?;

        let old_status = task.status.as_str().to_string();
        task.status = new_status;

        // Save the group
        storage
            .update_group(&tag, &group)
            .map_err(|e: anyhow::Error| RpcError::internal_error(&e.to_string()))?;

        Ok(serde_json::json!({
            "task_id": params.task_id,
            "old_status": old_status,
            "new_status": params.status
        }))
    }

    /// Handle "next_task" method - get next available task
    async fn handle_next_task(&self, params: Value) -> Result<Value, RpcError> {
        let params: NextTaskParams = serde_json::from_value(params).unwrap_or_default();

        let storage = crate::storage::Storage::new(self.config.project_root.clone());

        // Use next command logic
        let result = crate::commands::helpers::find_next_task(
            &storage,
            params.tag.as_deref(),
            params.all_tags,
        );

        match result {
            Some((task, tag)) => Ok(serde_json::json!({
                "task_id": task.id,
                "title": task.title,
                "tag": tag,
                "complexity": task.complexity
            })),
            None => Ok(serde_json::json!({
                "task_id": null,
                "message": "No tasks available"
            })),
        }
    }

    /// Emit a JSON RPC response to stdout
    fn emit_response(&self, response: RpcResponse) -> Result<()> {
        let json = serde_json::to_string(&response)?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
        Ok(())
    }

    /// Emit a JSON RPC notification to stdout
    fn emit_notification(&self, notification: RpcNotification) -> Result<()> {
        let json = serde_json::to_string(&notification)?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
        Ok(())
    }

    /// Try to extract ID from malformed JSON for error responses
    fn extract_id(json_str: &str) -> Option<RpcId> {
        // Simple heuristic: try to parse just enough to get the id
        let parsed: Result<Value, _> = serde_json::from_str(json_str);
        if let Ok(v) = parsed {
            if let Some(id) = v.get("id") {
                if let Some(n) = id.as_i64() {
                    return Some(RpcId::Number(n));
                }
                if let Some(s) = id.as_str() {
                    return Some(RpcId::String(s.to_string()));
                }
            }
        }
        None
    }

    /// Forward agent events to stdout as JSON RPC notifications
    async fn forward_events(mut event_rx: mpsc::Receiver<AgentEvent>) {
        while let Some(event) = event_rx.recv().await {
            let notification = match event {
                AgentEvent::Started { task_id } => RpcNotification::agent_started(&task_id),
                AgentEvent::Output { task_id, line } => {
                    RpcNotification::agent_output(&task_id, &line)
                }
                AgentEvent::Completed { result } => RpcNotification::agent_completed(
                    &result.task_id,
                    result.success,
                    result.exit_code,
                    result.duration_ms,
                ),
                AgentEvent::SpawnFailed { task_id, error } => {
                    RpcNotification::agent_spawn_failed(&task_id, &error)
                }
            };

            // Write to stdout
            if let Ok(json) = serde_json::to_string(&notification) {
                let mut stdout = io::stdout().lock();
                let _ = writeln!(stdout, "{}", json);
                let _ = stdout.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = RpcServerConfig::default();
        assert_eq!(config.default_harness, Harness::default());
    }

    #[test]
    fn test_extract_id_number() {
        let id = RpcServer::extract_id(r#"{"id": 42, "invalid": true}"#);
        assert_eq!(id, Some(RpcId::Number(42)));
    }

    #[test]
    fn test_extract_id_string() {
        let id = RpcServer::extract_id(r#"{"id": "abc-123"}"#);
        assert_eq!(id, Some(RpcId::String("abc-123".to_string())));
    }
}
