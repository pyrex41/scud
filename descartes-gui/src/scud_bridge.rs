//! SCUD Bridge - Async communication layer between Iced GUI and SCUD Core
//!
//! Provides event-driven communication with SCUD through direct library calls
//! for task operations and subprocess calls for swarm execution.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use scud_core::{compute_waves, Phase, Storage, Task, TaskStatus};

use crate::state::RalphConfig;

// Import headless streaming infrastructure from scud-cli
use scud::commands::spawn::headless::{
    create_runner, AnyRunner, SessionHandle, SessionStatus, StreamEventKind, StreamStore,
};
use scud::commands::spawn::terminal::Harness;

use crate::state::TaskInfo;

/// Events emitted by SCUD execution
///
/// These events are sent from the ScudBridge to the GUI to update state.
#[derive(Debug, Clone)]
pub enum ScudEvent {
    /// Tasks loaded from SCUD storage
    TasksLoaded(Vec<TaskInfo>),

    /// Available tags loaded from storage
    TagsLoaded(Vec<String>),

    /// Available agent types loaded from .scud/agents
    AgentsLoaded(Vec<String>),

    /// Waves computed from task dependencies
    /// Contains full TaskInfo for each task, avoiding fragile ID lookups in the GUI
    WavesComputed(Vec<Vec<TaskInfo>>),

    /// Swarm execution started
    SwarmStarted { tag: String, total_waves: usize },

    /// A wave of tasks started
    WaveStarted { wave: usize, tasks: Vec<String> },

    /// Individual task started execution
    TaskStarted { task_id: String },

    /// Task output received
    TaskOutput { task_id: String, text: String },

    /// Individual task completed
    TaskCompleted { task_id: String, success: bool },

    /// Validation started
    ValidationStarted,

    /// Validation completed
    ValidationCompleted { passed: bool, output: String },

    /// Wave completed
    WaveCompleted { wave: usize },

    /// Swarm execution completed
    SwarmCompleted { success: bool },

    /// Generic output (for streaming text)
    Output(String),

    /// Error occurred
    Error(String),

    // Headless streaming events (for direct agent output visibility)
    /// Headless session started
    HeadlessStarted { task_id: String, harness: String },

    /// Tool execution started (headless mode)
    ToolStart {
        task_id: String,
        tool_name: String,
        tool_id: String,
        input_summary: String,
    },

    /// Tool execution completed (headless mode)
    ToolResult {
        task_id: String,
        tool_name: String,
        tool_id: String,
        success: bool,
    },

    /// Agent session ID assigned (for continuation)
    SessionAssigned { task_id: String, session_id: String },

    /// Tag archived successfully
    TagArchived { tag: String },

    // Ralph mode events
    /// Ralph loop started
    RalphStarted { tag: String, max_iterations: usize },

    /// A new Ralph iteration began (one task)
    RalphIterationStarted { iteration: usize, task_id: String, task_title: String },

    /// Backpressure validation started for a task
    RalphValidationStarted { task_id: String },

    /// Backpressure validation completed
    RalphValidationCompleted { task_id: String, passed: bool, output: String },

    /// Repair agent spawned for a failed validation
    RalphRepairStarted { task_id: String, attempt: usize },

    /// A Ralph iteration completed (task done or failed)
    RalphIterationCompleted { iteration: usize, task_id: String, success: bool },

    /// Ralph loop finished
    RalphCompleted { iterations: usize, completed: usize, failed: usize },
}

/// Commands to send to SCUD
///
/// These commands are sent from the GUI to the ScudBridge.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ScudCommand {
    /// Load tasks from SCUD, optionally filtered by tag
    LoadTasks { tag: Option<String> },

    /// Compute execution waves for a tag
    ComputeWaves { tag: String },

    /// Load available tags from storage
    LoadAvailableTags,

    /// Load available agent types from .scud/agents
    LoadAvailableAgents,

    /// Start swarm execution (headless runners with streaming output)
    StartSwarm {
        tag: String,
        harness: String,
        round_size: usize,
        model: String,
    },

    /// Run a single task with streaming output
    RunTask {
        task_id: String,
        harness: String,
        model: String,
    },

    /// Pause the currently running swarm
    PauseSwarm,

    /// Resume a paused swarm
    ResumeSwarm,

    /// Stop the currently running swarm
    StopSwarm,

    /// Mark a task as complete
    CompleteTask { task_id: String },

    /// Mark a task as blocked
    BlockTask { task_id: String },

    /// Archive a tag's tasks
    ArchiveTag { tag: String },

    /// Attach to a session in interactive terminal mode
    AttachSession {
        task_id: String,
        harness: String,
        session_id: String,
    },

    /// Start Ralph mode execution (sequential, backpressure-driven)
    StartRalph {
        tag: String,
        harness: String,
        model: String,
        ralph_config: RalphConfig,
    },

    /// Stop a running Ralph loop
    StopRalph,
}

/// JSON event format from SCUD CLI when running with --json-events
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum ScudJsonEvent {
    SwarmStarted {
        tag: String,
        total_waves: usize,
    },
    WaveStarted {
        wave: usize,
        tasks: Vec<String>,
    },
    TaskStarted {
        task_id: String,
    },
    TaskOutput {
        task_id: String,
        text: String,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
    },
    ValidationStarted,
    ValidationCompleted {
        passed: bool,
        #[serde(default)]
        output: String,
    },
    WaveCompleted {
        wave: usize,
    },
    SwarmCompleted {
        success: bool,
    },
}

impl From<ScudJsonEvent> for ScudEvent {
    fn from(json_event: ScudJsonEvent) -> Self {
        match json_event {
            ScudJsonEvent::SwarmStarted { tag, total_waves } => {
                ScudEvent::SwarmStarted { tag, total_waves }
            }
            ScudJsonEvent::WaveStarted { wave, tasks } => ScudEvent::WaveStarted { wave, tasks },
            ScudJsonEvent::TaskStarted { task_id } => ScudEvent::TaskStarted { task_id },
            ScudJsonEvent::TaskOutput { task_id, text } => ScudEvent::TaskOutput { task_id, text },
            ScudJsonEvent::TaskCompleted { task_id, success } => {
                ScudEvent::TaskCompleted { task_id, success }
            }
            ScudJsonEvent::ValidationStarted => ScudEvent::ValidationStarted,
            ScudJsonEvent::ValidationCompleted { passed, output } => {
                ScudEvent::ValidationCompleted { passed, output }
            }
            ScudJsonEvent::WaveCompleted { wave } => ScudEvent::WaveCompleted { wave },
            ScudJsonEvent::SwarmCompleted { success } => ScudEvent::SwarmCompleted { success },
        }
    }
}

/// JSON task format from SCUD CLI when running with --json
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ScudJsonTask {
    id: String,
    title: String,
    status: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    complexity: Option<usize>,
    #[serde(default)]
    agent_type: Option<String>,
}

impl From<ScudJsonTask> for TaskInfo {
    fn from(task: ScudJsonTask) -> Self {
        TaskInfo {
            id: task.id,
            title: task.title,
            status: task.status,
            agent: task.agent_type,
        }
    }
}

/// Bridge between Iced GUI and SCUD Core
///
/// Uses direct scud-core library calls for task operations (load, complete, block, waves)
/// and subprocess spawning for swarm execution (which requires complex orchestration).
pub struct ScudBridge {
    /// Sender for events to GUI
    event_tx: mpsc::Sender<ScudEvent>,

    /// Receiver for commands from GUI
    command_rx: mpsc::Receiver<ScudCommand>,

    /// Handle to current swarm process (for cancellation)
    swarm_handle: Option<tokio::process::Child>,

    /// Whether the swarm is currently paused
    paused: bool,

    /// Stream store for headless session management
    stream_store: StreamStore,

    /// Working directory for headless execution
    working_dir: PathBuf,

    /// Cancellation flag for Ralph mode
    ralph_cancelled: Arc<AtomicBool>,
}

impl ScudBridge {
    /// Create a new ScudBridge with the given channel endpoints
    pub fn new(event_tx: mpsc::Sender<ScudEvent>, command_rx: mpsc::Receiver<ScudCommand>) -> Self {
        Self {
            event_tx,
            command_rx,
            swarm_handle: None,
            paused: false,
            stream_store: StreamStore::new(),
            working_dir: std::env::current_dir().unwrap_or_default(),
            ralph_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[allow(dead_code)]
    /// Create a new ScudBridge with a specific working directory
    pub fn with_working_dir(
        event_tx: mpsc::Sender<ScudEvent>,
        command_rx: mpsc::Receiver<ScudCommand>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            event_tx,
            command_rx,
            swarm_handle: None,
            paused: false,
            stream_store: StreamStore::new(),
            working_dir,
            ralph_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new ScudBridge and return the channel handles for the GUI
    ///
    /// Returns (bridge, command_sender, event_receiver)
    pub fn create() -> (Self, mpsc::Sender<ScudCommand>, mpsc::Receiver<ScudEvent>) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (command_tx, command_rx) = mpsc::channel(100);

        let bridge = Self::new(event_tx, command_rx);
        (bridge, command_tx, event_rx)
    }

    #[allow(dead_code)]
    /// Create a new ScudBridge with specific working directory and return channel handles
    pub fn create_with_working_dir(
        working_dir: PathBuf,
    ) -> (Self, mpsc::Sender<ScudCommand>, mpsc::Receiver<ScudEvent>) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (command_tx, command_rx) = mpsc::channel(100);

        let bridge = Self::with_working_dir(event_tx, command_rx, working_dir);
        (bridge, command_tx, event_rx)
    }

    /// Main run loop - processes commands from the GUI
    pub async fn run(mut self) {
        info!("ScudBridge started");

        while let Some(cmd) = self.command_rx.recv().await {
            debug!("ScudBridge received command: {:?}", cmd);

            match cmd {
                ScudCommand::LoadTasks { tag } => {
                    self.load_tasks(tag).await;
                }
                ScudCommand::ComputeWaves { tag } => {
                    self.compute_waves_impl(&tag).await;
                }
                ScudCommand::LoadAvailableTags => {
                    self.load_available_tags().await;
                }
                ScudCommand::LoadAvailableAgents => {
                    self.load_available_agents().await;
                }
                ScudCommand::StartSwarm {
                    tag,
                    harness,
                    round_size,
                    model,
                } => {
                    let model_override = if model.trim().is_empty() {
                        None
                    } else {
                        Some(model)
                    };
                    self.run_swarm(&tag, &harness, round_size, model_override)
                        .await;
                }
                ScudCommand::PauseSwarm => {
                    self.pause_swarm().await;
                }
                ScudCommand::ResumeSwarm => {
                    self.resume_swarm().await;
                }
                ScudCommand::StopSwarm => {
                    self.stop_swarm().await;
                }
                ScudCommand::RunTask {
                    task_id,
                    harness,
                    model,
                } => {
                    let model_override = if model.trim().is_empty() {
                        None
                    } else {
                        Some(model)
                    };
                    self.run_task(&task_id, &harness, model_override).await;
                }
                ScudCommand::CompleteTask { task_id } => {
                    self.complete_task(&task_id).await;
                }
                ScudCommand::BlockTask { task_id } => {
                    self.block_task(&task_id).await;
                }
                ScudCommand::ArchiveTag { tag } => {
                    self.archive_tag(&tag).await;
                }
                ScudCommand::AttachSession {
                    task_id,
                    harness,
                    session_id,
                } => {
                    self.attach_session(&task_id, &harness, &session_id).await;
                }
                ScudCommand::StartRalph {
                    tag,
                    harness,
                    model,
                    ralph_config,
                } => {
                    let model_override = if model.trim().is_empty() {
                        None
                    } else {
                        Some(model)
                    };
                    self.run_ralph(&tag, &harness, model_override, ralph_config)
                        .await;
                }
                ScudCommand::StopRalph => {
                    info!("StopRalph received, setting cancellation flag");
                    self.ralph_cancelled.store(true, Ordering::SeqCst);
                }
            }
        }

        info!("ScudBridge shutting down");
    }

    /// Load tasks from SCUD storage using direct library calls
    ///
    /// Uses Storage to load the active Phase, converts tasks to TaskInfo,
    /// and also computes waves so the Waves view shows proper groupings.
    async fn load_tasks(&self, tag: Option<String>) {
        debug!("Loading tasks via scud-core (tag: {:?})", tag);

        // Run blocking storage operations in a spawn_blocking task
        // Returns (flat task list, computed waves)
        let result = tokio::task::spawn_blocking(
            move || -> Result<(Vec<TaskInfo>, Vec<Vec<TaskInfo>>), String> {
                let storage = Storage::new(None);

                let phase = if let Some(ref tag) = tag {
                    storage.load_group(tag).map_err(|e| e.to_string())?
                } else {
                    storage.load_active_group().map_err(|e| e.to_string())?
                };

                let all_tasks = Self::phase_to_task_infos(&phase);

                // Also compute waves for proper display
                let actionable: Vec<&Task> = phase.get_actionable_tasks();
                let pending_tasks: Vec<&Task> = actionable
                    .into_iter()
                    .filter(|t| {
                        matches!(
                            t.status,
                            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Failed
                        )
                    })
                    .collect();

                let wave_result = compute_waves(&pending_tasks);
                let waves: Vec<Vec<TaskInfo>> = wave_result
                    .waves
                    .into_iter()
                    .map(|wave| {
                        wave.tasks
                            .into_iter()
                            .filter_map(|task_id| {
                                phase.get_task(&task_id).map(Self::task_to_task_info)
                            })
                            .collect()
                    })
                    .filter(|wave: &Vec<TaskInfo>| !wave.is_empty())
                    .collect();

                Ok((all_tasks, waves))
            },
        )
        .await;

        match result {
            Ok(Ok((task_infos, waves))) => {
                let _ = self.event_tx.send(ScudEvent::TasksLoaded(task_infos)).await;
                if !waves.is_empty() {
                    let _ = self.event_tx.send(ScudEvent::WavesComputed(waves)).await;
                }
            }
            Ok(Err(e)) => {
                error!("Failed to load tasks: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to load tasks: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Convert a Phase's tasks to TaskInfo vec
    fn phase_to_task_infos(phase: &Phase) -> Vec<TaskInfo> {
        phase
            .tasks
            .iter()
            .map(|task| TaskInfo {
                id: task.id.clone(),
                title: task.title.clone(),
                status: task.status.as_str().to_string(),
                agent: task.agent_type.clone(),
            })
            .collect()
    }

    /// Convert a scud_core Task to TaskInfo
    fn task_to_task_info(task: &Task) -> TaskInfo {
        TaskInfo {
            id: task.id.clone(),
            title: task.title.clone(),
            status: task.status.as_str().to_string(),
            agent: task.agent_type.clone(),
        }
    }

    /// Compute execution waves for a tag using direct library calls
    ///
    /// Uses Storage to load the Phase, then scud_core::compute_waves() to compute waves,
    /// and maps the results to TaskInfo for the GUI.
    async fn compute_waves_impl(&self, tag: &str) {
        let tag = tag.to_string();
        debug!("Computing waves via scud-core for tag: {}", tag);

        // Run blocking storage operations in a spawn_blocking task
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<TaskInfo>>, String> {
            let storage = Storage::new(None);
            let phase = storage.load_group(&tag).map_err(|e| e.to_string())?;

            // Get actionable tasks (filters out expanded parents, etc.)
            let actionable: Vec<&Task> = phase.get_actionable_tasks();

            // Filter to only pending/in-progress tasks for wave computation
            let pending_tasks: Vec<&Task> = actionable
                .into_iter()
                .filter(|t| {
                    matches!(
                        t.status,
                        TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Failed
                    )
                })
                .collect();

            // Compute waves using scud-core
            let wave_result = compute_waves(&pending_tasks);

            // Convert waves to TaskInfo, preserving task details
            let waves: Vec<Vec<TaskInfo>> = wave_result
                .waves
                .into_iter()
                .map(|wave| {
                    wave.tasks
                        .into_iter()
                        .filter_map(|task_id| phase.get_task(&task_id).map(Self::task_to_task_info))
                        .collect()
                })
                .filter(|wave: &Vec<TaskInfo>| !wave.is_empty())
                .collect();

            Ok(waves)
        })
        .await;

        match result {
            Ok(Ok(waves)) => {
                let _ = self.event_tx.send(ScudEvent::WavesComputed(waves)).await;
            }
            Ok(Err(e)) => {
                error!("Failed to compute waves: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to compute waves: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Load available tags from storage
    async fn load_available_tags(&self) {
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
            let storage = Storage::new(None);
            let tasks = storage.load_tasks().map_err(|e| e.to_string())?;
            let mut tags: Vec<String> = tasks.keys().cloned().collect();
            tags.sort();
            Ok(tags)
        })
        .await;

        match result {
            Ok(Ok(tags)) => {
                let _ = self.event_tx.send(ScudEvent::TagsLoaded(tags)).await;
            }
            Ok(Err(e)) => {
                error!("Failed to load tags: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to load tags: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Load available agent types from .scud/agents
    async fn load_available_agents(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
            let agents_dir = working_dir.join(".scud").join("agents");
            if !agents_dir.exists() {
                return Ok(Vec::new());
            }

            let mut agents = Vec::new();
            let entries = fs::read_dir(&agents_dir).map_err(|e| e.to_string())?;
            for entry in entries {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("toml"))
                        .unwrap_or(false)
                {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        agents.push(stem.to_string());
                    }
                }
            }

            agents.sort();
            Ok(agents)
        })
        .await;

        match result {
            Ok(Ok(agents)) => {
                let _ = self.event_tx.send(ScudEvent::AgentsLoaded(agents)).await;
            }
            Ok(Err(e)) => {
                error!("Failed to load agents: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to load agents: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Pause the currently running swarm
    async fn pause_swarm(&mut self) {
        if self.paused {
            info!("Swarm is already paused");
            return;
        }

        self.paused = true;
        info!("Pausing swarm execution");

        // Emit event to notify GUI
        let _ = self
            .event_tx
            .send(ScudEvent::Output("Swarm execution paused".to_string()))
            .await;

        // If we have a process handle, send SIGSTOP to pause it
        #[cfg(unix)]
        if let Some(ref handle) = self.swarm_handle {
            if let Some(pid) = handle.id() {
                // Send SIGSTOP to pause the process group
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGSTOP);
                }
                info!("Sent SIGSTOP to swarm process group {}", pid);
            }
        }
    }

    /// Resume a paused swarm
    async fn resume_swarm(&mut self) {
        if !self.paused {
            info!("Swarm is not paused");
            return;
        }

        self.paused = false;
        info!("Resuming swarm execution");

        // Emit event to notify GUI
        let _ = self
            .event_tx
            .send(ScudEvent::Output("Swarm execution resumed".to_string()))
            .await;

        // If we have a process handle, send SIGCONT to resume it
        #[cfg(unix)]
        if let Some(ref handle) = self.swarm_handle {
            if let Some(pid) = handle.id() {
                // Send SIGCONT to resume the process group
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGCONT);
                }
                info!("Sent SIGCONT to swarm process group {}", pid);
            }
        }
    }

    /// Stop the currently running swarm
    async fn stop_swarm(&mut self) {
        self.paused = false;
        if let Some(ref mut handle) = self.swarm_handle {
            info!("Stopping swarm process");
            if let Err(e) = handle.kill().await {
                warn!("Failed to kill swarm process: {}", e);
            }
            self.swarm_handle = None;
        }
    }

    /// Mark a task as complete using direct library calls
    async fn complete_task(&self, task_id: &str) {
        let task_id = task_id.to_string();
        let task_id_log = task_id.clone();
        debug!("Completing task {} via scud-core", task_id);

        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let storage = Storage::new(None);

            // Get the active group tag
            let tag = storage
                .get_active_group()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "No active task group".to_string())?;

            // Load the phase, update the task, and save
            let mut phase = storage.load_group(&tag).map_err(|e| e.to_string())?;

            let task = phase
                .get_task_mut(&task_id)
                .ok_or_else(|| format!("Task '{}' not found", task_id))?;

            task.set_status(TaskStatus::Done);

            storage
                .update_group(&tag, &phase)
                .map_err(|e| e.to_string())?;

            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Task {} marked as done", task_id_log);
            }
            Ok(Err(e)) => {
                error!("Failed to complete task: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to complete task: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Mark a task as blocked using direct library calls
    async fn block_task(&self, task_id: &str) {
        let task_id = task_id.to_string();
        let task_id_log = task_id.clone();
        debug!("Blocking task {} via scud-core", task_id);

        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let storage = Storage::new(None);

            // Get the active group tag
            let tag = storage
                .get_active_group()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "No active task group".to_string())?;

            // Load the phase, update the task, and save
            let mut phase = storage.load_group(&tag).map_err(|e| e.to_string())?;

            let task = phase
                .get_task_mut(&task_id)
                .ok_or_else(|| format!("Task '{}' not found", task_id))?;

            task.set_status(TaskStatus::Blocked);

            storage
                .update_group(&tag, &phase)
                .map_err(|e| e.to_string())?;

            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Task {} marked as blocked", task_id_log);
            }
            Ok(Err(e)) => {
                error!("Failed to block task: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to block task: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
            }
        }
    }

    /// Archive a tag by shelling out to `scud clean --tag <tag>`
    async fn archive_tag(&self, tag: &str) {
        let tag = tag.to_string();
        let tag_for_event = tag.clone();
        info!("Archiving tag '{}'", tag);

        let result = tokio::process::Command::new("scud")
            .args(["clean", "--tag", &tag])
            .output()
            .await;

        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("Tag '{}' archived successfully", tag_for_event);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::TagArchived {
                            tag: tag_for_event,
                        })
                        .await;
                    // Reload tags after archiving
                    self.load_available_tags().await;
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("Failed to archive tag '{}': {}", tag_for_event, stderr);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::Error(format!(
                            "Failed to archive tag '{}': {}",
                            tag_for_event, stderr
                        )))
                        .await;
                }
            }
            Err(e) => {
                error!("Failed to run scud clean: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to archive: {}", e)))
                    .await;
            }
        }
    }

    /// Attach to a session in interactive terminal mode
    ///
    /// Opens a new terminal window with the harness's resume command.
    async fn attach_session(&self, task_id: &str, harness_name: &str, session_id: &str) {
        info!(
            "Attaching to session {} for task {} (harness: {})",
            session_id, task_id, harness_name
        );

        // Build the command based on harness type
        let cmd_args: Vec<String> = match harness_name {
            "claude" => {
                vec!["claude".to_string(), "--resume".to_string(), session_id.to_string()]
            }
            "opencode" => {
                vec![
                    "opencode".to_string(),
                    "attach".to_string(),
                    "http://localhost:4096".to_string(),
                    "--session".to_string(),
                    session_id.to_string(),
                ]
            }
            "cursor" => {
                vec!["cursor-agent".to_string(), "--resume".to_string(), session_id.to_string()]
            }
            _ => {
                error!("Unknown harness for attach: {}", harness_name);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!(
                        "Cannot attach: unknown harness '{}'",
                        harness_name
                    )))
                    .await;
                return;
            }
        };

        // Open in a new terminal using open -a Terminal on macOS
        // The command is: open -a Terminal -- <binary> <args>...
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "tell application \"Terminal\"\n\
                    activate\n\
                    do script \"{}\"\n\
                end tell",
                cmd_args.join(" ").replace("\"", "\\\"")
            );

            let result = tokio::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .await;

            match result {
                Ok(output) => {
                    if output.status.success() {
                        info!("Opened terminal for session {}", session_id);
                        let _ = self
                            .event_tx
                            .send(ScudEvent::Output(format!(
                                "Opened terminal to attach to session {}",
                                session_id
                            )))
                            .await;
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("Failed to open terminal: {}", stderr);
                        let _ = self
                            .event_tx
                            .send(ScudEvent::Error(format!(
                                "Failed to open terminal: {}",
                                stderr
                            )))
                            .await;
                    }
                }
                Err(e) => {
                    error!("Failed to run osascript: {}", e);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::Error(format!("Failed to open terminal: {}", e)))
                        .await;
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // On Linux, try xterm or gnome-terminal
            let term_cmd = if std::process::Command::new("which")
                .arg("gnome-terminal")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                "gnome-terminal"
            } else {
                "xterm"
            };

            let result = tokio::process::Command::new(term_cmd)
                .arg("-e")
                .args(&cmd_args)
                .spawn();

            match result {
                Ok(_) => {
                    info!("Opened terminal for session {}", session_id);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::Output(format!(
                            "Opened terminal to attach to session {}",
                            session_id
                        )))
                        .await;
                }
                Err(e) => {
                    error!("Failed to open terminal: {}", e);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::Error(format!("Failed to open terminal: {}", e)))
                        .await;
                }
            }
        }
    }

    /// Run swarm execution using headless runners for each task
    ///
    /// Spawns headless runners per-task so each task gets its own StreamStore
    /// session visible in the Monitor view.
    async fn run_swarm(
        &mut self,
        tag: &str,
        harness_name: &str,
        round_size: usize,
        model: Option<String>,
    ) {
        info!(
            "Starting headless swarm for tag '{}' with harness '{}' (round_size={})",
            tag, harness_name, round_size
        );

        // Parse harness type
        let harness = match Harness::parse(harness_name) {
            Ok(h) => h,
            Err(e) => {
                error!("Invalid harness: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Invalid harness: {}", e)))
                    .await;
                return;
            }
        };

        // Load tasks and compute waves
        let tag_owned = tag.to_string();
        let wave_result =
            tokio::task::spawn_blocking(move || -> Result<(Vec<Vec<Task>>, Phase), String> {
                let storage = Storage::new(None);
                let phase = storage.load_group(&tag_owned).map_err(|e| e.to_string())?;

                let actionable: Vec<&Task> = phase.get_actionable_tasks();
                let pending_tasks: Vec<&Task> = actionable
                    .into_iter()
                    .filter(|t| {
                        matches!(
                            t.status,
                            TaskStatus::Pending | TaskStatus::InProgress | TaskStatus::Failed
                        )
                    })
                    .collect();

                let wave_result = compute_waves(&pending_tasks);

                // Convert wave IDs to full Task objects
                let waves: Vec<Vec<Task>> = wave_result
                    .waves
                    .into_iter()
                    .map(|wave| {
                        wave.tasks
                            .into_iter()
                            .filter_map(|task_id| phase.get_task(&task_id).cloned())
                            .collect()
                    })
                    .filter(|wave: &Vec<Task>| !wave.is_empty())
                    .collect();

                Ok((waves, phase))
            })
            .await;

        let (waves, _phase) = match wave_result {
            Ok(Ok((waves, phase))) => (waves, phase),
            Ok(Err(e)) => {
                error!("Failed to compute waves: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to compute waves: {}", e)))
                    .await;
                return;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
                return;
            }
        };

        let total_waves = waves.len();
        let _ = self
            .event_tx
            .send(ScudEvent::SwarmStarted {
                tag: tag.to_string(),
                total_waves,
            })
            .await;

        let mut all_success = true;

        for (wave_idx, wave_tasks) in waves.into_iter().enumerate() {
            let task_ids: Vec<String> = wave_tasks.iter().map(|t| t.id.clone()).collect();
            let _ = self
                .event_tx
                .send(ScudEvent::WaveStarted {
                    wave: wave_idx,
                    tasks: task_ids.clone(),
                })
                .await;

            // Process tasks in chunks of round_size
            for chunk in wave_tasks.chunks(round_size) {
                let mut handles = Vec::new();

                for task in chunk {
                    let task_id = task.id.clone();
                    let task_title = task.title.clone();
                    let task_description = task.description.clone();
                    let harness_copy = harness;
                    let event_tx = self.event_tx.clone();
                    let working_dir = self.working_dir.clone();
                    let stream_store = self.stream_store.clone();
                    let tag_str = tag.to_string();
                    let model_override = model.clone();

                    let handle = tokio::spawn(async move {
                        // Build prompt
                        let prompt = format!(
                            "Complete the following task:\n\n## Task: {}\n\n{}\n\nWhen done, run: scud set-status {} done",
                            task_title, task_description, task_id
                        );

                        Self::execute_headless_task(
                            &task_id,
                            &task_title,
                            &prompt,
                            harness_copy,
                            model_override.as_deref(),
                            &working_dir,
                            &tag_str,
                            Some(wave_idx),
                            &event_tx,
                            &stream_store,
                        )
                        .await
                    });

                    handles.push(handle);
                }

                // Wait for this chunk to complete
                for handle in handles {
                    match handle.await {
                        Ok(success) => {
                            if !success {
                                all_success = false;
                            }
                        }
                        Err(e) => {
                            error!("Task join error: {}", e);
                            all_success = false;
                        }
                    }
                }
            }

            let _ = self
                .event_tx
                .send(ScudEvent::WaveCompleted { wave: wave_idx })
                .await;
        }

        let _ = self
            .event_tx
            .send(ScudEvent::SwarmCompleted {
                success: all_success,
            })
            .await;
        info!("Headless swarm completed with success={}", all_success);
    }

    /// Run a single task with direct streaming output
    ///
    /// Uses the scud-cli headless infrastructure for proper event parsing.
    /// Spawns the appropriate harness (Claude/OpenCode) and streams structured
    /// events (text deltas, tool calls, completion) back to the GUI.
    async fn run_task(
        &mut self,
        task_id: &str,
        harness_name: &str,
        model: Option<String>,
    ) {
        info!(
            "Running task {} in headless mode with harness {}",
            task_id, harness_name
        );

        // Parse harness type
        let harness = match Harness::parse(harness_name) {
            Ok(h) => h,
            Err(e) => {
                error!("Invalid harness: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Invalid harness: {}", e)))
                    .await;
                return;
            }
        };

        // Load task details
        let task_id_clone = task_id.to_string();
        let task_result = tokio::task::spawn_blocking(move || -> Result<(Task, String), String> {
            let storage = Storage::new(None);
            let tag = storage
                .get_active_group()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "No active task group".to_string())?;
            let phase = storage.load_group(&tag).map_err(|e| e.to_string())?;
            let task = phase
                .get_task(&task_id_clone)
                .cloned()
                .ok_or_else(|| format!("Task '{}' not found", task_id_clone))?;
            Ok((task, tag))
        })
        .await;

        let (task, tag) = match task_result {
            Ok(Ok((t, tag))) => (t, tag),
            Ok(Err(e)) => {
                error!("Failed to load task: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to load task: {}", e)))
                    .await;
                return;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
                return;
            }
        };

        // Create stream session for this task
        self.stream_store.create_session(task_id, &tag);

        // Emit headless started event
        let _ = self
            .event_tx
            .send(ScudEvent::HeadlessStarted {
                task_id: task_id.to_string(),
                harness: harness_name.to_string(),
            })
            .await;

        // Emit task started event
        let _ = self
            .event_tx
            .send(ScudEvent::TaskStarted {
                task_id: task_id.to_string(),
            })
            .await;

        // Build prompt for the agent
        let prompt = format!(
            "Complete the following task:\n\n## Task: {}\n\n{}\n\nWhen done, run: scud set-status {} done",
            task.title, task.description, task_id
        );

        // Create the headless runner
        let runner: AnyRunner = match create_runner(harness) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to create runner: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to create runner: {}", e)))
                    .await;
                return;
            }
        };

        info!("Starting headless runner for task {}", task_id);

        // Start the session
        let mut session: SessionHandle = match runner
            .start(task_id, &prompt, &self.working_dir, model.as_deref())
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to start headless session: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!(
                        "Failed to start headless session: {}",
                        e
                    )))
                    .await;
                return;
            }
        };

        // Store PID for potential interruption
        if let Some(pid) = session.pid() {
            self.stream_store.set_pid(task_id, pid);
        }

        // Stream events from the session
        let task_id_owned = task_id.to_string();
        let event_tx = self.event_tx.clone();

        while let Some(stream_event) = session.events.recv().await {
            // Store event in stream store for persistence/replay
            self.stream_store
                .push_event(&task_id_owned, stream_event.clone());

            // Convert StreamEvent to ScudEvent and send to GUI
            match stream_event.kind {
                StreamEventKind::TextDelta { text } => {
                    let _ = event_tx
                        .send(ScudEvent::TaskOutput {
                            task_id: task_id_owned.clone(),
                            text,
                        })
                        .await;
                }
                StreamEventKind::ToolStart {
                    tool_name,
                    tool_id,
                    input_summary,
                } => {
                    let _ = event_tx
                        .send(ScudEvent::ToolStart {
                            task_id: task_id_owned.clone(),
                            tool_name,
                            tool_id,
                            input_summary,
                        })
                        .await;
                }
                StreamEventKind::ToolResult {
                    tool_name,
                    tool_id,
                    success,
                } => {
                    let _ = event_tx
                        .send(ScudEvent::ToolResult {
                            task_id: task_id_owned.clone(),
                            tool_name,
                            tool_id,
                            success,
                        })
                        .await;
                }
                StreamEventKind::SessionAssigned { session_id } => {
                    self.stream_store
                        .set_session_id(&task_id_owned, &session_id);
                    let _ = event_tx
                        .send(ScudEvent::SessionAssigned {
                            task_id: task_id_owned.clone(),
                            session_id,
                        })
                        .await;
                }
                StreamEventKind::Complete { success } => {
                    let _ = event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id_owned.clone(),
                            success,
                        })
                        .await;
                    if success {
                        info!("Headless task {} completed successfully", task_id_owned);
                    } else {
                        warn!("Headless task {} completed with failure", task_id_owned);
                    }
                    break;
                }
                StreamEventKind::Error { message } => {
                    error!("Headless task {} error: {}", task_id_owned, message);
                    let _ = event_tx
                        .send(ScudEvent::Error(format!(
                            "Task {} error: {}",
                            task_id_owned, message
                        )))
                        .await;
                    let _ = event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id_owned.clone(),
                            success: false,
                        })
                        .await;
                    break;
                }
            }
        }

        // Wait for the session to fully complete
        match session.wait().await {
            Ok(success) => {
                info!(
                    "Headless session for task {} finished with success={}",
                    task_id, success
                );
            }
            Err(e) => {
                warn!("Error waiting for headless session to complete: {}", e);
            }
        }

        // Save session metadata for potential continuation
        if let Err(e) = self
            .stream_store
            .save_session_metadata(task_id, &self.working_dir)
        {
            debug!("Failed to save session metadata: {}", e);
        }
    }

    /// Execute a single headless task, streaming events to the GUI
    ///
    /// Shared helper used by both run_swarm and run_ralph.
    /// Returns true if the task completed successfully.
    #[allow(clippy::too_many_arguments)]
    async fn execute_headless_task(
        task_id: &str,
        _task_title: &str,
        prompt: &str,
        harness: Harness,
        model: Option<&str>,
        working_dir: &Path,
        tag: &str,
        _wave: Option<usize>,
        event_tx: &mpsc::Sender<ScudEvent>,
        stream_store: &StreamStore,
    ) -> bool {
        let harness_name = harness.name().to_string();

        // Create stream session
        stream_store.create_session(task_id, tag);

        // Emit headless started
        let _ = event_tx
            .send(ScudEvent::HeadlessStarted {
                task_id: task_id.to_string(),
                harness: harness_name.clone(),
            })
            .await;

        let _ = event_tx
            .send(ScudEvent::TaskStarted {
                task_id: task_id.to_string(),
            })
            .await;

        // Create runner
        let runner: AnyRunner = match create_runner(harness) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to create runner for task {}: {}", task_id, e);
                let _ = event_tx
                    .send(ScudEvent::TaskCompleted {
                        task_id: task_id.to_string(),
                        success: false,
                    })
                    .await;
                return false;
            }
        };

        // Start session
        let mut session: SessionHandle = match runner
            .start(task_id, prompt, working_dir, model)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Failed to start headless session for task {}: {}",
                    task_id, e
                );
                let _ = event_tx
                    .send(ScudEvent::TaskCompleted {
                        task_id: task_id.to_string(),
                        success: false,
                    })
                    .await;
                return false;
            }
        };

        // Store PID
        if let Some(pid) = session.pid() {
            stream_store.set_pid(task_id, pid);
        }

        // Stream events
        let mut task_success = false;
        while let Some(stream_event) = session.events.recv().await {
            stream_store.push_event(task_id, stream_event.clone());

            match stream_event.kind {
                StreamEventKind::TextDelta { text } => {
                    let _ = event_tx
                        .send(ScudEvent::TaskOutput {
                            task_id: task_id.to_string(),
                            text,
                        })
                        .await;
                }
                StreamEventKind::ToolStart {
                    tool_name,
                    tool_id,
                    input_summary,
                } => {
                    let _ = event_tx
                        .send(ScudEvent::ToolStart {
                            task_id: task_id.to_string(),
                            tool_name,
                            tool_id,
                            input_summary,
                        })
                        .await;
                }
                StreamEventKind::ToolResult {
                    tool_name,
                    tool_id,
                    success,
                } => {
                    let _ = event_tx
                        .send(ScudEvent::ToolResult {
                            task_id: task_id.to_string(),
                            tool_name,
                            tool_id,
                            success,
                        })
                        .await;
                }
                StreamEventKind::SessionAssigned { session_id } => {
                    stream_store.set_session_id(task_id, &session_id);
                    let _ = event_tx
                        .send(ScudEvent::SessionAssigned {
                            task_id: task_id.to_string(),
                            session_id,
                        })
                        .await;
                }
                StreamEventKind::Complete { success } => {
                    task_success = success;
                    let _ = event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id.to_string(),
                            success,
                        })
                        .await;
                    break;
                }
                StreamEventKind::Error { message } => {
                    let _ = event_tx
                        .send(ScudEvent::Error(format!(
                            "Task {} error: {}",
                            task_id, message
                        )))
                        .await;
                    let _ = event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id.to_string(),
                            success: false,
                        })
                        .await;
                    break;
                }
            }
        }

        // Wait for session to finish
        let _ = session.wait().await;
        task_success
    }

    /// Run Ralph mode: sequential task execution with backpressure validation and repair
    async fn run_ralph(
        &mut self,
        tag: &str,
        harness_name: &str,
        model: Option<String>,
        ralph_config: RalphConfig,
    ) {
        info!(
            "Starting Ralph mode for tag '{}' with harness '{}' (max_iterations={})",
            tag, harness_name, ralph_config.max_iterations
        );

        // Parse harness type
        let harness = match Harness::parse(harness_name) {
            Ok(h) => h,
            Err(e) => {
                error!("Invalid harness: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Invalid harness: {}", e)))
                    .await;
                return;
            }
        };

        // Load backpressure config (sync, needs spawn_blocking)
        let working_dir = self.working_dir.clone();
        let bp_config = if ralph_config.validate {
            let wd = working_dir.clone();
            match tokio::task::spawn_blocking(move || {
                scud::backpressure::BackpressureConfig::load(Some(&wd))
            })
            .await
            {
                Ok(Ok(config)) if !config.commands.is_empty() => Some(config),
                Ok(Ok(_)) => {
                    info!("No backpressure commands found, validation disabled");
                    None
                }
                Ok(Err(e)) => {
                    warn!("Failed to load backpressure config: {}, validation disabled", e);
                    None
                }
                Err(e) => {
                    warn!("Spawn error loading backpressure config: {}, validation disabled", e);
                    None
                }
            }
        } else {
            None
        };

        // Reset cancellation flag
        self.ralph_cancelled.store(false, Ordering::SeqCst);

        let max_iterations = ralph_config.max_iterations;
        let _ = self
            .event_tx
            .send(ScudEvent::RalphStarted {
                tag: tag.to_string(),
                max_iterations,
            })
            .await;

        let mut completed_count: usize = 0;
        let mut failed_count: usize = 0;

        for iteration in 1..=max_iterations {
            // Check cancellation
            if self.ralph_cancelled.load(Ordering::SeqCst) {
                info!("Ralph cancelled at iteration {}", iteration);
                break;
            }

            // Find next task (spawn_blocking)
            let tag_owned = tag.to_string();
            let next_task = tokio::task::spawn_blocking(move || -> Option<Task> {
                let storage = Storage::new(None);
                let phase = match storage.load_group(&tag_owned) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Failed to load phase: {}", e);
                        return None;
                    }
                };

                // Get actionable tasks sorted by dependency order
                let actionable = phase.get_actionable_tasks();
                let pending: Vec<&Task> = actionable
                    .into_iter()
                    .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Failed))
                    .collect();

                if pending.is_empty() {
                    return None;
                }

                // Compute waves to get dependency order, take first task from first wave
                let wave_result = compute_waves(&pending);
                wave_result
                    .waves
                    .into_iter()
                    .flat_map(|w| w.tasks)
                    .next()
                    .and_then(|id| phase.get_task(&id).cloned())
            })
            .await;

            let task = match next_task {
                Ok(Some(t)) => t,
                Ok(None) => {
                    info!("No more pending tasks, Ralph loop complete");
                    break;
                }
                Err(e) => {
                    error!("Spawn error finding next task: {}", e);
                    break;
                }
            };

            let task_id = task.id.clone();
            let task_title = task.title.clone();

            // Mark task in-progress
            {
                let tid = task_id.clone();
                let tag_owned = tag.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(None);
                    if let Ok(mut phase) = storage.load_group(&tag_owned) {
                        if let Some(t) = phase.get_task_mut(&tid) {
                            t.set_status(TaskStatus::InProgress);
                            let _ = storage.update_group(&tag_owned, &phase);
                        }
                    }
                })
                .await;
            }

            let _ = self
                .event_tx
                .send(ScudEvent::RalphIterationStarted {
                    iteration,
                    task_id: task_id.clone(),
                    task_title: task_title.clone(),
                })
                .await;

            // Resolve agent config for this task
            // Convert scud_core::Task to scud::models::Task via JSON round-trip
            // (both types derive Serialize/Deserialize with the same schema)
            let resolved = {
                let task_json = serde_json::to_value(&task).ok();
                let tag_str = tag.to_string();
                let harness_copy = harness;
                let model_clone = model.clone();
                let wd = working_dir.clone();
                tokio::task::spawn_blocking(move || {
                    if let Some(json) = task_json {
                        if let Ok(scud_task) = serde_json::from_value::<scud::models::Task>(json) {
                            return Some(scud::commands::spawn::agent::resolve_agent_config(
                                &scud_task,
                                &tag_str,
                                harness_copy,
                                model_clone.as_deref(),
                                &wd,
                            ));
                        }
                    }
                    None
                })
                .await
            };

            let resolved = match resolved {
                Ok(Some(r)) => r,
                Ok(None) | Err(_) => {
                    // Fallback: build a simple prompt manually
                    scud::commands::spawn::agent::ResolvedAgentConfig {
                        harness,
                        model: model.clone(),
                        prompt: format!(
                            "Complete the following task:\n\n## Task: {}\n\n{}\n\nWhen done, run: scud set-status {} done",
                            task_title, task.description, task_id
                        ),
                        from_agent_def: false,
                        agent_type: task.agent_type.clone(),
                    }
                }
            };

            // Execute the task
            let success = Self::execute_headless_task(
                &task_id,
                &task_title,
                &resolved.prompt,
                resolved.harness,
                resolved.model.as_deref(),
                &working_dir,
                tag,
                None, // no wave number in Ralph mode
                &self.event_tx,
                &self.stream_store,
            )
            .await;

            if !success {
                // Mark task failed
                let tid = task_id.clone();
                let tag_owned = tag.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(None);
                    if let Ok(mut phase) = storage.load_group(&tag_owned) {
                        if let Some(t) = phase.get_task_mut(&tid) {
                            t.set_status(TaskStatus::Failed);
                            let _ = storage.update_group(&tag_owned, &phase);
                        }
                    }
                })
                .await;
                failed_count += 1;
                let _ = self
                    .event_tx
                    .send(ScudEvent::RalphIterationCompleted {
                        iteration,
                        task_id: task_id.clone(),
                        success: false,
                    })
                    .await;
                continue;
            }

            // Validation phase
            let mut validation_passed = true;
            if let Some(ref bp) = bp_config {
                if self.ralph_cancelled.load(Ordering::SeqCst) {
                    break;
                }

                let _ = self
                    .event_tx
                    .send(ScudEvent::RalphValidationStarted {
                        task_id: task_id.clone(),
                    })
                    .await;

                let wd = working_dir.clone();
                let bp_clone = bp.clone();
                let val_result = tokio::task::spawn_blocking(move || {
                    scud::backpressure::run_validation(&wd, &bp_clone)
                })
                .await;

                match val_result {
                    Ok(Ok(result)) => {
                        validation_passed = result.all_passed;
                        let output = if result.all_passed {
                            "All checks passed".to_string()
                        } else {
                            format!("Failed: {}", result.failures.join(", "))
                        };

                        let _ = self
                            .event_tx
                            .send(ScudEvent::RalphValidationCompleted {
                                task_id: task_id.clone(),
                                passed: result.all_passed,
                                output: output.clone(),
                            })
                            .await;

                        // Append validation output to session
                        let _ = self.event_tx.send(ScudEvent::TaskOutput {
                            task_id: task_id.clone(),
                            text: format!("\n--- VALIDATION ---\n{}\n", output),
                        }).await;

                        // Repair loop
                        if !result.all_passed && ralph_config.repair {
                            let mut repair_results = result.results;
                            for attempt in 1..=ralph_config.max_repair_attempts {
                                if self.ralph_cancelled.load(Ordering::SeqCst) {
                                    break;
                                }

                                let _ = self
                                    .event_tx
                                    .send(ScudEvent::RalphRepairStarted {
                                        task_id: task_id.clone(),
                                        attempt,
                                    })
                                    .await;

                                let _ = self.event_tx.send(ScudEvent::TaskOutput {
                                    task_id: task_id.clone(),
                                    text: format!("\n--- REPAIR ATTEMPT {} ---\n", attempt),
                                }).await;

                                // Build repair prompt from failures
                                let failure_details: Vec<String> = repair_results
                                    .iter()
                                    .filter(|r| !r.passed)
                                    .map(|r| {
                                        format!(
                                            "Command `{}` failed (exit {}):\nstdout: {}\nstderr: {}",
                                            r.command,
                                            r.exit_code.unwrap_or(-1),
                                            r.stdout,
                                            r.stderr
                                        )
                                    })
                                    .collect();

                                let repair_prompt = format!(
                                    "The previous task '{}' completed but validation failed. Fix the issues:\n\n{}\n\nMake minimal, targeted fixes to pass validation.",
                                    task_title,
                                    failure_details.join("\n\n")
                                );

                                // Run repair agent (same task_id to reuse session in monitor)
                                let repair_success = Self::execute_headless_task(
                                    &task_id,
                                    &format!("{} (repair #{})", task_title, attempt),
                                    &repair_prompt,
                                    resolved.harness,
                                    resolved.model.as_deref(),
                                    &working_dir,
                                    tag,
                                    None,
                                    &self.event_tx,
                                    &self.stream_store,
                                )
                                .await;

                                if !repair_success {
                                    continue;
                                }

                                // Re-validate
                                let wd = working_dir.clone();
                                let bp_clone = bp.clone();
                                let reval = tokio::task::spawn_blocking(move || {
                                    scud::backpressure::run_validation(&wd, &bp_clone)
                                })
                                .await;

                                match reval {
                                    Ok(Ok(r)) => {
                                        let out = if r.all_passed {
                                            "All checks passed".to_string()
                                        } else {
                                            format!("Failed: {}", r.failures.join(", "))
                                        };
                                        let _ = self
                                            .event_tx
                                            .send(ScudEvent::RalphValidationCompleted {
                                                task_id: task_id.clone(),
                                                passed: r.all_passed,
                                                output: out.clone(),
                                            })
                                            .await;
                                        let _ = self.event_tx.send(ScudEvent::TaskOutput {
                                            task_id: task_id.clone(),
                                            text: format!("\n--- VALIDATION ---\n{}\n", out),
                                        }).await;

                                        if r.all_passed {
                                            validation_passed = true;
                                            break;
                                        }
                                        repair_results = r.results;
                                    }
                                    _ => {
                                        // Validation itself errored, treat as failed
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("Validation error: {}, treating as passed", e);
                    }
                    Err(e) => {
                        warn!("Spawn error during validation: {}, treating as passed", e);
                    }
                }
            }

            if !validation_passed {
                // Mark task failed
                let tid = task_id.clone();
                let tag_owned = tag.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(None);
                    if let Ok(mut phase) = storage.load_group(&tag_owned) {
                        if let Some(t) = phase.get_task_mut(&tid) {
                            t.set_status(TaskStatus::Failed);
                            let _ = storage.update_group(&tag_owned, &phase);
                        }
                    }
                })
                .await;
                failed_count += 1;
                let _ = self
                    .event_tx
                    .send(ScudEvent::RalphIterationCompleted {
                        iteration,
                        task_id: task_id.clone(),
                        success: false,
                    })
                    .await;
                continue;
            }

            // Mark task done
            {
                let tid = task_id.clone();
                let tag_owned = tag.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(None);
                    if let Ok(mut phase) = storage.load_group(&tag_owned) {
                        if let Some(t) = phase.get_task_mut(&tid) {
                            t.set_status(TaskStatus::Done);
                            let _ = storage.update_group(&tag_owned, &phase);
                        }
                    }
                })
                .await;
            }

            // Git push if configured
            if ralph_config.git_push {
                let wd = working_dir.clone();
                let _ = tokio::process::Command::new("git")
                    .arg("push")
                    .current_dir(&wd)
                    .output()
                    .await;
            }

            completed_count += 1;
            let _ = self
                .event_tx
                .send(ScudEvent::RalphIterationCompleted {
                    iteration,
                    task_id: task_id.clone(),
                    success: true,
                })
                .await;
        }

        let _ = self
            .event_tx
            .send(ScudEvent::RalphCompleted {
                iterations: completed_count + failed_count,
                completed: completed_count,
                failed: failed_count,
            })
            .await;

        info!(
            "Ralph loop finished: {} completed, {} failed",
            completed_count, failed_count
        );
    }

    #[allow(dead_code)]
    /// Get the stream store for external access (e.g., for TUI integration)
    pub fn stream_store(&self) -> &StreamStore {
        &self.stream_store
    }

    #[allow(dead_code)]
    /// Get headless session output for a task
    pub fn get_headless_output(&self, task_id: &str, limit: usize) -> Vec<String> {
        self.stream_store.get_output(task_id, limit)
    }

    #[allow(dead_code)]
    /// Check if a headless session is active for a task
    pub fn is_headless_active(&self, task_id: &str) -> bool {
        self.stream_store
            .get_status(task_id)
            .map(|s| matches!(s, SessionStatus::Starting | SessionStatus::Running))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scud_json_event_parsing() {
        let swarm_started = r#"{"event": "swarm_started", "tag": "feature", "total_waves": 3}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(swarm_started).unwrap();
        match parsed {
            ScudJsonEvent::SwarmStarted { tag, total_waves } => {
                assert_eq!(tag, "feature");
                assert_eq!(total_waves, 3);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_task_started_parsing() {
        let task_started = r#"{"event": "task_started", "task_id": "1.2"}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(task_started).unwrap();
        match parsed {
            ScudJsonEvent::TaskStarted { task_id } => {
                assert_eq!(task_id, "1.2");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_task_completed_parsing() {
        let task_completed = r#"{"event": "task_completed", "task_id": "1", "success": true}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(task_completed).unwrap();
        match parsed {
            ScudJsonEvent::TaskCompleted { task_id, success } => {
                assert_eq!(task_id, "1");
                assert!(success);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_validation_completed_parsing() {
        let validation =
            r#"{"event": "validation_completed", "passed": false, "output": "Build failed"}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(validation).unwrap();
        match parsed {
            ScudJsonEvent::ValidationCompleted { passed, output } => {
                assert!(!passed);
                assert_eq!(output, "Build failed");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_wave_events_parsing() {
        let wave_started = r#"{"event": "wave_started", "wave": 0, "tasks": ["1", "2"]}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(wave_started).unwrap();
        match parsed {
            ScudJsonEvent::WaveStarted { wave, tasks } => {
                assert_eq!(wave, 0);
                assert_eq!(tasks, vec!["1", "2"]);
            }
            _ => panic!("Wrong event type"),
        }

        let wave_completed = r#"{"event": "wave_completed", "wave": 0}"#;
        let parsed: ScudJsonEvent = serde_json::from_str(wave_completed).unwrap();
        match parsed {
            ScudJsonEvent::WaveCompleted { wave } => {
                assert_eq!(wave, 0);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_task_info_from_json() {
        let json_task = ScudJsonTask {
            id: "1".to_string(),
            title: "Test task".to_string(),
            status: "Pending".to_string(),
            dependencies: vec!["0".to_string()],
            priority: Some("High".to_string()),
            complexity: Some(3),
            agent_type: Some("builder".to_string()),
        };

        let task_info: TaskInfo = json_task.into();
        assert_eq!(task_info.id, "1");
        assert_eq!(task_info.title, "Test task");
        assert_eq!(task_info.status, "Pending");
        assert_eq!(task_info.agent, Some("builder".to_string()));
    }

    #[test]
    fn test_scud_event_conversion() {
        let json_event = ScudJsonEvent::TaskStarted {
            task_id: "test-123".to_string(),
        };

        let scud_event: ScudEvent = json_event.into();
        match scud_event {
            ScudEvent::TaskStarted { task_id } => {
                assert_eq!(task_id, "test-123");
            }
            _ => panic!("Wrong event type"),
        }
    }

    // Headless streaming event tests

    #[test]
    fn test_headless_started_event() {
        let event = ScudEvent::HeadlessStarted {
            task_id: "task-1".to_string(),
            harness: "claude".to_string(),
        };
        match event {
            ScudEvent::HeadlessStarted { task_id, harness } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(harness, "claude");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_tool_start_event() {
        let event = ScudEvent::ToolStart {
            task_id: "task-1".to_string(),
            tool_name: "Read".to_string(),
            tool_id: "tool_123".to_string(),
            input_summary: "{path}".to_string(),
        };
        match event {
            ScudEvent::ToolStart {
                task_id,
                tool_name,
                tool_id,
                input_summary,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_id, "tool_123");
                assert_eq!(input_summary, "{path}");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_tool_result_event() {
        let event = ScudEvent::ToolResult {
            task_id: "task-1".to_string(),
            tool_name: "Bash".to_string(),
            tool_id: "tool_456".to_string(),
            success: true,
        };
        match event {
            ScudEvent::ToolResult {
                task_id,
                tool_name,
                tool_id,
                success,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_id, "tool_456");
                assert!(success);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_session_assigned_event() {
        let event = ScudEvent::SessionAssigned {
            task_id: "task-1".to_string(),
            session_id: "sess_abc123".to_string(),
        };
        match event {
            ScudEvent::SessionAssigned {
                task_id,
                session_id,
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(session_id, "sess_abc123");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_bridge_creation_with_working_dir() {
        let temp_dir = std::env::temp_dir();
        let (bridge, _cmd_tx, _event_rx) = ScudBridge::create_with_working_dir(temp_dir.clone());
        assert_eq!(bridge.working_dir, temp_dir);
    }

    #[test]
    fn test_stream_store_initialization() {
        let (bridge, _cmd_tx, _event_rx) = ScudBridge::create();
        // Stream store should be accessible and empty
        assert!(bridge.stream_store().all_tasks().is_empty());
    }

    #[test]
    fn test_is_headless_active_no_session() {
        let (bridge, _cmd_tx, _event_rx) = ScudBridge::create();
        // No session exists, should return false
        assert!(!bridge.is_headless_active("nonexistent-task"));
    }

    #[test]
    fn test_get_headless_output_no_session() {
        let (bridge, _cmd_tx, _event_rx) = ScudBridge::create();
        // No session exists, should return empty vec
        let output = bridge.get_headless_output("nonexistent-task", 100);
        assert!(output.is_empty());
    }
}
