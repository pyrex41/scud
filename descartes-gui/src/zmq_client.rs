//! ZeroMQ client for connecting to running SCUD swarms
//!
//! Discovers swarm ZMQ endpoints via discovery files and provides
//! event subscription and control command interfaces.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use zeromq::{Socket, SocketRecv, SocketSend, SubSocket, ReqSocket};

use crate::scud_bridge::ScudEvent;
use scud::commands::swarm::events::{AgentEvent, EventKind};
use scud::db;

/// Discovered swarm session
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub tag: String,
    pub pub_endpoint: String,
    pub rep_endpoint: String,
    pub session_dir: PathBuf,
}

/// Discover running swarm sessions
pub fn discover_sessions(project_root: &Path) -> Vec<DiscoveredSession> {
    let swarm_dir = project_root.join(".scud/swarm");
    let mut sessions = vec![];

    if let Ok(entries) = std::fs::read_dir(&swarm_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let pub_path = path.join("zmq-pub.addr");
                let rep_path = path.join("zmq-rep.addr");

                if pub_path.exists() && rep_path.exists() {
                    if let (Ok(pub_addr), Ok(rep_addr)) = (
                        std::fs::read_to_string(&pub_path),
                        std::fs::read_to_string(&rep_path),
                    ) {
                        // Try to get tag from session name (format: tag-timestamp)
                        let session_id = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let tag = session_id.split('-').next()
                            .unwrap_or("unknown")
                            .to_string();

                        sessions.push(DiscoveredSession {
                            session_id,
                            tag,
                            pub_endpoint: pub_addr.trim().to_string(),
                            rep_endpoint: rep_addr.trim().to_string(),
                            session_dir: path,
                        });
                    }
                }
            }
        }
    }

    sessions
}

/// ZMQ event format (must match swarm publisher)
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ZmqEvent {
    SwarmStarted { session_id: String, tag: String, total_waves: usize },
    WaveStarted { wave: usize, tasks: Vec<String>, task_count: usize },
    TaskStarted { task_id: String },
    TaskSpawned { task_id: String },
    TaskOutput { task_id: String, text: String },
    TaskCompleted { task_id: String, success: bool, duration_ms: Option<u64> },
    TaskFailed { task_id: String, reason: String },
    ValidationStarted,
    ValidationCompleted { passed: bool, output: String },
    ValidationPassed,
    ValidationFailed { failures: Vec<String> },
    WaveCompleted { wave: usize, duration_ms: Option<u64> },
    SwarmCompleted { success: bool },
    SwarmPaused,
    SwarmResumed,
    Heartbeat { timestamp: String },
    ToolCall { task_id: String, tool: String, input_summary: Option<String> },
    ToolResult { task_id: String, tool: String, success: bool, duration_ms: Option<u64> },
    FileRead { task_id: String, path: String },
    FileWrite { task_id: String, path: String, lines_changed: Option<u32> },
    DependencyMet { task_id: String, dependency_id: String },
    TaskUnblocked { task_id: String, by_task_id: String },
    RepairStarted { attempt: usize, task_ids: Vec<String> },
    RepairCompleted { attempt: usize, success: bool },
}

impl From<ZmqEvent> for ScudEvent {
    fn from(event: ZmqEvent) -> Self {
        match event {
            ZmqEvent::SwarmStarted { tag, total_waves, .. } => {
                ScudEvent::SwarmStarted { tag, total_waves }
            }
            ZmqEvent::WaveStarted { wave, tasks, .. } => {
                ScudEvent::WaveStarted { wave, tasks }
            }
            ZmqEvent::TaskStarted { task_id } => {
                ScudEvent::TaskStarted { task_id }
            }
            ZmqEvent::TaskSpawned { task_id } => {
                ScudEvent::TaskSpawned { task_id }
            }
            ZmqEvent::TaskOutput { task_id, text } => {
                ScudEvent::TaskOutput { task_id, text }
            }
            ZmqEvent::TaskCompleted { task_id, success, .. } => {
                ScudEvent::TaskCompleted { task_id, success }
            }
            ZmqEvent::TaskFailed { task_id, reason } => {
                ScudEvent::TaskFailed { task_id, reason }
            }
            ZmqEvent::ValidationStarted => ScudEvent::ValidationStarted,
            ZmqEvent::ValidationCompleted { passed, output } => {
                ScudEvent::ValidationCompleted { passed, output }
            }
            ZmqEvent::ValidationPassed => ScudEvent::ValidationPassed,
            ZmqEvent::ValidationFailed { failures } => {
                ScudEvent::ValidationFailed { failures }
            }
            ZmqEvent::WaveCompleted { wave, .. } => {
                ScudEvent::WaveCompleted { wave }
            }
            ZmqEvent::SwarmCompleted { success } => {
                ScudEvent::SwarmCompleted { success }
            }
            ZmqEvent::SwarmPaused => ScudEvent::SwarmPaused,
            ZmqEvent::SwarmResumed => ScudEvent::SwarmResumed,
            ZmqEvent::Heartbeat { timestamp } => ScudEvent::Heartbeat { timestamp },
            ZmqEvent::ToolCall { task_id, tool, input_summary } => {
                ScudEvent::ToolCall { task_id, tool, input_summary }
            }
            ZmqEvent::ToolResult { task_id, tool, success, duration_ms } => {
                ScudEvent::ToolResult { task_id, tool, success, duration_ms }
            }
            ZmqEvent::FileRead { task_id, path } => {
                ScudEvent::FileRead { task_id, path }
            }
            ZmqEvent::FileWrite { task_id, path, lines_changed } => {
                ScudEvent::FileWrite { task_id, path, lines_changed }
            }
            ZmqEvent::DependencyMet { task_id, dependency_id } => {
                ScudEvent::DependencyMet { task_id, dependency_id }
            }
            ZmqEvent::TaskUnblocked { task_id, by_task_id } => {
                ScudEvent::TaskUnblocked { task_id, by_task_id }
            }
            ZmqEvent::RepairStarted { attempt, task_ids } => {
                ScudEvent::RepairStarted { attempt, task_ids }
            }
            ZmqEvent::RepairCompleted { attempt, success } => {
                ScudEvent::RepairCompleted { attempt, success }
            }
        }
    }
}

/// Convert database AgentEvent to GUI ScudEvent
pub fn convert_db_event_to_scud_event(event: AgentEvent) -> ScudEvent {
    match event.event {
        EventKind::Spawned => ScudEvent::TaskStarted { task_id: event.task_id },
        EventKind::Started => ScudEvent::TaskStarted { task_id: event.task_id },
        EventKind::Completed { success, .. } => {
            ScudEvent::TaskCompleted { task_id: event.task_id, success }
        }
        EventKind::Failed { .. } => {
            ScudEvent::TaskCompleted { task_id: event.task_id, success: false }
        }
        EventKind::ToolCall { tool, .. } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("Tool call: {}", tool) }
        }
        EventKind::ToolResult { success, .. } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("Tool result: {}", if success { "success" } else { "failed" }) }
        }
        EventKind::FileRead { path } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("File read: {}", path) }
        }
        EventKind::FileWrite { path, lines_changed } => {
            let lines = lines_changed.map_or("".to_string(), |l| format!(" ({} lines)", l));
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("File write: {}{}", path, lines) }
        }
        EventKind::DependencyMet { dependency_id } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("Dependency met: {}", dependency_id) }
        }
        EventKind::Unblocked { by_task_id } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("Unblocked by: {}", by_task_id) }
        }
        EventKind::Output { line } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: line }
        }
        EventKind::WaveStarted { wave_number, task_count } => {
            ScudEvent::WaveStarted { wave: wave_number, tasks: vec![] } // Tasks would need to be populated from context
        }
        EventKind::WaveCompleted { wave_number, .. } => {
            ScudEvent::WaveCompleted { wave: wave_number }
        }
        EventKind::ValidationPassed => {
            ScudEvent::ValidationCompleted { passed: true, output: "Validation passed".to_string() }
        }
        EventKind::ValidationFailed { failures } => {
            ScudEvent::ValidationCompleted { passed: false, output: failures.join(", ") }
        }
        EventKind::RepairStarted { attempt, .. } => {
            ScudEvent::ValidationStarted // Map to validation started for simplicity
        }
        EventKind::RepairCompleted { success, .. } => {
            ScudEvent::ValidationCompleted { passed: success, output: format!("Repair {}", if success { "succeeded" } else { "failed" }) }
        }
        EventKind::Heartbeat => {
            ScudEvent::Output("".to_string()) // Ignore heartbeats
        }
        EventKind::Custom { name, .. } => {
            ScudEvent::TaskOutput { task_id: event.task_id, text: format!("Custom event: {}", name) }
        }
    }
}

/// Control commands
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Pause,
    Resume,
    Stop,
    Status,
}

/// Control response
#[derive(Debug, Clone, Deserialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
}

/// ZMQ client for a specific swarm session
pub struct SwarmClient {
    session: DiscoveredSession,
    event_rx: Option<mpsc::Receiver<ScudEvent>>,
    subscriber_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SwarmClient {
    /// Connect to a discovered swarm session
    pub fn connect(session: DiscoveredSession) -> Result<Self> {
        Ok(Self {
            session,
            event_rx: None,
            subscriber_handle: None,
        })
    }

    /// Start receiving events
    pub async fn subscribe(&mut self) -> Result<mpsc::Receiver<ScudEvent>> {
        let (tx, rx) = mpsc::channel(1000);
        let endpoint = self.session.pub_endpoint.clone();

        let handle = tokio::spawn(async move {
            let mut socket = SubSocket::new();
            if let Err(e) = socket.connect(&endpoint).await {
                tracing::error!("Failed to connect SUB socket: {}", e);
                return;
            }

            // Subscribe to all messages
            if let Err(e) = socket.subscribe("").await {
                tracing::error!("Failed to subscribe: {}", e);
                return;
            }

            tracing::info!("Connected to swarm events at {}", endpoint);

            loop {
                match socket.recv().await {
                    Ok(msg) => {
                        if let Ok(text) = std::str::from_utf8(&msg) {
                            if let Ok(event) = serde_json::from_str::<ZmqEvent>(text) {
                                let scud_event: ScudEvent = event.into();
                                if tx.send(scud_event).await.is_err() {
                                    break; // Channel closed
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("SUB socket error: {}", e);
                        break;
                    }
                }
            }
        });

        self.subscriber_handle = Some(handle);
        self.event_rx = Some(rx.clone());

        Ok(rx)
    }

    /// Fetch historical events from database and emit them
    pub async fn catch_up(&self, event_tx: &tokio::sync::mpsc::Sender<ScudEvent>) -> Result<()> {
        // Use tokio::task::spawn_blocking for database operations
        let session_id = self.session.session_id.clone();
        let events = tokio::task::spawn_blocking(move || -> Result<Vec<AgentEvent>> {
            let db = db::Database::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            db.initialize()?;
            let guard = db.connection()?;
            db::events::get_events_for_session(&guard, &session_id)
        })
        .await??;

        // Convert and emit events in chronological order
        for db_event in events {
            let scud_event = convert_db_event_to_scud_event(db_event);
            if event_tx.send(scud_event).await.is_err() {
                break; // Channel closed
            }
        }

        Ok(())
    }

    /// Send a control command
    pub async fn send_command(&self, command: ControlCommand) -> Result<ControlResponse> {
        let mut socket = ReqSocket::new();
        socket.connect(&self.session.rep_endpoint).await?;

        let json = serde_json::to_string(&command)?;
        socket.send(json.into()).await?;

        let response = socket.recv().await?;
        let response: ControlResponse = serde_json::from_slice(&response)?;

        Ok(response)
    }

    /// Pause the swarm
    pub async fn pause(&self) -> Result<ControlResponse> {
        self.send_command(ControlCommand::Pause).await
    }

    /// Resume the swarm
    pub async fn resume(&self) -> Result<ControlResponse> {
        self.send_command(ControlCommand::Resume).await
    }

    /// Stop the swarm
    pub async fn stop(&self) -> Result<ControlResponse> {
        self.send_command(ControlCommand::Stop).await
    }

    /// Get session info
    pub fn session(&self) -> &DiscoveredSession {
        &self.session
    }
}

impl Drop for SwarmClient {
    fn drop(&mut self) {
        if let Some(handle) = self.subscriber_handle.take() {
            handle.abort();
        }
    }
}