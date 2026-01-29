//! ZeroMQ event publisher for real-time swarm monitoring
//!
//! Publishes swarm events via PUB socket and accepts control commands via REP socket.
//! Clients can discover socket addresses via files in `.scud/swarm/<session>/`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(feature = "zmq")]
use zmq;

/// Discovered swarm session with ZMQ endpoints
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub tag: String,
    pub pub_endpoint: String,
    pub rep_endpoint: String,
    pub session_dir: std::path::PathBuf,
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

/// ZMQ Publisher for swarm events
#[cfg(feature = "zmq")]
pub struct EventPublisher {
    pub_socket: zmq::Socket,
    rep_socket: zmq::Socket,
    pub_endpoint: String,
    rep_endpoint: String,
    session_dir: PathBuf,
    _context: zmq::Context, // Keep context alive
}

impl EventPublisher {
    /// Create and bind publisher sockets
    ///
    /// Binds to dynamic ports on localhost and writes addresses to discovery files.
    pub fn new(session_dir: &Path) -> Result<Self> {
        let context = zmq::Context::new();

        // Create PUB socket for events
        let pub_socket = context.socket(zmq::PUB)?;
        let pub_endpoint = pub_socket.bind("tcp://127.0.0.1:0")?;
        tracing::info!("ZMQ PUB bound to {}", pub_endpoint);

        // Create REP socket for control
        let rep_socket = context.socket(zmq::REP)?;
        let rep_endpoint = rep_socket.bind("tcp://127.0.0.1:0")?;
        tracing::info!("ZMQ REP bound to {}", rep_endpoint);

        let publisher = Self {
            pub_socket,
            rep_socket,
            pub_endpoint: pub_endpoint.clone(),
            rep_endpoint: rep_endpoint.clone(),
            session_dir: session_dir.to_path_buf(),
            _context: context,
        };

        // Write discovery files
        publisher.write_discovery_files()?;

        Ok(publisher)
    }

    /// Write socket addresses to discovery files
    fn write_discovery_files(&self) -> Result<()> {
        std::fs::create_dir_all(&self.session_dir)?;

        let pub_path = self.session_dir.join("zmq-pub.addr");
        std::fs::write(&pub_path, &self.pub_endpoint)?;
        tracing::debug!("Wrote PUB address to {:?}", pub_path);

        let rep_path = self.session_dir.join("zmq-rep.addr");
        std::fs::write(&rep_path, &self.rep_endpoint)?;
        tracing::debug!("Wrote REP address to {:?}", rep_path);

        Ok(())
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: &ZmqEvent) -> Result<()> {
        let json = serde_json::to_string(event)?;
        self.pub_socket.send(&json, 0)?;
        Ok(())
    }

    /// Get the REP socket for control command handling
    pub fn rep_socket(&self) -> &zmq::Socket {
        &self.rep_socket
    }

    /// Get the PUB endpoint address
    pub fn pub_endpoint(&self) -> &str {
        &self.pub_endpoint
    }

    /// Get the REP endpoint address
    pub fn rep_endpoint(&self) -> &str {
        &self.rep_endpoint
    }

    /// Handle REP socket requests (control commands)
    pub fn handle_control_request(&self) -> Result<Option<String>> {
        // Try to receive a request (non-blocking)
        match self.rep_socket.recv_string(0) {
            Ok(Ok(request)) => {
                let response = self.process_control_request(&request);
                self.rep_socket.send(&response, 0)?;
                Ok(Some(request))
            }
            Ok(Err(_)) => {
                // Received non-UTF8 data, ignore
                Ok(None)
            }
            Err(zmq::Error::EAGAIN) => {
                // No message available
                Ok(None)
            }
            Err(e) => Err(anyhow::anyhow!("REP socket error: {}", e)),
        }
    }

    /// Process a control command request
    fn process_control_request(&self, request: &str) -> String {
        match request.trim() {
            "pause" => {
                // TODO: This should set a pause flag that the swarm loop checks
                "paused".to_string()
            }
            "resume" => {
                // TODO: This should clear the pause flag
                "resumed".to_string()
            }
            "stop" => {
                // TODO: This should set a stop flag
                "stopping".to_string()
            }
            "status" => {
                // TODO: Return current swarm status
                "running".to_string()
            }
            _ => {
                format!("unknown command: {}", request)
            }
        }
    }

    /// Clean up discovery files on shutdown
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(self.session_dir.join("zmq-pub.addr"));
        let _ = std::fs::remove_file(self.session_dir.join("zmq-rep.addr"));
    }
}

#[cfg(feature = "zmq")]
#[cfg(feature = "zmq")]
impl Drop for EventPublisher {
    fn drop(&mut self) {
        self.cleanup();
    }
}
