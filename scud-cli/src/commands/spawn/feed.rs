//! Socket feed for live monitor output using ZeroMQ
//!
//! Exposes monitor state via ZMQ PUB socket for external consumers.
//! Supports both TCP (remote) and IPC (local) bindings.
//!
//! ## Topics
//! - `session` - Full session state snapshots
//! - `agent` - Individual agent status updates
//! - `output` - Live terminal output lines
//! - `wave` - Wave/task progress updates
//! - `stats` - Aggregate statistics
//!
//! ## Usage
//! ```bash
//! # Start monitor with socket feed
//! scud monitor --session my-session --feed tcp://*:5555
//!
//! # Or with IPC for local consumers
//! scud monitor --session my-session --feed ipc:///tmp/scud-monitor.sock
//! ```
//!
//! ## Subscribing (Python example)
//! ```python
//! import zmq
//! context = zmq.Context()
//! socket = context.socket(zmq.SUB)
//! socket.connect("tcp://localhost:5555")
//! socket.setsockopt_string(zmq.SUBSCRIBE, "")  # Subscribe to all topics
//!
//! while True:
//!     topic, message = socket.recv_multipart()
//!     data = json.loads(message)
//!     print(f"{topic}: {data}")
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use zeromq::{Socket, SocketSend};

use super::monitor::{AgentState, AgentStatus, SpawnSession, SpawnStats};

// ============================================================================
// Feed Message Types
// ============================================================================

/// Topic prefixes for message filtering
pub mod topics {
    pub const SESSION: &str = "session";
    pub const AGENT: &str = "agent";
    pub const OUTPUT: &str = "output";
    pub const WAVE: &str = "wave";
    pub const STATS: &str = "stats";
    pub const HEARTBEAT: &str = "heartbeat";
}

/// Full session state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_name: String,
    pub tag: String,
    pub terminal: String,
    pub created_at: String,
    pub working_dir: String,
    pub agents: Vec<AgentSnapshot>,
    pub stats: StatsSnapshot,
    pub timestamp: String,
}

/// Agent state in feed messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub task_id: String,
    pub task_title: String,
    pub window_name: String,
    pub status: String,
    pub started_at: String,
    pub tag: String,
}

impl From<&AgentState> for AgentSnapshot {
    fn from(agent: &AgentState) -> Self {
        Self {
            task_id: agent.task_id.clone(),
            task_title: agent.task_title.clone(),
            window_name: agent.window_name.clone(),
            status: format!("{:?}", agent.status).to_lowercase(),
            started_at: agent.started_at.clone(),
            tag: agent.tag.clone(),
        }
    }
}

/// Individual agent status update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub task_id: String,
    pub status: String,
    pub previous_status: Option<String>,
    pub timestamp: String,
}

/// Live terminal output message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMessage {
    pub task_id: String,
    pub lines: Vec<String>,
    pub line_count: usize,
    pub timestamp: String,
}

/// Wave/task progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveUpdate {
    pub waves: Vec<WaveSnapshot>,
    pub ready_count: usize,
    pub running_count: usize,
    pub done_count: usize,
    pub blocked_count: usize,
    pub timestamp: String,
}

/// Single wave snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveSnapshot {
    pub number: usize,
    pub tasks: Vec<TaskSnapshot>,
}

/// Task state in wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub title: String,
    pub state: String,
    pub complexity: u32,
}

/// Aggregate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub session_name: String,
    pub tag: String,
    pub total_agents: usize,
    pub starting: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub timestamp: String,
}

impl From<&SpawnStats> for StatsSnapshot {
    fn from(stats: &SpawnStats) -> Self {
        Self {
            session_name: stats.session_name.clone(),
            tag: stats.tag.clone(),
            total_agents: stats.total_agents,
            starting: stats.starting,
            running: stats.running,
            completed: stats.completed,
            failed: stats.failed,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Heartbeat message (sent periodically to confirm feed is alive)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub session_name: String,
    pub uptime_secs: u64,
    pub message_count: u64,
    pub timestamp: String,
}

// ============================================================================
// Feed Messages (Internal)
// ============================================================================

/// Messages sent to the feed publisher task
#[derive(Debug)]
pub enum FeedMessage {
    Session(SessionSnapshot),
    AgentUpdate(AgentUpdate),
    Output(OutputMessage),
    WaveUpdate(WaveUpdate),
    Stats(StatsSnapshot),
    Heartbeat(Heartbeat),
    Shutdown,
}

// ============================================================================
// Feed Publisher
// ============================================================================

/// Configuration for the socket feed
#[derive(Debug, Clone)]
pub struct FeedConfig {
    /// ZMQ endpoint to bind to (e.g., "tcp://*:5555" or "ipc:///tmp/scud.sock")
    pub endpoint: String,
    /// Enable heartbeat messages
    pub heartbeat: bool,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            endpoint: "tcp://*:5555".to_string(),
            heartbeat: true,
            heartbeat_interval_secs: 5,
        }
    }
}

impl FeedConfig {
    /// Create config for TCP endpoint
    pub fn tcp(port: u16) -> Self {
        Self {
            endpoint: format!("tcp://*:{}", port),
            ..Default::default()
        }
    }

    /// Create config for IPC endpoint
    pub fn ipc(path: &str) -> Self {
        Self {
            endpoint: format!("ipc://{}", path),
            ..Default::default()
        }
    }

    /// Parse endpoint from string (auto-detect TCP vs IPC)
    pub fn from_endpoint(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            ..Default::default()
        }
    }
}

/// Handle to send messages to the feed
#[derive(Clone)]
pub struct FeedHandle {
    tx: mpsc::Sender<FeedMessage>,
}

impl FeedHandle {
    /// Publish a session snapshot
    pub async fn publish_session(&self, snapshot: SessionSnapshot) {
        let _ = self.tx.send(FeedMessage::Session(snapshot)).await;
    }

    /// Publish an agent status update
    pub async fn publish_agent_update(&self, update: AgentUpdate) {
        let _ = self.tx.send(FeedMessage::AgentUpdate(update)).await;
    }

    /// Publish live output
    pub async fn publish_output(&self, output: OutputMessage) {
        let _ = self.tx.send(FeedMessage::Output(output)).await;
    }

    /// Publish wave update
    pub async fn publish_wave_update(&self, update: WaveUpdate) {
        let _ = self.tx.send(FeedMessage::WaveUpdate(update)).await;
    }

    /// Publish stats
    pub async fn publish_stats(&self, stats: StatsSnapshot) {
        let _ = self.tx.send(FeedMessage::Stats(stats)).await;
    }

    /// Shutdown the feed
    pub async fn shutdown(&self) {
        let _ = self.tx.send(FeedMessage::Shutdown).await;
    }
}

/// Synchronous feed handle for use from non-async contexts
pub struct FeedHandleSync {
    tx: mpsc::Sender<FeedMessage>,
    runtime: tokio::runtime::Handle,
}

impl FeedHandleSync {
    /// Create from async handle
    pub fn new(tx: mpsc::Sender<FeedMessage>, runtime: tokio::runtime::Handle) -> Self {
        Self { tx, runtime }
    }

    /// Publish a session snapshot (blocking)
    pub fn publish_session(&self, snapshot: SessionSnapshot) {
        let tx = self.tx.clone();
        let _ = self
            .runtime
            .block_on(async move { tx.send(FeedMessage::Session(snapshot)).await });
    }

    /// Publish an agent status update (blocking)
    pub fn publish_agent_update(&self, update: AgentUpdate) {
        let tx = self.tx.clone();
        let _ = self
            .runtime
            .block_on(async move { tx.send(FeedMessage::AgentUpdate(update)).await });
    }

    /// Publish live output (blocking)
    pub fn publish_output(&self, output: OutputMessage) {
        let tx = self.tx.clone();
        let _ = self
            .runtime
            .block_on(async move { tx.send(FeedMessage::Output(output)).await });
    }

    /// Publish wave update (blocking)
    pub fn publish_wave_update(&self, update: WaveUpdate) {
        let tx = self.tx.clone();
        let _ = self
            .runtime
            .block_on(async move { tx.send(FeedMessage::WaveUpdate(update)).await });
    }

    /// Publish stats (blocking)
    pub fn publish_stats(&self, stats: StatsSnapshot) {
        let tx = self.tx.clone();
        let _ = self
            .runtime
            .block_on(async move { tx.send(FeedMessage::Stats(stats)).await });
    }

    /// Non-blocking send (best effort, may drop if channel full)
    pub fn try_publish_output(&self, output: OutputMessage) {
        let _ = self.tx.try_send(FeedMessage::Output(output));
    }

    /// Get the sender for cloning
    pub fn sender(&self) -> mpsc::Sender<FeedMessage> {
        self.tx.clone()
    }
}

impl Clone for FeedHandleSync {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            runtime: self.runtime.clone(),
        }
    }
}

/// Start the feed publisher background task
///
/// Returns a handle for sending messages and the endpoint that was bound.
pub async fn start_feed(config: FeedConfig) -> Result<(FeedHandle, String)> {
    let (tx, rx) = mpsc::channel::<FeedMessage>(1000);

    // Create PUB socket
    let mut socket = zeromq::PubSocket::new();
    socket
        .bind(&config.endpoint)
        .await
        .context(format!("Failed to bind ZMQ socket to {}", config.endpoint))?;

    let endpoint = config.endpoint.clone();

    // Spawn publisher task
    tokio::spawn(async move {
        run_publisher(socket, rx, config).await;
    });

    Ok((FeedHandle { tx }, endpoint))
}

/// Start feed and return sync handle for use in non-async TUI
pub fn start_feed_sync(config: FeedConfig) -> Result<(FeedHandleSync, String)> {
    // Create a new runtime for the feed
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("Failed to create tokio runtime for feed")?;

    let handle = rt.handle().clone();

    let (tx, rx) = mpsc::channel::<FeedMessage>(1000);

    // Create PUB socket synchronously via runtime
    let endpoint = config.endpoint.clone();
    let endpoint_clone = endpoint.clone();

    let socket = handle.block_on(async {
        let mut socket = zeromq::PubSocket::new();
        socket
            .bind(&endpoint_clone)
            .await
            .context(format!("Failed to bind ZMQ socket to {}", endpoint_clone))?;
        Ok::<_, anyhow::Error>(socket)
    })?;

    // Spawn publisher task in the runtime
    let config_clone = config.clone();
    std::thread::spawn(move || {
        rt.block_on(async move {
            run_publisher(socket, rx, config_clone).await;
        });
    });

    Ok((FeedHandleSync::new(tx, handle), endpoint))
}

/// Run the publisher loop
async fn run_publisher(
    mut socket: zeromq::PubSocket,
    mut rx: mpsc::Receiver<FeedMessage>,
    config: FeedConfig,
) {
    let start_time = std::time::Instant::now();
    let mut message_count: u64 = 0;
    let mut session_name = String::new();

    // Heartbeat interval
    let heartbeat_interval = tokio::time::Duration::from_secs(config.heartbeat_interval_secs);
    let mut heartbeat_timer = tokio::time::interval(heartbeat_interval);

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(FeedMessage::Shutdown) | None => {
                        break;
                    }
                    Some(msg) => {
                        if let FeedMessage::Session(ref s) = msg {
                            session_name = s.session_name.clone();
                        }

                        let (topic, payload) = serialize_message(&msg);
                        if let Err(e) = send_message(&mut socket, &topic, &payload).await {
                            eprintln!("Feed send error: {}", e);
                        } else {
                            message_count += 1;
                        }
                    }
                }
            }
            _ = heartbeat_timer.tick(), if config.heartbeat => {
                let heartbeat = Heartbeat {
                    session_name: session_name.clone(),
                    uptime_secs: start_time.elapsed().as_secs(),
                    message_count,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let payload = serde_json::to_string(&heartbeat).unwrap_or_default();
                if let Err(e) = send_message(&mut socket, topics::HEARTBEAT, &payload).await {
                    eprintln!("Heartbeat send error: {}", e);
                }
            }
        }
    }
}

/// Serialize a message to topic and JSON payload
fn serialize_message(msg: &FeedMessage) -> (String, String) {
    match msg {
        FeedMessage::Session(s) => (
            topics::SESSION.to_string(),
            serde_json::to_string(s).unwrap_or_default(),
        ),
        FeedMessage::AgentUpdate(u) => (
            topics::AGENT.to_string(),
            serde_json::to_string(u).unwrap_or_default(),
        ),
        FeedMessage::Output(o) => (
            topics::OUTPUT.to_string(),
            serde_json::to_string(o).unwrap_or_default(),
        ),
        FeedMessage::WaveUpdate(w) => (
            topics::WAVE.to_string(),
            serde_json::to_string(w).unwrap_or_default(),
        ),
        FeedMessage::Stats(s) => (
            topics::STATS.to_string(),
            serde_json::to_string(s).unwrap_or_default(),
        ),
        FeedMessage::Heartbeat(h) => (
            topics::HEARTBEAT.to_string(),
            serde_json::to_string(h).unwrap_or_default(),
        ),
        FeedMessage::Shutdown => ("shutdown".to_string(), "{}".to_string()),
    }
}

/// Send a message on the PUB socket
async fn send_message(
    socket: &mut zeromq::PubSocket,
    topic: &str,
    payload: &str,
) -> Result<()> {
    // ZMQ PUB/SUB uses topic prefix for filtering
    // Format: "topic payload" where topic is space-separated
    let message = format!("{} {}", topic, payload);
    socket
        .send(message.into())
        .await
        .context("Failed to send ZMQ message")?;
    Ok(())
}

// ============================================================================
// Helper functions for creating messages from App state
// ============================================================================

/// Create a session snapshot from SpawnSession
pub fn session_to_snapshot(session: &SpawnSession) -> SessionSnapshot {
    let stats = SpawnStats::from(session);
    SessionSnapshot {
        session_name: session.session_name.clone(),
        tag: session.tag.clone(),
        terminal: session.terminal.clone(),
        created_at: session.created_at.clone(),
        working_dir: session.working_dir.clone(),
        agents: session.agents.iter().map(AgentSnapshot::from).collect(),
        stats: StatsSnapshot::from(&stats),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Create an output message
pub fn create_output_message(task_id: &str, lines: Vec<String>) -> OutputMessage {
    let line_count = lines.len();
    OutputMessage {
        task_id: task_id.to_string(),
        lines,
        line_count,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Create an agent update message
pub fn create_agent_update(
    task_id: &str,
    status: &AgentStatus,
    previous: Option<&AgentStatus>,
) -> AgentUpdate {
    AgentUpdate {
        task_id: task_id.to_string(),
        status: format!("{:?}", status).to_lowercase(),
        previous_status: previous.map(|s| format!("{:?}", s).to_lowercase()),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_config_tcp() {
        let config = FeedConfig::tcp(5555);
        assert_eq!(config.endpoint, "tcp://*:5555");
    }

    #[test]
    fn test_feed_config_ipc() {
        let config = FeedConfig::ipc("/tmp/scud.sock");
        assert_eq!(config.endpoint, "ipc:///tmp/scud.sock");
    }

    #[test]
    fn test_serialize_stats() {
        let stats = StatsSnapshot {
            session_name: "test".to_string(),
            tag: "auth".to_string(),
            total_agents: 5,
            starting: 1,
            running: 2,
            completed: 2,
            failed: 0,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("auth"));
    }

    #[test]
    fn test_serialize_output() {
        let output = OutputMessage {
            task_id: "auth:1".to_string(),
            lines: vec!["line1".to_string(), "line2".to_string()],
            line_count: 2,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("auth:1"));
        assert!(json.contains("line1"));
    }
}
