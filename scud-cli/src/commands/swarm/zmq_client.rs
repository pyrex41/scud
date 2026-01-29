//! ZMQ client module for SCUD swarm monitoring and control
//!
//! This module provides client-side functionality for:
//! - Discovering running swarms via ZMQ endpoint files
//! - Subscribing to swarm events via ZMQ PUB socket
//! - Sending control commands via ZMQ REP socket

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Discovered swarm session with ZMQ endpoints
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub tag: String,
    pub pub_endpoint: String,
    pub rep_endpoint: String,
    pub session_dir: PathBuf,
}

/// ZMQ event format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ZmqEvent {
    SwarmStarted {
        session_id: String,
        tag: String,
        total_waves: usize,
    },
    WaveStarted {
        wave: usize,
        tasks: Vec<String>,
        task_count: usize,
    },
    TaskStarted {
        task_id: String,
    },
    TaskSpawned {
        task_id: String,
    },
    TaskOutput {
        task_id: String,
        text: String,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
        duration_ms: Option<u64>,
    },
    TaskFailed {
        task_id: String,
        reason: String,
    },
    ValidationStarted,
    ValidationCompleted {
        passed: bool,
        output: String,
    },
    ValidationPassed,
    ValidationFailed {
        failures: Vec<String>,
    },
    WaveCompleted {
        wave: usize,
        duration_ms: Option<u64>,
    },
    SwarmCompleted {
        success: bool,
    },
    SwarmPaused,
    SwarmResumed,
    Heartbeat {
        timestamp: String,
    },
    ToolCall {
        task_id: String,
        tool: String,
        input_summary: Option<String>,
    },
    ToolResult {
        task_id: String,
        tool: String,
        success: bool,
        duration_ms: Option<u64>,
    },
    FileRead {
        task_id: String,
        path: String,
    },
    FileWrite {
        task_id: String,
        path: String,
        lines_changed: Option<u32>,
    },
    DependencyMet {
        task_id: String,
        dependency_id: String,
    },
    TaskUnblocked {
        task_id: String,
        by_task_id: String,
    },
    RepairStarted {
        attempt: usize,
        task_ids: Vec<String>,
    },
    RepairCompleted {
        attempt: usize,
        success: bool,
    },
}

/// High-level SCUD event for client applications
#[derive(Debug, Clone)]
pub enum ScudEvent {
    SwarmStarted { tag: String, total_waves: usize },
    WaveStarted { wave: usize, tasks: Vec<String> },
    TaskStarted { task_id: String },
    TaskOutput { task_id: String, text: String },
    TaskCompleted { task_id: String, success: bool },
    ValidationStarted,
    ValidationCompleted { passed: bool, output: String },
    WaveCompleted { wave: usize },
    SwarmCompleted { success: bool },
    SwarmPaused,
    SwarmResumed,
    Heartbeat,
    Output(String),
    Error(String),
}

/// Discover ZMQ endpoints for a specific session directory
pub fn discover_endpoints(session_dir: &Path) -> Option<(String, String)> {
    let pub_path = session_dir.join("zmq-pub.addr");
    let rep_path = session_dir.join("zmq-rep.addr");

    if pub_path.exists() && rep_path.exists() {
        if let (Ok(pub_addr), Ok(rep_addr)) = (
            std::fs::read_to_string(&pub_path),
            std::fs::read_to_string(&rep_path),
        ) {
            Some((pub_addr.trim().to_string(), rep_addr.trim().to_string()))
        } else {
            None
        }
    } else {
        None
    }
}

/// Discover all running swarm sessions
pub fn discover_sessions(project_root: &Path) -> Vec<DiscoveredSession> {
    let swarm_dir = project_root.join(".scud/swarm");
    let mut sessions = vec![];

    if let Ok(entries) = std::fs::read_dir(&swarm_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some((pub_addr, rep_addr)) = discover_endpoints(&path) {
                    // Try to get tag from session name (format: tag-timestamp)
                    let session_id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let tag = session_id
                        .split('-')
                        .next()
                        .unwrap_or("unknown")
                        .to_string();

                    sessions.push(DiscoveredSession {
                        session_id,
                        tag,
                        pub_endpoint: pub_addr,
                        rep_endpoint: rep_addr,
                        session_dir: path,
                    });
                }
            }
        }
    }

    sessions
}

/// ZMQ Swarm Client for event subscription and control
pub struct SwarmClient {
    pub_endpoint: String,
    rep_endpoint: String,
    event_rx: Option<mpsc::Receiver<ScudEvent>>,
}

impl SwarmClient {
    /// Create a new SwarmClient for the given session
    pub fn new(session: &DiscoveredSession) -> Self {
        Self {
            pub_endpoint: session.pub_endpoint.clone(),
            rep_endpoint: session.rep_endpoint.clone(),
            event_rx: None,
        }
    }

    /// Subscribe to swarm events (starts background task)
    pub async fn subscribe(&mut self) -> Result<mpsc::Receiver<ScudEvent>> {
        let (tx, rx) = mpsc::channel(100);

        let pub_endpoint = self.pub_endpoint.clone();

        // Start background task to receive ZMQ events
        tokio::spawn(async move {
            if let Err(e) = Self::event_loop(&pub_endpoint, tx).await {
                tracing::error!("ZMQ event loop error: {}", e);
            }
        });

        self.event_rx = Some(rx);
        Ok(self.event_rx.take().unwrap())
    }

    /// Send a control command to the swarm
    pub async fn send_control_command(&self, command: &str) -> Result<String> {
        let _rep_endpoint = &self.rep_endpoint;

        // TODO: Implement actual REP socket connection
        // For now, return a placeholder response
        match command {
            "pause" => Ok("paused".to_string()),
            "resume" => Ok("resumed".to_string()),
            "stop" => Ok("stopping".to_string()),
            "status" => Ok("running".to_string()),
            _ => Ok(format!("unknown command: {}", command)),
        }
    }

    /// Background event receiving loop
    async fn event_loop(_pub_endpoint: &str, tx: mpsc::Sender<ScudEvent>) -> Result<()> {
        // TODO: Connect to ZMQ PUB socket and receive events
        // This is a placeholder - the actual implementation uses zeromq crate in watch.rs

        // For now, send a heartbeat event to test the channel
        let heartbeat = ScudEvent::Heartbeat;
        let _ = tx.send(heartbeat).await;

        Ok(())
    }
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
            ZmqEvent::TaskStarted { task_id } => ScudEvent::TaskStarted { task_id },
            ZmqEvent::TaskOutput { task_id, text } => ScudEvent::TaskOutput { task_id, text },
            ZmqEvent::TaskCompleted { task_id, success, .. } => {
                ScudEvent::TaskCompleted { task_id, success }
            }
            ZmqEvent::ValidationStarted => ScudEvent::ValidationStarted,
            ZmqEvent::ValidationCompleted { passed, output } => {
                ScudEvent::ValidationCompleted { passed, output }
            }
            ZmqEvent::WaveCompleted { wave, .. } => ScudEvent::WaveCompleted { wave },
            ZmqEvent::SwarmCompleted { success } => ScudEvent::SwarmCompleted { success },
            ZmqEvent::SwarmPaused => ScudEvent::SwarmPaused,
            ZmqEvent::SwarmResumed => ScudEvent::SwarmResumed,
            ZmqEvent::Heartbeat { .. } => ScudEvent::Heartbeat,
            _ => ScudEvent::Output(format!("{:?}", event)),
        }
    }
}