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

use scud::storage::Storage as CliStorage;
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

    /// PRD files discovered from scanning
    PrdFilesFound(Vec<PathBuf>),

    /// Generate pipeline status update
    GenerateStatus(String),

    /// Generate pipeline completed
    GenerateCompleted(Result<(), String>),

    /// Tag summaries loaded for the tag explorer
    TagSummariesLoaded(Vec<crate::state::TagSummary>),

    /// Archives loaded for the tag explorer
    ArchivesLoaded(Vec<crate::state::ArchiveEntry>),

    /// Active tag changed
    ActiveTagChanged(String),

    /// Archive restored
    ArchiveRestored(Result<Vec<String>, String>),

    /// Backpressure config loaded
    BackpressureConfigLoaded {
        commands: Vec<String>,
        stop_on_failure: bool,
        timeout_secs: u64,
        is_auto_detected: bool,
    },

    /// Backpressure config saved
    BackpressureConfigSaved(Result<(), String>),

    /// A single line of generate output (streaming)
    GenerateOutputLine(String),

    /// Project is not initialized (no .scud/ directory)
    ProjectNotInitialized,

    /// Project was initialized successfully
    ProjectInitialized(Result<(), String>),

    /// LLM config loaded from .scud/config.toml
    LlmConfigLoaded {
        provider: String,
        model: String,
        smart_provider: String,
        smart_model: String,
        fast_provider: String,
        fast_model: String,
        max_tokens: String,
    },

    /// LLM config saved
    LlmConfigSaved(Result<(), String>),

    // Ralph mode events
    /// Ralph loop started
    RalphStarted { tag: String, max_iterations: usize },

    /// A new Ralph iteration began (one task)
    RalphIterationStarted {
        iteration: usize,
        task_id: String,
        task_title: String,
    },

    /// Backpressure validation started for a task
    RalphValidationStarted { task_id: String },

    /// Backpressure validation completed
    RalphValidationCompleted {
        task_id: String,
        passed: bool,
        output: String,
    },

    /// Repair agent spawned for a failed validation
    RalphRepairStarted { task_id: String, attempt: usize },

    /// A Ralph iteration completed (task done or failed)
    RalphIterationCompleted {
        iteration: usize,
        task_id: String,
        success: bool,
    },

    /// Ralph loop finished
    RalphCompleted {
        iterations: usize,
        completed: usize,
        failed: usize,
    },
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

    /// Update the bridge working directory (used when switching projects)
    SetWorkingDirectory { path: PathBuf },

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
        terminal_app: String,
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

    /// Stop a specific headless session by killing its process
    StopSession { task_id: String },

    /// Scan directories for PRD markdown files
    ScanPrdFiles,

    /// Run generate pipeline
    RunGenerate {
        prd_file: PathBuf,
        tag: String,
        num_tasks: u32,
        no_expand: bool,
        no_check_deps: bool,
        append: bool,
    },

    /// Load tag summaries for the tag explorer
    LoadTagSummaries,

    /// Load archives for the tag explorer
    LoadArchives,

    /// Set the active tag
    SetActiveTag { tag: String },

    /// Restore an archive
    RestoreArchive { filename: String },

    /// Load backpressure configuration (from config or auto-detect)
    LoadBackpressureConfig,

    /// Save backpressure configuration to .scud/config.toml
    SaveBackpressureConfig {
        commands: Vec<String>,
        stop_on_failure: bool,
        timeout_secs: u64,
    },

    /// Initialize the project (create .scud/ directory)
    InitProject,

    /// Load LLM configuration from .scud/config.toml
    LoadLlmConfig,

    /// Save LLM configuration to .scud/config.toml
    SaveLlmConfig {
        provider: String,
        model: String,
        smart_provider: String,
        smart_model: String,
        fast_provider: String,
        fast_model: String,
        max_tokens: String,
    },
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

fn reconcile_task_success(
    process_success: bool,
    stream_terminal: bool,
    stream_success: bool,
) -> bool {
    process_success && (!stream_terminal || stream_success)
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

    /// Cancellation flag for swarm mode
    swarm_cancelled: Arc<AtomicBool>,

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
            swarm_cancelled: Arc::new(AtomicBool::new(false)),
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
            swarm_cancelled: Arc::new(AtomicBool::new(false)),
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
                ScudCommand::SetWorkingDirectory { path } => {
                    info!(
                        "ScudBridge switching working directory to {}",
                        path.display()
                    );
                    self.working_dir = path;
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
                    // Reset cancellation and spawn as background task
                    // so the command loop stays responsive
                    self.swarm_cancelled.store(false, Ordering::SeqCst);
                    let event_tx = self.event_tx.clone();
                    let stream_store = self.stream_store.clone();
                    let working_dir = self.working_dir.clone();
                    let cancelled = self.swarm_cancelled.clone();
                    tokio::spawn(async move {
                        Self::run_swarm_bg(
                            &tag,
                            &harness,
                            round_size,
                            model_override,
                            event_tx,
                            stream_store,
                            working_dir,
                            cancelled,
                        )
                        .await;
                    });
                }
                ScudCommand::PauseSwarm => {
                    info!("PauseSwarm received (pause not yet supported for background swarm)");
                }
                ScudCommand::ResumeSwarm => {
                    info!("ResumeSwarm received (resume not yet supported for background swarm)");
                }
                ScudCommand::StopSwarm => {
                    info!("StopSwarm received, setting cancellation flag");
                    self.swarm_cancelled.store(true, Ordering::SeqCst);
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
                    terminal_app,
                } => {
                    self.attach_session(&task_id, &harness, &session_id, &terminal_app)
                        .await;
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
                ScudCommand::StopSession { task_id } => {
                    self.stop_session(&task_id).await;
                }
                ScudCommand::ScanPrdFiles => {
                    self.scan_prd_files().await;
                }
                ScudCommand::RunGenerate {
                    prd_file,
                    tag,
                    num_tasks,
                    no_expand,
                    no_check_deps,
                    append,
                } => {
                    self.run_generate(&prd_file, &tag, num_tasks, no_expand, no_check_deps, append)
                        .await;
                }
                ScudCommand::LoadTagSummaries => {
                    self.load_tag_summaries().await;
                }
                ScudCommand::LoadArchives => {
                    self.load_archives().await;
                }
                ScudCommand::SetActiveTag { tag } => {
                    self.set_active_tag(&tag).await;
                }
                ScudCommand::RestoreArchive { filename } => {
                    self.restore_archive(&filename).await;
                }
                ScudCommand::LoadBackpressureConfig => {
                    self.load_backpressure_config().await;
                }
                ScudCommand::SaveBackpressureConfig {
                    commands,
                    stop_on_failure,
                    timeout_secs,
                } => {
                    self.save_backpressure_config(commands, stop_on_failure, timeout_secs)
                        .await;
                }
                ScudCommand::InitProject => {
                    self.init_project().await;
                }
                ScudCommand::LoadLlmConfig => {
                    self.load_llm_config().await;
                }
                ScudCommand::SaveLlmConfig {
                    provider,
                    model,
                    smart_provider,
                    smart_model,
                    fast_provider,
                    fast_model,
                    max_tokens,
                } => {
                    self.save_llm_config(
                        provider,
                        model,
                        smart_provider,
                        smart_model,
                        fast_provider,
                        fast_model,
                        max_tokens,
                    )
                    .await;
                }
            }
        }

        info!("ScudBridge shutting down");
    }

    /// Load tasks from SCUD storage using direct library calls
    ///
    /// Uses Storage to load the active Phase, converts tasks to TaskInfo,
    /// and also computes waves so the Waves view shows proper groupings.
    ///
    /// When tag is None, loads the active group and also reports the active tag
    /// back to the GUI via ActiveTagChanged so the GUI state stays in sync.
    async fn load_tasks(&self, tag: Option<String>) {
        debug!("Loading tasks via scud-core (tag: {:?})", tag);

        // Run blocking storage operations in a spawn_blocking task
        // Returns (flat task list, computed waves, resolved tag name)
        let working_dir = self.working_dir.clone();
        #[allow(clippy::type_complexity)]
        let result: Result<
            Result<(Vec<TaskInfo>, Vec<Vec<TaskInfo>>, Option<String>), String>,
            _,
        > = tokio::task::spawn_blocking(
            move || -> Result<(Vec<TaskInfo>, Vec<Vec<TaskInfo>>, Option<String>), String> {
                let storage = Storage::new(Some(working_dir.clone()));

                // If storage isn't initialized, signal the GUI and return empty
                if !storage.is_initialized() {
                    return Err("__not_initialized__".to_string());
                }

                // Resolve the active tag name only on initial load (tag=None)
                // so the GUI knows which tag is active. When tag is Some,
                // the GUI already knows - don't re-emit to avoid loops.
                let resolved_tag = if tag.is_none() {
                    let cli_storage = CliStorage::new(Some(working_dir));
                    cli_storage.get_active_group().ok().flatten()
                } else {
                    None
                };

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

                Ok((all_tasks, waves, resolved_tag))
            },
        )
        .await;

        match result {
            Ok(Ok((task_infos, waves, resolved_tag))) => {
                // If we resolved the active tag, notify the GUI so it stays in sync
                if let Some(tag) = resolved_tag {
                    let _ = self.event_tx.send(ScudEvent::ActiveTagChanged(tag)).await;
                }
                let _ = self.event_tx.send(ScudEvent::TasksLoaded(task_infos)).await;
                if !waves.is_empty() {
                    let _ = self.event_tx.send(ScudEvent::WavesComputed(waves)).await;
                }
            }
            Ok(Err(e)) if e == "__not_initialized__" => {
                let _ = self.event_tx.send(ScudEvent::ProjectNotInitialized).await;
                let _ = self.event_tx.send(ScudEvent::TasksLoaded(Vec::new())).await;
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
        let working_dir = self.working_dir.clone();
        debug!("Computing waves via scud-core for tag: {}", tag);

        // Run blocking storage operations in a spawn_blocking task
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<TaskInfo>>, String> {
            let storage = Storage::new(Some(working_dir));
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
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
            let storage = Storage::new(Some(working_dir));
            if !storage.is_initialized() {
                return Ok(Vec::new());
            }
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

    /// Mark a task as complete using direct library calls
    async fn complete_task(&self, task_id: &str) {
        let task_id = task_id.to_string();
        let task_id_log = task_id.clone();
        let working_dir = self.working_dir.clone();
        debug!("Completing task {} via scud-core", task_id);

        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let storage = Storage::new(Some(working_dir));

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
        let working_dir = self.working_dir.clone();
        debug!("Blocking task {} via scud-core", task_id);

        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let storage = Storage::new(Some(working_dir));

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
            .args(["clean", "--tag", &tag, "--force"])
            .output()
            .await;

        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("Tag '{}' archived successfully", tag_for_event);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::TagArchived { tag: tag_for_event })
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
    async fn attach_session(
        &self,
        task_id: &str,
        harness_name: &str,
        session_id: &str,
        terminal_app: &str,
    ) {
        info!(
            "Attaching to session {} for task {} (harness: {})",
            session_id, task_id, harness_name
        );

        // Build the command based on harness type
        let cmd_args: Vec<String> = match harness_name {
            "claude" => {
                vec![
                    "claude".to_string(),
                    "--resume".to_string(),
                    session_id.to_string(),
                ]
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
                vec![
                    "cursor-agent".to_string(),
                    "--resume".to_string(),
                    session_id.to_string(),
                ]
            }
            "rho" => vec![
                "rho-cli".to_string(),
                "--resume".to_string(),
                session_id.to_string(),
            ],
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

        // Open in user's preferred terminal, cd to working directory first
        #[cfg(target_os = "macos")]
        {
            let working_dir_str = self.working_dir.display().to_string();
            let shell_cmd = format!(
                "cd '{}' && {}",
                working_dir_str.replace("'", "'\\''"),
                cmd_args.join(" ")
            );

            // Build AppleScript based on terminal app
            let script = if terminal_app.contains("iTerm") {
                // iTerm2 uses its own AppleScript dictionary
                format!(
                    "tell application \"{}\"\n\
                        activate\n\
                        create window with default profile command \"{}\"\n\
                    end tell",
                    terminal_app,
                    shell_cmd.replace("\\", "\\\\").replace("\"", "\\\"")
                )
            } else {
                // Terminal.app and others that support `do script`
                format!(
                    "tell application \"{}\"\n\
                        activate\n\
                        do script \"{}\"\n\
                    end tell",
                    terminal_app,
                    shell_cmd.replace("\\", "\\\\").replace("\"", "\\\"")
                )
            };

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

    /// Stop a specific headless session by killing its process
    async fn stop_session(&self, task_id: &str) {
        info!("Stopping session for task {}", task_id);

        if let Some(pid) = self.stream_store.get_pid(task_id) {
            // Send SIGTERM to gracefully stop the process
            let result = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();

            match result {
                Ok(status) if status.success() => {
                    info!("Sent SIGTERM to PID {} for task {}", pid, task_id);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id.to_string(),
                            success: false,
                        })
                        .await;
                }
                Ok(_) | Err(_) => {
                    warn!("Failed to kill PID {} for task {}", pid, task_id);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::Error(format!(
                            "Failed to stop session for task {}",
                            task_id
                        )))
                        .await;
                }
            }
        } else {
            warn!("No PID found for task {}", task_id);
            let _ = self
                .event_tx
                .send(ScudEvent::Error(format!(
                    "No running process found for task {}",
                    task_id
                )))
                .await;
        }
    }

    /// Run swarm execution as a background task.
    ///
    /// Spawns headless runners per-task so each task gets its own StreamStore
    /// session visible in the Monitor view. Checks cancellation flag between waves.
    #[allow(clippy::too_many_arguments)]
    async fn run_swarm_bg(
        tag: &str,
        harness_name: &str,
        round_size: usize,
        model: Option<String>,
        event_tx: mpsc::Sender<ScudEvent>,
        stream_store: StreamStore,
        working_dir: PathBuf,
        cancelled: Arc<AtomicBool>,
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
                let _ = event_tx
                    .send(ScudEvent::Error(format!("Invalid harness: {}", e)))
                    .await;
                return;
            }
        };

        // Load tasks and compute waves
        let tag_owned = tag.to_string();
        let storage_dir = working_dir.clone();
        let wave_result =
            tokio::task::spawn_blocking(move || -> Result<(Vec<Vec<Task>>, Phase), String> {
                let storage = Storage::new(Some(storage_dir));
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
                let _ = event_tx
                    .send(ScudEvent::Error(format!("Failed to compute waves: {}", e)))
                    .await;
                return;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = event_tx
                    .send(ScudEvent::Error(format!("Task spawn error: {}", e)))
                    .await;
                return;
            }
        };

        let total_waves = waves.len();
        let _ = event_tx
            .send(ScudEvent::SwarmStarted {
                tag: tag.to_string(),
                total_waves,
            })
            .await;

        let mut all_success = true;

        'waves: for (wave_idx, wave_tasks) in waves.into_iter().enumerate() {
            // Check cancellation before each wave
            if cancelled.load(Ordering::SeqCst) {
                info!("Swarm cancelled before wave {}", wave_idx);
                break;
            }

            let task_ids: Vec<String> = wave_tasks.iter().map(|t| t.id.clone()).collect();
            let _ = event_tx
                .send(ScudEvent::WaveStarted {
                    wave: wave_idx,
                    tasks: task_ids.clone(),
                })
                .await;

            // Process tasks in chunks of round_size
            for chunk in wave_tasks.chunks(round_size) {
                // Check cancellation before each chunk
                if cancelled.load(Ordering::SeqCst) {
                    info!("Swarm cancelled during wave {}", wave_idx);
                    break 'waves;
                }

                let mut handles = Vec::new();

                for task in chunk {
                    let task_id = task.id.clone();
                    let task_title = task.title.clone();
                    let task_description = task.description.clone();
                    let harness_copy = harness;
                    let event_tx = event_tx.clone();
                    let working_dir = working_dir.clone();
                    let stream_store = stream_store.clone();
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

            let _ = event_tx
                .send(ScudEvent::WaveCompleted { wave: wave_idx })
                .await;
        }

        let _ = event_tx
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
    async fn run_task(&mut self, task_id: &str, harness_name: &str, model: Option<String>) {
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
        let storage_dir = self.working_dir.clone();
        let task_result = tokio::task::spawn_blocking(move || -> Result<(Task, String), String> {
            let storage = Storage::new(Some(storage_dir));
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
        let mut stream_terminal = false;
        let mut stream_success = false;

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
                    stream_terminal = true;
                    stream_success = success;
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
                    stream_terminal = true;
                    stream_success = false;
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
        let process_success = match session.wait().await {
            Ok(success) => {
                info!(
                    "Headless session for task {} finished with success={}",
                    task_id, success
                );
                success
            }
            Err(e) => {
                warn!("Error waiting for headless session to complete: {}", e);
                false
            }
        };

        let final_success =
            reconcile_task_success(process_success, stream_terminal, stream_success);

        // If stream never reported completion, or process contradicted a stream success,
        // emit a final completion update based on the actual process result.
        if !stream_terminal || final_success != stream_success {
            if !process_success && stream_success {
                let _ = event_tx
                    .send(ScudEvent::Error(format!(
                        "Task {} exited with non-zero status",
                        task_id_owned
                    )))
                    .await;
            }
            let _ = event_tx
                .send(ScudEvent::TaskCompleted {
                    task_id: task_id_owned.clone(),
                    success: final_success,
                })
                .await;
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
        let mut session: SessionHandle =
            match runner.start(task_id, prompt, working_dir, model).await {
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
        let mut stream_terminal = false;
        let mut stream_success = false;
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
                    stream_terminal = true;
                    stream_success = success;
                    let _ = event_tx
                        .send(ScudEvent::TaskCompleted {
                            task_id: task_id.to_string(),
                            success,
                        })
                        .await;
                    break;
                }
                StreamEventKind::Error { message } => {
                    stream_terminal = true;
                    stream_success = false;
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
        let process_success = match session.wait().await {
            Ok(success) => success,
            Err(e) => {
                warn!("Error waiting for task {} session: {}", task_id, e);
                false
            }
        };
        let final_success =
            reconcile_task_success(process_success, stream_terminal, stream_success);

        if !stream_terminal || final_success != stream_success {
            if !process_success && stream_success {
                let _ = event_tx
                    .send(ScudEvent::Error(format!(
                        "Task {} exited with non-zero status",
                        task_id
                    )))
                    .await;
            }
            let _ = event_tx
                .send(ScudEvent::TaskCompleted {
                    task_id: task_id.to_string(),
                    success: final_success,
                })
                .await;
        }

        final_success
    }

    /// Scan directories for PRD markdown files
    async fn scan_prd_files(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Vec<PathBuf> {
            let excluded = [
                "CLAUDE.md",
                "AGENTS.md",
                "README.md",
                "CHANGELOG.md",
                "IMPLEMENTATION_PLAN.md",
            ];

            let mut scan_dirs = vec![
                working_dir.join(".scud/docs/prd"),
                working_dir.join("thoughts/shared/prd"),
                working_dir.join("docs"),
                working_dir.join("prds"),
                working_dir.clone(),
            ];

            // Load extra paths from .scud/config.toml
            let config_path = working_dir.join(".scud/config.toml");
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(table) = content.parse::<toml::Table>() {
                    if let Some(gen) = table.get("generate").and_then(|v| v.as_table()) {
                        if let Some(paths) = gen.get("prd_paths").and_then(|v| v.as_array()) {
                            for p in paths {
                                if let Some(s) = p.as_str() {
                                    let path = Path::new(s);
                                    let abs = if path.is_absolute() {
                                        path.to_path_buf()
                                    } else {
                                        working_dir.join(path)
                                    };
                                    scan_dirs.push(abs);
                                }
                            }
                        }
                    }
                }
            }

            let mut files = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for dir in &scan_dirs {
                if !dir.is_dir() {
                    continue;
                }
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md")
                        {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                if excluded.contains(&name) {
                                    continue;
                                }
                            }
                            if seen.insert(path.clone()) {
                                files.push(path);
                            }
                        }
                    }
                }
            }

            files.sort();
            files
        })
        .await;

        match result {
            Ok(files) => {
                let _ = self.event_tx.send(ScudEvent::PrdFilesFound(files)).await;
            }
            Err(e) => {
                error!("Failed to scan PRD files: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to scan PRD files: {}", e)))
                    .await;
            }
        }
    }

    /// Run generate pipeline via subprocess with streaming output
    async fn run_generate(
        &self,
        prd_file: &Path,
        tag: &str,
        num_tasks: u32,
        no_expand: bool,
        no_check_deps: bool,
        append: bool,
    ) {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut args = vec![
            "generate".to_string(),
            prd_file.display().to_string(),
            "--tag".to_string(),
            tag.to_string(),
            "-n".to_string(),
            num_tasks.to_string(),
        ];
        if no_expand {
            args.push("--no-expand".to_string());
        }
        if no_check_deps {
            args.push("--no-check-deps".to_string());
        }
        if append {
            args.push("--append".to_string());
        }

        let tag_str = tag.to_string();
        info!("Running generate: scud {}", args.join(" "));

        let _ = self
            .event_tx
            .send(ScudEvent::GenerateStatus(
                "Starting generate pipeline...".to_string(),
            ))
            .await;

        let child_result = tokio::process::Command::new("scud")
            .args(&args)
            .current_dir(&self.working_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut child = match child_result {
            Ok(child) => child,
            Err(e) => {
                let err = format!("Failed to run scud generate: {}", e);
                error!("{}", err);
                let _ = self
                    .event_tx
                    .send(ScudEvent::GenerateCompleted(Err(err)))
                    .await;
                return;
            }
        };

        // Stream stdout lines
        if let Some(stdout) = child.stdout.take() {
            let event_tx = self.event_tx.clone();
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = event_tx
                    .send(ScudEvent::GenerateOutputLine(line.clone()))
                    .await;
                let _ = event_tx.send(ScudEvent::GenerateStatus(line)).await;
            }
        }

        // Read any remaining stderr
        let mut stderr_text = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = stderr.read_to_string(&mut stderr_text).await;
        }

        // Wait for the process to exit
        match child.wait().await {
            Ok(status) => {
                if status.success() {
                    info!("Generate completed for tag '{}'", tag_str);
                    let _ = self
                        .event_tx
                        .send(ScudEvent::GenerateCompleted(Ok(())))
                        .await;
                    // Reload tags after generating
                    self.load_available_tags().await;
                    self.load_tag_summaries().await;
                } else {
                    let err = format!("Generate failed: {}", stderr_text);
                    error!("{}", err);
                    if !stderr_text.is_empty() {
                        let _ = self
                            .event_tx
                            .send(ScudEvent::GenerateOutputLine(stderr_text.clone()))
                            .await;
                    }
                    let _ = self
                        .event_tx
                        .send(ScudEvent::GenerateCompleted(Err(err)))
                        .await;
                }
            }
            Err(e) => {
                let err = format!("Failed to wait for scud generate: {}", e);
                error!("{}", err);
                let _ = self
                    .event_tx
                    .send(ScudEvent::GenerateCompleted(Err(err)))
                    .await;
            }
        }
    }

    /// Load tag summaries for the tag explorer
    async fn load_tag_summaries(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(
            move || -> Result<Vec<crate::state::TagSummary>, String> {
                let cli_storage = CliStorage::new(Some(working_dir));
                let phases = cli_storage.load_tasks().map_err(|e| e.to_string())?;
                let active_tag = cli_storage.get_active_group().ok().flatten();

                let mut summaries: Vec<crate::state::TagSummary> = phases
                    .iter()
                    .map(|(name, phase)| {
                        let mut done = 0;
                        let mut pending = 0;
                        let mut in_progress = 0;
                        let mut failed = 0;

                        for task in &phase.tasks {
                            let status_str = task.status.as_str();
                            match status_str {
                                "done" => done += 1,
                                "pending" => pending += 1,
                                "in-progress" => in_progress += 1,
                                "failed" => failed += 1,
                                "blocked" => pending += 1,
                                _ => pending += 1,
                            }
                        }

                        crate::state::TagSummary {
                            name: name.clone(),
                            total_tasks: phase.tasks.len(),
                            done_count: done,
                            pending_count: pending,
                            in_progress_count: in_progress,
                            failed_count: failed,
                            is_active: active_tag.as_deref() == Some(name.as_str()),
                        }
                    })
                    .collect();

                summaries.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(summaries)
            },
        )
        .await;

        match result {
            Ok(Ok(summaries)) => {
                let _ = self
                    .event_tx
                    .send(ScudEvent::TagSummariesLoaded(summaries))
                    .await;
            }
            Ok(Err(e)) => {
                error!("Failed to load tag summaries: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!(
                        "Failed to load tag summaries: {}",
                        e
                    )))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
            }
        }
    }

    /// Load archives for the tag explorer
    async fn load_archives(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(
            move || -> Result<Vec<crate::state::ArchiveEntry>, String> {
                let cli_storage = CliStorage::new(Some(working_dir));
                let archives = cli_storage.list_archives().map_err(|e| e.to_string())?;

                Ok(archives
                    .into_iter()
                    .map(|a| crate::state::ArchiveEntry {
                        filename: a.filename,
                        date: a.date,
                        tag: a.tag,
                        task_count: a.task_count,
                    })
                    .collect())
            },
        )
        .await;

        match result {
            Ok(Ok(entries)) => {
                let _ = self.event_tx.send(ScudEvent::ArchivesLoaded(entries)).await;
            }
            Ok(Err(e)) => {
                error!("Failed to load archives: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to load archives: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
            }
        }
    }

    /// Set the active tag
    async fn set_active_tag(&self, tag: &str) {
        let tag = tag.to_string();
        let tag_for_event = tag.clone();
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let cli_storage = CliStorage::new(Some(working_dir));
            cli_storage
                .set_active_group(&tag)
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Active tag set to '{}'", tag_for_event);
                let _ = self
                    .event_tx
                    .send(ScudEvent::ActiveTagChanged(tag_for_event))
                    .await;
                self.load_tag_summaries().await;
                self.load_available_tags().await;
            }
            Ok(Err(e)) => {
                error!("Failed to set active tag: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to set active tag: {}", e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
            }
        }
    }

    /// Restore an archive
    async fn restore_archive(&self, filename: &str) {
        let filename = filename.to_string();
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
            let cli_storage = CliStorage::new(Some(working_dir));
            cli_storage
                .restore_archive(&filename, false)
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(tags)) => {
                info!("Archive restored: {:?}", tags);
                let _ = self
                    .event_tx
                    .send(ScudEvent::ArchiveRestored(Ok(tags)))
                    .await;
                self.load_tag_summaries().await;
                self.load_archives().await;
                self.load_available_tags().await;
            }
            Ok(Err(e)) => {
                error!("Failed to restore archive: {}", e);
                let _ = self.event_tx.send(ScudEvent::ArchiveRestored(Err(e))).await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::ArchiveRestored(Err(e.to_string())))
                    .await;
            }
        }
    }

    /// Load backpressure configuration from .scud/config.toml or auto-detect
    async fn load_backpressure_config(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || {
            // Check if config.toml has explicit backpressure section
            let config_path = working_dir.join(".scud").join("config.toml");
            let mut is_auto_detected = true;

            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(config) = content.parse::<toml::Value>() {
                        if let Some(swarm) = config.get("swarm") {
                            if swarm.get("backpressure").is_some() {
                                is_auto_detected = false;
                            }
                        }
                    }
                }
            }

            let bp_config = scud::backpressure::BackpressureConfig::load(Some(&working_dir))
                .unwrap_or_default();

            (bp_config, is_auto_detected)
        })
        .await;

        match result {
            Ok((config, is_auto_detected)) => {
                let _ = self
                    .event_tx
                    .send(ScudEvent::BackpressureConfigLoaded {
                        commands: config.commands,
                        stop_on_failure: config.stop_on_failure,
                        timeout_secs: config.timeout_secs,
                        is_auto_detected,
                    })
                    .await;
            }
            Err(e) => {
                error!("Failed to load backpressure config: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!(
                        "Failed to load backpressure config: {}",
                        e
                    )))
                    .await;
            }
        }
    }

    /// Save backpressure configuration to .scud/config.toml
    async fn save_backpressure_config(
        &self,
        commands: Vec<String>,
        stop_on_failure: bool,
        timeout_secs: u64,
    ) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let config_path = working_dir.join(".scud").join("config.toml");

            // Ensure .scud directory exists
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Read existing config or start fresh
            let mut doc = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
                content
                    .parse::<toml_edit::DocumentMut>()
                    .map_err(|e| e.to_string())?
            } else {
                toml_edit::DocumentMut::new()
            };

            // Ensure [swarm] table exists
            if !doc.contains_key("swarm") {
                doc["swarm"] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            let swarm = doc["swarm"].as_table_mut().ok_or("swarm is not a table")?;

            // Set [swarm.backpressure] section
            let mut bp_table = toml_edit::Table::new();
            let mut cmd_array = toml_edit::Array::new();
            for cmd in &commands {
                cmd_array.push(cmd.as_str());
            }
            bp_table["commands"] = toml_edit::value(cmd_array);
            bp_table["stop_on_failure"] = toml_edit::value(stop_on_failure);
            bp_table["timeout_secs"] = toml_edit::value(timeout_secs as i64);

            swarm["backpressure"] = toml_edit::Item::Table(bp_table);

            std::fs::write(&config_path, doc.to_string()).map_err(|e| e.to_string())?;

            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Backpressure config saved");
                let _ = self
                    .event_tx
                    .send(ScudEvent::BackpressureConfigSaved(Ok(())))
                    .await;
            }
            Ok(Err(e)) => {
                error!("Failed to save backpressure config: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::BackpressureConfigSaved(Err(e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::BackpressureConfigSaved(Err(e.to_string())))
                    .await;
            }
        }
    }

    /// Initialize the project by creating .scud/ directory structure
    async fn init_project(&self) {
        info!("Initializing project");
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let storage = CliStorage::new(Some(working_dir));
            storage.initialize().map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Project initialized successfully");
                let _ = self
                    .event_tx
                    .send(ScudEvent::ProjectInitialized(Ok(())))
                    .await;
                // Reload tasks and tags
                self.load_tasks(None).await;
                self.load_available_tags().await;
            }
            Ok(Err(e)) => {
                error!("Failed to initialize project: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::ProjectInitialized(Err(e)))
                    .await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::ProjectInitialized(Err(e.to_string())))
                    .await;
            }
        }
    }

    /// Load LLM configuration from .scud/config.toml
    async fn load_llm_config(&self) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || {
            let config_path = working_dir.join(".scud").join("config.toml");
            if config_path.exists() {
                scud::config::Config::load(&config_path).ok()
            } else {
                None
            }
        })
        .await;

        match result {
            Ok(Some(config)) => {
                let _ = self
                    .event_tx
                    .send(ScudEvent::LlmConfigLoaded {
                        provider: config.llm.provider,
                        model: config.llm.model,
                        smart_provider: config.llm.smart_provider,
                        smart_model: config.llm.smart_model,
                        fast_provider: config.llm.fast_provider,
                        fast_model: config.llm.fast_model,
                        max_tokens: config.llm.max_tokens.to_string(),
                    })
                    .await;
            }
            Ok(None) => {
                // No config file, send defaults
                let config = scud::config::Config::default();
                let _ = self
                    .event_tx
                    .send(ScudEvent::LlmConfigLoaded {
                        provider: config.llm.provider,
                        model: config.llm.model,
                        smart_provider: config.llm.smart_provider,
                        smart_model: config.llm.smart_model,
                        fast_provider: config.llm.fast_provider,
                        fast_model: config.llm.fast_model,
                        max_tokens: config.llm.max_tokens.to_string(),
                    })
                    .await;
            }
            Err(e) => {
                error!("Failed to load LLM config: {}", e);
            }
        }
    }

    /// Save LLM configuration to .scud/config.toml using toml_edit to preserve other sections
    #[allow(clippy::too_many_arguments)]
    async fn save_llm_config(
        &self,
        provider: String,
        model: String,
        smart_provider: String,
        smart_model: String,
        fast_provider: String,
        fast_model: String,
        max_tokens: String,
    ) {
        let working_dir = self.working_dir.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let config_path = working_dir.join(".scud").join("config.toml");

            // Ensure .scud directory exists
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Read existing config or start fresh
            let mut doc = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
                content
                    .parse::<toml_edit::DocumentMut>()
                    .map_err(|e| e.to_string())?
            } else {
                toml_edit::DocumentMut::new()
            };

            // Ensure [llm] table exists
            if !doc.contains_key("llm") {
                doc["llm"] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            let llm = doc["llm"].as_table_mut().ok_or("llm is not a table")?;

            llm["provider"] = toml_edit::value(&provider);
            llm["model"] = toml_edit::value(&model);
            llm["smart_provider"] = toml_edit::value(&smart_provider);
            llm["smart_model"] = toml_edit::value(&smart_model);
            llm["fast_provider"] = toml_edit::value(&fast_provider);
            llm["fast_model"] = toml_edit::value(&fast_model);

            if let Ok(tokens) = max_tokens.parse::<i64>() {
                llm["max_tokens"] = toml_edit::value(tokens);
            }

            std::fs::write(&config_path, doc.to_string()).map_err(|e| e.to_string())?;

            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("LLM config saved");
                let _ = self.event_tx.send(ScudEvent::LlmConfigSaved(Ok(()))).await;
            }
            Ok(Err(e)) => {
                error!("Failed to save LLM config: {}", e);
                let _ = self.event_tx.send(ScudEvent::LlmConfigSaved(Err(e))).await;
            }
            Err(e) => {
                error!("Task spawn error: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::LlmConfigSaved(Err(e.to_string())))
                    .await;
            }
        }
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
                    warn!(
                        "Failed to load backpressure config: {}, validation disabled",
                        e
                    );
                    None
                }
                Err(e) => {
                    warn!(
                        "Spawn error loading backpressure config: {}, validation disabled",
                        e
                    );
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
            let storage_dir = working_dir.clone();
            let next_task = tokio::task::spawn_blocking(move || -> Option<Task> {
                let storage = Storage::new(Some(storage_dir));
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
                let storage_dir = working_dir.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(Some(storage_dir));
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
                let storage_dir = working_dir.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(Some(storage_dir));
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
                        let _ = self
                            .event_tx
                            .send(ScudEvent::TaskOutput {
                                task_id: task_id.clone(),
                                text: format!("\n--- VALIDATION ---\n{}\n", output),
                            })
                            .await;

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

                                let _ = self
                                    .event_tx
                                    .send(ScudEvent::TaskOutput {
                                        task_id: task_id.clone(),
                                        text: format!("\n--- REPAIR ATTEMPT {} ---\n", attempt),
                                    })
                                    .await;

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
                                        let _ = self
                                            .event_tx
                                            .send(ScudEvent::TaskOutput {
                                                task_id: task_id.clone(),
                                                text: format!("\n--- VALIDATION ---\n{}\n", out),
                                            })
                                            .await;

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
                let storage_dir = working_dir.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(Some(storage_dir));
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
                let storage_dir = working_dir.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let storage = Storage::new(Some(storage_dir));
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
    fn test_reconcile_task_success_uses_process_exit_as_source_of_truth() {
        assert!(reconcile_task_success(true, false, false));
        assert!(reconcile_task_success(true, true, true));
        assert!(!reconcile_task_success(false, true, true));
        assert!(!reconcile_task_success(false, false, false));
    }

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
