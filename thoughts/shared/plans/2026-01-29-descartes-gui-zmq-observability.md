# Descartes GUI ZMQ Observability Implementation Plan

## Overview

Add ZeroMQ-based real-time event streaming and control to SCUD swarm execution, enabling descartes-gui (and other clients) to connect to running swarms, receive live events, and send control commands (pause/resume/stop). This transforms descartes-gui from a "swarm launcher" into a true "swarm monitor."

## Current State Analysis

**descartes-gui limitations:**
- Can only monitor swarms it spawns itself (`scud_bridge.rs:419-491`)
- References non-existent `--json-events` flag (`scud_bridge.rs:429`)
- Cannot attach to swarms started from terminal
- Cannot control external swarms

**Existing event infrastructure:**
- SQLite events at `.scud/scud.db` (`db/events.rs`)
- Session state files at `.scud/swarm/<session>.json`
- Lock files at `.scud/swarm/<tag>.lock`
- RPC server exists but doesn't support swarm monitoring (`rpc/server.rs`)

### Key Discoveries:
- EventWriter already centralizes event emission (`swarm/events.rs:165-292`)
- Events are written synchronously to SQLite
- Session lock contains PID for process detection (`session.rs:368-373`)
- No real-time streaming mechanism exists

## Desired End State

After implementation:

1. **Any swarm** publishes events via ZMQ PUB socket by default
2. **descartes-gui** can discover and connect to any running swarm
3. **Multiple clients** can watch the same swarm simultaneously
4. **Control commands** (pause/resume/stop) work via ZMQ REQ/REP
5. **Late joiners** catch up from SQLite, then receive live stream
6. **CLI tool** `scud watch` provides terminal-based monitoring

**Verification:**
```bash
# Terminal 1: Start swarm
scud swarm --tag myproject

# Terminal 2: Watch with GUI
descartes-gui  # Can see and control the swarm

# Terminal 3: Watch with CLI
scud watch --tag myproject  # Also sees events

# All three see the same events in real-time
```

## What We're NOT Doing

- Web browser dashboard (would need WebSocket bridge)
- Remote network access (localhost only for security)
- Encrypted ZMQ channels (not needed for localhost)
- Persisting ZMQ messages (SQLite handles persistence)
- Changing the TUI monitor (`scud spawn -m`) - separate system

## Implementation Approach

Use ZeroMQ with two socket patterns:
- **PUB/SUB** for event streaming (one-to-many, fire-and-forget)
- **REQ/REP** for control commands (request-response)

```
┌─────────────────────────────────────────────────────────────┐
│                        SCUD Swarm                           │
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │ Wave Loop   │───►│EventPublisher│───►│ SQLite DB   │     │
│  └─────────────┘    └──────┬──────┘    └─────────────┘     │
│                            │                                │
│         ┌──────────────────┼──────────────────┐            │
│         ▼                                     ▼            │
│  ┌─────────────┐                       ┌─────────────┐     │
│  │ ZMQ PUB     │ tcp://127.0.0.1:0     │ ZMQ REP     │     │
│  │ (events)    │ (dynamic port)        │ (control)   │     │
│  └─────────────┘                       └─────────────┘     │
└─────────────────────────────────────────────────────────────┘
           │                                     ▲
           │ SUB                                 │ REQ
           ▼                                     │
    ┌─────────────┐                       ┌──────┴──────┐
    │ descartes   │                       │ descartes   │
    │ gui (events)│                       │ gui (ctrl)  │
    └─────────────┘                       └─────────────┘
```

---

## Phase 1: ZMQ Infrastructure in scud-cli

### Overview
Add ZeroMQ event publishing and control socket to swarm execution. Events are published alongside SQLite writes. Control commands enable pause/resume/stop.

### Changes Required:

#### 1.1 Add zeromq dependency

**File**: `scud-cli/Cargo.toml`
**Changes**: Add zeromq crate

```toml
[dependencies]
# ... existing deps ...
zeromq = "0.4"
```

#### 1.2 Create ZMQ publisher module

**File**: `scud-cli/src/commands/swarm/publisher.rs` (new file)
**Changes**: Create ZMQ socket management

```rust
//! ZeroMQ event publisher for real-time swarm monitoring
//!
//! Publishes swarm events via PUB socket and accepts control commands via REP socket.
//! Clients can discover socket addresses via files in `.scud/swarm/<session>/`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use zeromq::{PubSocket, RepSocket, Socket, SocketRecv, SocketSend};

/// Published event format (JSON over ZMQ)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ZmqEvent {
    SwarmStarted { session_id: String, tag: String, total_waves: usize },
    WaveStarted { wave: usize, tasks: Vec<String> },
    TaskStarted { task_id: String },
    TaskOutput { task_id: String, text: String },
    TaskCompleted { task_id: String, success: bool, duration_ms: Option<u64> },
    ValidationStarted,
    ValidationCompleted { passed: bool, output: String },
    WaveCompleted { wave: usize, duration_ms: Option<u64> },
    SwarmCompleted { success: bool },
    SwarmPaused,
    SwarmResumed,
    Heartbeat { timestamp: String },
}

/// Control command format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Pause,
    Resume,
    Stop,
    Status,
}

/// Control response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<SwarmStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStatus {
    pub state: String, // "running", "paused", "completed"
    pub current_wave: usize,
    pub total_waves: usize,
    pub tasks_completed: usize,
    pub tasks_total: usize,
}

/// ZMQ Publisher for swarm events
pub struct EventPublisher {
    pub_socket: PubSocket,
    pub_endpoint: String,
    rep_socket: Arc<Mutex<RepSocket>>,
    rep_endpoint: String,
    session_dir: std::path::PathBuf,
}

impl EventPublisher {
    /// Create and bind publisher sockets
    ///
    /// Binds to dynamic ports on localhost and writes addresses to discovery files.
    pub async fn new(session_dir: &Path) -> Result<Self> {
        // Create PUB socket for events
        let mut pub_socket = PubSocket::new();
        let pub_endpoint = pub_socket.bind("tcp://127.0.0.1:0").await?;
        tracing::info!("ZMQ PUB bound to {}", pub_endpoint);

        // Create REP socket for control
        let mut rep_socket = RepSocket::new();
        let rep_endpoint = rep_socket.bind("tcp://127.0.0.1:0").await?;
        tracing::info!("ZMQ REP bound to {}", rep_endpoint);

        let publisher = Self {
            pub_socket,
            pub_endpoint: pub_endpoint.clone(),
            rep_socket: Arc::new(Mutex::new(rep_socket)),
            rep_endpoint: rep_endpoint.clone(),
            session_dir: session_dir.to_path_buf(),
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
    pub async fn publish(&mut self, event: ZmqEvent) -> Result<()> {
        let json = serde_json::to_string(&event)?;
        self.pub_socket.send(json.into()).await?;
        Ok(())
    }

    /// Get the REP socket for control command handling
    pub fn control_socket(&self) -> Arc<Mutex<RepSocket>> {
        self.rep_socket.clone()
    }

    /// Get the PUB endpoint address
    pub fn pub_endpoint(&self) -> &str {
        &self.pub_endpoint
    }

    /// Get the REP endpoint address
    pub fn rep_endpoint(&self) -> &str {
        &self.rep_endpoint
    }

    /// Clean up discovery files on shutdown
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(self.session_dir.join("zmq-pub.addr"));
        let _ = std::fs::remove_file(self.session_dir.join("zmq-rep.addr"));
    }
}

impl Drop for EventPublisher {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Discover ZMQ endpoints for a session
pub fn discover_endpoints(session_dir: &Path) -> Option<(String, String)> {
    let pub_path = session_dir.join("zmq-pub.addr");
    let rep_path = session_dir.join("zmq-rep.addr");

    let pub_addr = std::fs::read_to_string(&pub_path).ok()?;
    let rep_addr = std::fs::read_to_string(&rep_path).ok()?;

    Some((pub_addr.trim().to_string(), rep_addr.trim().to_string()))
}
```

#### 1.3 Update swarm module exports

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add publisher module and integrate with swarm execution

```rust
// Add module declaration near top
pub mod publisher;
```

#### 1.4 Integrate publisher with EventWriter

**File**: `scud-cli/src/commands/swarm/events.rs`
**Changes**: Add ZMQ publishing to EventWriter

Add field and methods to EventWriter struct:

```rust
use super::publisher::{EventPublisher, ZmqEvent};

pub struct EventWriter {
    // ... existing fields ...
    db: Option<Database>,
    session_id: String,
    /// ZMQ publisher for real-time event streaming
    zmq_publisher: Option<tokio::sync::Mutex<EventPublisher>>,
}

impl EventWriter {
    /// Create EventWriter with optional ZMQ publishing
    pub async fn new_with_zmq(
        project_root: &Path,
        session_id: &str,
        enable_zmq: bool,
    ) -> Result<Self> {
        let db = Database::new(project_root).ok();

        let zmq_publisher = if enable_zmq {
            let session_dir = project_root.join(".scud/swarm").join(session_id);
            match EventPublisher::new(&session_dir).await {
                Ok(pub) => Some(tokio::sync::Mutex::new(pub)),
                Err(e) => {
                    tracing::warn!("Failed to create ZMQ publisher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            db,
            session_id: session_id.to_string(),
            zmq_publisher,
        })
    }

    /// Publish event via ZMQ (non-blocking, best-effort)
    async fn zmq_publish(&self, event: ZmqEvent) {
        if let Some(ref publisher) = self.zmq_publisher {
            if let Ok(mut pub_guard) = publisher.try_lock() {
                if let Err(e) = pub_guard.publish(event).await {
                    tracing::debug!("ZMQ publish error (non-fatal): {}", e);
                }
            }
        }
    }

    /// Get the control socket for command handling
    pub fn control_socket(&self) -> Option<Arc<tokio::sync::Mutex<zeromq::RepSocket>>> {
        self.zmq_publisher.as_ref().map(|p| {
            // This is a bit awkward - we need to access the inner socket
            // Consider refactoring to expose control handling differently
        })
    }
}
```

Update each log method to also publish via ZMQ. Example for `log_wave_started`:

```rust
pub async fn log_wave_started(&self, wave_number: usize, task_count: usize) {
    // Existing SQLite write
    self.write_event(EventKind::WaveStarted { wave_number, task_count });

    // ZMQ publish
    self.zmq_publish(ZmqEvent::WaveStarted {
        wave: wave_number,
        tasks: vec![], // TODO: pass task IDs
    }).await;
}
```

#### 1.5 Add --no-publish-events flag

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add CLI flag and pass to EventWriter

In the SwarmArgs struct:

```rust
#[derive(Parser, Debug)]
pub struct SwarmArgs {
    // ... existing args ...

    /// Disable ZMQ event publishing (no real-time monitoring)
    #[arg(long, default_value = "false")]
    pub no_publish_events: bool,
}
```

In the swarm execution setup:

```rust
// Replace EventWriter::new() with:
let event_writer = EventWriter::new_with_zmq(
    &project_root,
    &session_id,
    !args.no_publish_events,  // enable_zmq
).await?;
```

#### 1.6 Add control command handler

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Spawn task to handle control commands

```rust
use publisher::{ControlCommand, ControlResponse, SwarmStatus};

/// Spawn background task to handle control commands
fn spawn_control_handler(
    control_socket: Arc<Mutex<RepSocket>>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    status_fn: impl Fn() -> SwarmStatus + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let mut socket = control_socket.lock().await;

            // Receive command (with timeout to check stop_flag)
            match tokio::time::timeout(
                Duration::from_millis(100),
                socket.recv()
            ).await {
                Ok(Ok(msg)) => {
                    let response = match serde_json::from_slice::<ControlCommand>(&msg) {
                        Ok(ControlCommand::Pause) => {
                            pause_flag.store(true, Ordering::SeqCst);
                            ControlResponse {
                                success: true,
                                message: "Swarm paused".into(),
                                status: None,
                            }
                        }
                        Ok(ControlCommand::Resume) => {
                            pause_flag.store(false, Ordering::SeqCst);
                            ControlResponse {
                                success: true,
                                message: "Swarm resumed".into(),
                                status: None,
                            }
                        }
                        Ok(ControlCommand::Stop) => {
                            stop_flag.store(true, Ordering::SeqCst);
                            ControlResponse {
                                success: true,
                                message: "Swarm stopping".into(),
                                status: None,
                            }
                        }
                        Ok(ControlCommand::Status) => {
                            ControlResponse {
                                success: true,
                                message: "Status retrieved".into(),
                                status: Some(status_fn()),
                            }
                        }
                        Err(e) => {
                            ControlResponse {
                                success: false,
                                message: format!("Invalid command: {}", e),
                                status: None,
                            }
                        }
                    };

                    let json = serde_json::to_vec(&response).unwrap();
                    let _ = socket.send(json.into()).await;
                }
                Ok(Err(e)) => {
                    tracing::debug!("Control socket error: {}", e);
                }
                Err(_) => {
                    // Timeout - check if we should stop
                    if stop_flag.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }
        }
    })
}
```

#### 1.7 Integrate pause/stop flags into wave loop

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Check flags during execution

In the wave execution loop, add pause checking:

```rust
// At start of each wave iteration
while pause_flag.load(Ordering::SeqCst) {
    tokio::time::sleep(Duration::from_millis(100)).await;
    if stop_flag.load(Ordering::SeqCst) {
        break 'wave_loop;
    }
}

if stop_flag.load(Ordering::SeqCst) {
    tracing::info!("Swarm stopped by control command");
    break 'wave_loop;
}
```

#### 1.8 Add heartbeat task

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Spawn background task to emit heartbeats every 5 seconds

```rust
/// Spawn heartbeat task for connection liveness detection
fn spawn_heartbeat_task(
    publisher: Arc<Mutex<EventPublisher>>,
    stop_flag: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;

            if stop_flag.load(Ordering::SeqCst) {
                break;
            }

            if let Ok(mut pub_guard) = publisher.try_lock() {
                let _ = pub_guard.publish(ZmqEvent::Heartbeat {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }).await;
            }
        }
    })
}
```

Start heartbeat alongside control handler:

```rust
// In swarm setup, after creating publisher:
let heartbeat_handle = spawn_heartbeat_task(
    publisher.clone(),
    stop_flag.clone(),
);

// In cleanup:
heartbeat_handle.abort();
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] Tests pass: `cargo test -p scud-cli`
- [ ] New flag appears in help: `scud swarm --help | grep no-publish-events`
- [ ] ZMQ files created during swarm: `ls .scud/swarm/*/zmq-*.addr`

#### Manual Verification:
- [ ] Start swarm, verify zmq-pub.addr and zmq-rep.addr files are created
- [ ] Use `zmq` CLI tool to subscribe and see events: `zmq sub tcp://127.0.0.1:<port>`
- [ ] Send pause command via ZMQ REQ and verify swarm pauses
- [ ] Files are cleaned up when swarm exits

---

## Phase 2: descartes-gui ZMQ Client

### Overview
Update descartes-gui to discover and connect to running swarms via ZMQ, receive live events, and send control commands.

### Changes Required:

#### 2.1 Add zeromq dependency

**File**: `descartes-gui/Cargo.toml`
**Changes**: Add zeromq crate

```toml
[dependencies]
# ... existing deps ...
zeromq = "0.4"
```

#### 2.2 Add ZMQ subscriber module

**File**: `descartes-gui/src/zmq_client.rs` (new file)
**Changes**: ZMQ client for connecting to swarms

```rust
//! ZeroMQ client for connecting to running SCUD swarms
//!
//! Discovers swarm ZMQ endpoints via discovery files and provides
//! event subscription and control command interfaces.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use zeromq::{ReqSocket, Socket, SocketRecv, SocketSend, SubSocket};

use crate::scud_bridge::ScudEvent;

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
    WaveStarted { wave: usize, tasks: Vec<String> },
    TaskStarted { task_id: String },
    TaskOutput { task_id: String, text: String },
    TaskCompleted { task_id: String, success: bool, duration_ms: Option<u64> },
    ValidationStarted,
    ValidationCompleted { passed: bool, output: String },
    WaveCompleted { wave: usize, duration_ms: Option<u64> },
    SwarmCompleted { success: bool },
    SwarmPaused,
    SwarmResumed,
    Heartbeat { timestamp: String },
}

impl From<ZmqEvent> for ScudEvent {
    fn from(event: ZmqEvent) -> Self {
        match event {
            ZmqEvent::SwarmStarted { tag, total_waves, .. } => {
                ScudEvent::SwarmStarted { tag, total_waves }
            }
            ZmqEvent::WaveStarted { wave, tasks } => {
                ScudEvent::WaveStarted { wave, tasks }
            }
            ZmqEvent::TaskStarted { task_id } => {
                ScudEvent::TaskStarted { task_id }
            }
            ZmqEvent::TaskOutput { task_id, text } => {
                ScudEvent::TaskOutput { task_id, text }
            }
            ZmqEvent::TaskCompleted { task_id, success, .. } => {
                ScudEvent::TaskCompleted { task_id, success }
            }
            ZmqEvent::ValidationStarted => ScudEvent::ValidationStarted,
            ZmqEvent::ValidationCompleted { passed, output } => {
                ScudEvent::ValidationCompleted { passed, output }
            }
            ZmqEvent::WaveCompleted { wave, .. } => {
                ScudEvent::WaveCompleted { wave }
            }
            ZmqEvent::SwarmCompleted { success } => {
                ScudEvent::SwarmCompleted { success }
            }
            ZmqEvent::SwarmPaused => {
                ScudEvent::Output("Swarm paused".to_string())
            }
            ZmqEvent::SwarmResumed => {
                ScudEvent::Output("Swarm resumed".to_string())
            }
            ZmqEvent::Heartbeat { .. } => {
                // Ignore heartbeats in GUI
                ScudEvent::Output("".to_string())
            }
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
    pub async fn connect(session: DiscoveredSession) -> Result<Self> {
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

    /// Send a control command
    pub async fn send_command(&self, command: ControlCommand) -> Result<ControlResponse> {
        let mut socket = ReqSocket::new();
        socket.connect(&self.session.rep_endpoint).await?;

        let json = serde_json::to_vec(&command)?;
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
```

#### 2.3 Update ScudBridge to support attachment

**File**: `descartes-gui/src/scud_bridge.rs`
**Changes**: Add attach mode alongside spawn mode

Add new command variant:

```rust
pub enum ScudCommand {
    // ... existing variants ...

    /// Attach to an existing running swarm via ZMQ
    AttachToSwarm { session_id: String },

    /// Disconnect from attached swarm
    DetachFromSwarm,

    /// List discoverable swarm sessions
    DiscoverSwarms,
}
```

Add new event variants:

```rust
pub enum ScudEvent {
    // ... existing variants ...

    /// List of discovered swarm sessions
    SwarmsDiscovered(Vec<crate::zmq_client::DiscoveredSession>),

    /// Successfully attached to swarm
    AttachedToSwarm { session_id: String, tag: String },

    /// Detached from swarm
    DetachedFromSwarm,
}
```

Update ScudBridge to handle attachment:

```rust
use crate::zmq_client::{discover_sessions, SwarmClient, DiscoveredSession};

pub struct ScudBridge {
    // ... existing fields ...

    /// Active ZMQ client when attached to external swarm
    zmq_client: Option<SwarmClient>,

    /// Whether we're in attached mode vs spawned mode
    attached_mode: bool,
}

impl ScudBridge {
    async fn handle_attach(&mut self, session_id: &str) -> Result<()> {
        let project_root = std::env::current_dir()?;
        let sessions = discover_sessions(&project_root);

        let session = sessions.into_iter()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        let mut client = SwarmClient::connect(session.clone()).await?;
        let event_rx = client.subscribe().await?;

        // Forward events to GUI
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let mut rx = event_rx;
            while let Some(event) = rx.recv().await {
                if event_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        self.zmq_client = Some(client);
        self.attached_mode = true;

        self.event_tx.send(ScudEvent::AttachedToSwarm {
            session_id: session.session_id,
            tag: session.tag,
        }).await?;

        Ok(())
    }

    async fn handle_discover(&self) -> Result<()> {
        let project_root = std::env::current_dir()?;
        let sessions = discover_sessions(&project_root);
        self.event_tx.send(ScudEvent::SwarmsDiscovered(sessions)).await?;
        Ok(())
    }
}
```

Update control command handling to use ZMQ when attached:

```rust
async fn pause_swarm(&mut self) {
    if let Some(ref client) = self.zmq_client {
        // Attached mode - send via ZMQ
        match client.pause().await {
            Ok(resp) => {
                if resp.success {
                    self.event_tx.send(ScudEvent::Output("Swarm paused".into())).await.ok();
                } else {
                    self.event_tx.send(ScudEvent::Error(resp.message)).await.ok();
                }
            }
            Err(e) => {
                self.event_tx.send(ScudEvent::Error(e.to_string())).await.ok();
            }
        }
    } else if let Some(ref handle) = self.swarm_handle {
        // Spawned mode - use SIGSTOP (existing code)
        // ...
    }
}
```

#### 2.4 Add session discovery UI

**File**: `descartes-gui/src/views/agents.rs`
**Changes**: Add session list and attach button

```rust
pub fn view<'a>(
    agent_status: AgentStatus,
    current_task: &Option<String>,
    active_tag: &Option<String>,
    swarm_defaults: &SwarmDefaults,
    discovered_sessions: &[DiscoveredSession],  // New parameter
    attached_session: &Option<String>,           // New parameter
) -> Element<'a, Message> {
    let mut content = column![].spacing(10);

    // Show discovered sessions section
    if !discovered_sessions.is_empty() {
        content = content.push(text("Running Swarms:").size(16));

        for session in discovered_sessions {
            let is_attached = attached_session.as_ref() == Some(&session.session_id);

            let session_row = row![
                text(&session.tag).width(Length::FillPortion(2)),
                text(&session.session_id).width(Length::FillPortion(3)),
                if is_attached {
                    button("Detach").on_press(Message::DetachFromSwarm)
                } else {
                    button("Attach").on_press(Message::AttachToSwarm {
                        session_id: session.session_id.clone()
                    })
                }
            ].spacing(10);

            content = content.push(session_row);
        }

        content = content.push(horizontal_rule(1));
    }

    // Refresh button to discover sessions
    content = content.push(
        button("Refresh Sessions").on_press(Message::DiscoverSwarms)
    );

    // ... rest of existing view ...
}
```

#### 2.5 Update main.rs message handling

**File**: `descartes-gui/src/main.rs`
**Changes**: Handle new messages and state

Add state fields:

```rust
struct DescartesGui {
    // ... existing fields ...
    discovered_sessions: Vec<DiscoveredSession>,
    attached_session: Option<String>,
}
```

Add message variants:

```rust
pub enum Message {
    // ... existing variants ...
    DiscoverSwarms,
    AttachToSwarm { session_id: String },
    DetachFromSwarm,
}
```

Handle new messages:

```rust
Message::DiscoverSwarms => {
    if let Some(ref tx) = self.scud_command_tx {
        let tx = tx.clone();
        return Task::perform(
            async move { tx.send(ScudCommand::DiscoverSwarms).await },
            |_| Message::Tick,
        );
    }
    Task::none()
}

Message::AttachToSwarm { session_id } => {
    if let Some(ref tx) = self.scud_command_tx {
        let tx = tx.clone();
        return Task::perform(
            async move { tx.send(ScudCommand::AttachToSwarm { session_id }).await },
            |_| Message::Tick,
        );
    }
    Task::none()
}

Message::ScudEvent(ScudEvent::SwarmsDiscovered(sessions)) => {
    self.discovered_sessions = sessions;
    Task::none()
}

Message::ScudEvent(ScudEvent::AttachedToSwarm { session_id, tag }) => {
    self.attached_session = Some(session_id);
    self.state.active_tag = Some(tag);
    self.state.agent_status = AgentStatus::Running;
    Task::none()
}
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p descartes-gui`
- [ ] Tests pass: `cargo test -p descartes-gui`

#### Manual Verification:
- [ ] Start swarm in terminal, GUI discovers it via "Refresh Sessions"
- [ ] Click "Attach" and see live events flowing
- [ ] Pause/Resume/Cancel buttons work on attached swarm
- [ ] Detach works and stops event flow
- [ ] Can still start new swarms via existing "Start Swarm" button

---

## Phase 3: CLI Watch Command

### Overview
Add `scud watch` command for terminal-based swarm monitoring, providing an alternative to the GUI.

### Changes Required:

#### 3.1 Create watch command

**File**: `scud-cli/src/commands/watch.rs` (new file)
**Changes**: CLI command to watch swarm events

```rust
//! Watch command - monitor running swarms via ZMQ

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use zeromq::{Socket, SocketRecv, SubSocket};

use crate::commands::swarm::publisher::{discover_endpoints, ZmqEvent};

#[derive(Parser, Debug)]
pub struct WatchArgs {
    /// Session ID to watch (discovers automatically if not specified)
    #[arg(long)]
    session: Option<String>,

    /// Tag to filter sessions
    #[arg(long)]
    tag: Option<String>,

    /// Project root directory
    #[arg(long)]
    project_root: Option<PathBuf>,

    /// Output format: text, json
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn run(args: WatchArgs) -> Result<()> {
    let project_root = args.project_root
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Discover sessions
    let swarm_dir = project_root.join(".scud/swarm");
    let mut sessions = vec![];

    for entry in std::fs::read_dir(&swarm_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some((pub_addr, _)) = discover_endpoints(&path) {
                let session_id = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Filter by tag if specified
                if let Some(ref tag) = args.tag {
                    if !session_id.starts_with(tag) {
                        continue;
                    }
                }

                // Filter by session if specified
                if let Some(ref session) = args.session {
                    if &session_id != session {
                        continue;
                    }
                }

                sessions.push((session_id, pub_addr, path));
            }
        }
    }

    if sessions.is_empty() {
        println!("No running swarms found.");
        if args.tag.is_some() {
            println!("Try without --tag to see all sessions.");
        }
        return Ok(());
    }

    // If multiple sessions, list them
    if sessions.len() > 1 && args.session.is_none() {
        println!("Multiple swarms running. Specify --session to watch one:");
        for (session_id, _, _) in &sessions {
            println!("  {}", session_id);
        }
        return Ok(());
    }

    let (session_id, pub_addr, _) = sessions.into_iter().next().unwrap();
    println!("Watching swarm: {}", session_id);
    println!("Connected to: {}", pub_addr);
    println!("---");

    // Connect and subscribe
    let mut socket = SubSocket::new();
    socket.connect(&pub_addr).await?;
    socket.subscribe("").await?;

    // Receive and print events
    loop {
        match socket.recv().await {
            Ok(msg) => {
                if let Ok(text) = std::str::from_utf8(&msg) {
                    if args.format == "json" {
                        println!("{}", text);
                    } else if let Ok(event) = serde_json::from_str::<ZmqEvent>(text) {
                        print_event(&event);
                    }
                }
            }
            Err(e) => {
                eprintln!("Connection lost: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn print_event(event: &ZmqEvent) {
    match event {
        ZmqEvent::SwarmStarted { tag, total_waves, .. } => {
            println!("[SWARM] Started tag='{}' waves={}", tag, total_waves);
        }
        ZmqEvent::WaveStarted { wave, tasks } => {
            println!("[WAVE {}] Started with {} tasks: {:?}", wave, tasks.len(), tasks);
        }
        ZmqEvent::TaskStarted { task_id } => {
            println!("[TASK {}] Started", task_id);
        }
        ZmqEvent::TaskOutput { task_id, text } => {
            println!("[{}] {}", task_id, text);
        }
        ZmqEvent::TaskCompleted { task_id, success, duration_ms } => {
            let status = if *success { "completed" } else { "FAILED" };
            let duration = duration_ms.map(|d| format!(" ({}ms)", d)).unwrap_or_default();
            println!("[TASK {}] {}{}", task_id, status, duration);
        }
        ZmqEvent::ValidationStarted => {
            println!("[VALIDATION] Running...");
        }
        ZmqEvent::ValidationCompleted { passed, output } => {
            let status = if *passed { "PASSED" } else { "FAILED" };
            println!("[VALIDATION] {}: {}", status, output);
        }
        ZmqEvent::WaveCompleted { wave, duration_ms } => {
            let duration = duration_ms.map(|d| format!(" ({}ms)", d)).unwrap_or_default();
            println!("[WAVE {}] Completed{}", wave, duration);
        }
        ZmqEvent::SwarmCompleted { success } => {
            let status = if *success { "SUCCESS" } else { "FAILED" };
            println!("[SWARM] Completed: {}", status);
        }
        ZmqEvent::SwarmPaused => {
            println!("[SWARM] Paused");
        }
        ZmqEvent::SwarmResumed => {
            println!("[SWARM] Resumed");
        }
        ZmqEvent::Heartbeat { .. } => {
            // Ignore heartbeats in text output
        }
    }
}
```

#### 3.2 Register watch command

**File**: `scud-cli/src/main.rs`
**Changes**: Add watch subcommand

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// Watch a running swarm in real-time
    Watch(commands::watch::WatchArgs),
}

// In match block:
Commands::Watch(args) => commands::watch::run(args).await,
```

### Success Criteria:

#### Automated Verification:
- [ ] Build succeeds: `cargo build -p scud-cli`
- [ ] Help shows command: `scud watch --help`

#### Manual Verification:
- [ ] `scud watch` discovers running swarm
- [ ] Events stream in real-time as swarm executes
- [ ] `--format json` outputs raw JSON
- [ ] Multiple sessions prompts user to specify

---

## Phase 4: SQLite Catch-up for Late Joiners

### Overview
When GUI attaches to a swarm mid-execution, query SQLite for historical events to reconstruct current state.

### Changes Required:

#### 4.1 Add catch-up query to zmq_client

**File**: `descartes-gui/src/zmq_client.rs`
**Changes**: Query SQLite on attach with limits

```rust
use scud_core::Database;
use chrono::{Utc, Duration};

/// Maximum events to fetch for catch-up
const CATCHUP_EVENT_LIMIT: usize = 1000;
/// Maximum age of events to fetch (5 minutes)
const CATCHUP_TIME_LIMIT_SECS: i64 = 300;

impl SwarmClient {
    /// Load historical events from SQLite to catch up
    ///
    /// Limited to last 1000 events OR last 5 minutes, whichever is smaller.
    /// This prevents overwhelming the GUI when attaching to long-running swarms.
    pub async fn catch_up(&self, project_root: &Path) -> Result<Vec<ScudEvent>> {
        let db = Database::new(project_root)?;

        // Calculate time cutoff (5 minutes ago)
        let time_cutoff = Utc::now() - Duration::seconds(CATCHUP_TIME_LIMIT_SECS);

        // Query events for this session with limits
        let events = db.get_events_for_session_limited(
            &self.session.session_id,
            Some(time_cutoff),
            Some(CATCHUP_EVENT_LIMIT),
        )?;

        // Convert to ScudEvents
        let scud_events: Vec<ScudEvent> = events.into_iter()
            .filter_map(|e| convert_db_event_to_scud_event(e))
            .collect();

        tracing::info!(
            "Caught up with {} historical events for session {}",
            scud_events.len(),
            self.session.session_id
        );

        Ok(scud_events)
    }
}

fn convert_db_event_to_scud_event(event: DbEvent) -> Option<ScudEvent> {
    match event.kind.as_str() {
        "wave_started" => Some(ScudEvent::WaveStarted {
            wave: event.wave_number.unwrap_or(0),
            tasks: vec![], // TODO: parse from data
        }),
        "task_started" => Some(ScudEvent::TaskStarted {
            task_id: event.task_id?,
        }),
        "completed" => Some(ScudEvent::TaskCompleted {
            task_id: event.task_id?,
            success: event.success.unwrap_or(false),
        }),
        // ... other event types ...
        _ => None,
    }
}
```

#### 4.2 Add limited query to database

**File**: `scud-cli/src/db/events.rs`
**Changes**: Add method for limited event queries

```rust
impl Database {
    /// Get events for a session with optional time and count limits
    pub fn get_events_for_session_limited(
        &self,
        session_id: &str,
        since: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<Event>> {
        let conn = self.connection()?;

        let mut query = String::from(
            "SELECT * FROM events WHERE session_id = ?"
        );

        if since.is_some() {
            query.push_str(" AND timestamp >= ?");
        }

        query.push_str(" ORDER BY timestamp ASC");

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&query)?;

        let events = if let Some(since) = since {
            stmt.query_map(
                rusqlite::params![session_id, since.to_rfc3339()],
                |row| Event::from_row(row),
            )?
        } else {
            stmt.query_map(
                rusqlite::params![session_id],
                |row| Event::from_row(row),
            )?
        };

        events.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
```

#### 4.3 Send catch-up events on attach

**File**: `descartes-gui/src/scud_bridge.rs`
**Changes**: Emit historical events before live stream

```rust
async fn handle_attach(&mut self, session_id: &str) -> Result<()> {
    // ... existing connection code ...

    // Catch up from SQLite
    let project_root = std::env::current_dir()?;
    let historical = client.catch_up(&project_root).await?;

    // Emit historical events to GUI
    for event in historical {
        self.event_tx.send(event).await?;
    }

    // Then start live stream
    // ... existing subscription code ...
}
```

### Success Criteria:

#### Manual Verification:
- [ ] Attach to swarm that's already on wave 3, GUI shows waves 1-3 history
- [ ] Output buffer contains historical task output
- [ ] Current wave/task state is accurate

---

## Testing Strategy

### Unit Tests:
- ZmqEvent serialization/deserialization roundtrip
- ControlCommand/ControlResponse parsing
- Session discovery with mock filesystem
- Event conversion (ZmqEvent → ScudEvent)

### Integration Tests:
- Start swarm, connect subscriber, verify events received
- Send control commands, verify swarm responds
- Multiple subscribers receive same events
- Subscriber disconnect/reconnect

### Manual Testing Steps:
1. Start swarm: `scud swarm --tag test`
2. In another terminal: `scud watch --tag test` - verify events stream
3. Open descartes-gui, click "Refresh Sessions", verify swarm appears
4. Click "Attach", verify events flow to GUI
5. Click "Pause" in GUI, verify swarm pauses
6. Click "Resume", verify swarm continues
7. Click "Cancel", verify swarm stops
8. Verify zmq-*.addr files are cleaned up after swarm exits

## Performance Considerations

- ZMQ PUB is non-blocking - won't slow down swarm if no subscribers
- GUI uses mpsc channel buffer (1000) to handle event bursts
- SQLite catch-up query should be indexed by session_id
- **Heartbeat events** every 5 seconds for connection liveness detection
- **Catch-up limit**: Query last 1000 events OR last 5 minutes, whichever is smaller

## Migration Notes

- Existing swarms (started before this change) won't have ZMQ
- GUI should gracefully handle missing zmq-*.addr files
- `--no-publish-events` flag provides escape hatch if ZMQ causes issues

## References

- zeromq crate docs: https://docs.rs/zeromq
- ZMQ PUB/SUB pattern: https://zguide.zeromq.org/docs/chapter1/#Getting-the-Message-Out
- Existing event infrastructure: `scud-cli/src/commands/swarm/events.rs`
- descartes-gui entry point: `descartes-gui/src/main.rs`
