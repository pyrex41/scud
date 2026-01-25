//! SCUD Bridge - Async communication layer between Iced GUI and SCUD Core
//!
//! Provides event-driven communication with SCUD through direct library calls
//! for task operations and subprocess calls for swarm execution.

use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use scud_core::{compute_waves, Phase, Storage, Task, TaskStatus};

use crate::state::TaskInfo;

/// Events emitted by SCUD execution
///
/// These events are sent from the ScudBridge to the GUI to update state.
#[derive(Debug, Clone)]
pub enum ScudEvent {
    /// Tasks loaded from SCUD storage
    TasksLoaded(Vec<TaskInfo>),

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

    /// Start swarm execution
    StartSwarm {
        tag: String,
        harness: String,
        round_size: usize,
    },

    /// Run a single task with an agent
    RunTask {
        task_id: String,
        harness: String,
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
}

impl From<ScudJsonTask> for TaskInfo {
    fn from(task: ScudJsonTask) -> Self {
        TaskInfo {
            id: task.id,
            title: task.title,
            status: task.status,
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
}

impl ScudBridge {
    /// Create a new ScudBridge with the given channel endpoints
    pub fn new(
        event_tx: mpsc::Sender<ScudEvent>,
        command_rx: mpsc::Receiver<ScudCommand>,
    ) -> Self {
        Self {
            event_tx,
            command_rx,
            swarm_handle: None,
            paused: false,
        }
    }

    /// Create a new ScudBridge and return the channel handles for the GUI
    ///
    /// Returns (bridge, command_sender, event_receiver)
    pub fn create() -> (
        Self,
        mpsc::Sender<ScudCommand>,
        mpsc::Receiver<ScudEvent>,
    ) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (command_tx, command_rx) = mpsc::channel(100);

        let bridge = Self::new(event_tx, command_rx);
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
                ScudCommand::StartSwarm {
                    tag,
                    harness,
                    round_size,
                } => {
                    self.run_swarm(&tag, &harness, round_size).await;
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
                ScudCommand::RunTask { task_id, harness } => {
                    self.run_single_task(&task_id, &harness).await;
                }
                ScudCommand::CompleteTask { task_id } => {
                    self.complete_task(&task_id).await;
                }
                ScudCommand::BlockTask { task_id } => {
                    self.block_task(&task_id).await;
                }
            }
        }

        info!("ScudBridge shutting down");
    }

    /// Load tasks from SCUD storage using direct library calls
    ///
    /// Uses Storage to load the active Phase and converts tasks to TaskInfo
    async fn load_tasks(&self, tag: Option<String>) {
        debug!("Loading tasks via scud-core (tag: {:?})", tag);

        // Run blocking storage operations in a spawn_blocking task
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<TaskInfo>, String> {
            let storage = Storage::new(None);

            let phase = if let Some(tag) = tag {
                storage.load_group(&tag).map_err(|e| e.to_string())?
            } else {
                storage.load_active_group().map_err(|e| e.to_string())?
            };

            Ok(Self::phase_to_task_infos(&phase))
        })
        .await;

        match result {
            Ok(Ok(task_infos)) => {
                let _ = self.event_tx.send(ScudEvent::TasksLoaded(task_infos)).await;
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
            })
            .collect()
    }

    /// Convert a scud_core Task to TaskInfo
    fn task_to_task_info(task: &Task) -> TaskInfo {
        TaskInfo {
            id: task.id.clone(),
            title: task.title.clone(),
            status: task.status.as_str().to_string(),
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
                        .filter_map(|task_id| {
                            phase
                                .get_task(&task_id)
                                .map(Self::task_to_task_info)
                        })
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

    /// Run swarm execution with event streaming
    ///
    /// Spawns `scud swarm --tag <tag> --harness <harness> --json-events`
    /// and streams events as they occur
    async fn run_swarm(&mut self, tag: &str, harness: &str, round_size: usize) {
        let round_size_str = round_size.to_string();
        let args = vec![
            "swarm",
            "--tag",
            tag,
            "--harness",
            harness,
            "--round-size",
            &round_size_str,
            "--json-events",
        ];

        info!("Starting swarm: scud {}", args.join(" "));

        match Command::new("scud")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                // Take stdout for event streaming
                if let Some(stdout) = child.stdout.take() {
                    let event_tx = self.event_tx.clone();
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();

                    // Stream events from stdout
                    while let Ok(Some(line)) = lines.next_line().await {
                        // Try to parse as JSON event
                        if let Ok(event) = serde_json::from_str::<ScudJsonEvent>(&line) {
                            let scud_event: ScudEvent = event.into();
                            if event_tx.send(scud_event).await.is_err() {
                                warn!("Event channel closed");
                                break;
                            }
                        } else {
                            // Non-JSON line - send as generic output
                            if !line.trim().is_empty() {
                                let _ = event_tx.send(ScudEvent::Output(line)).await;
                            }
                        }
                    }
                }

                // Wait for process to complete
                match child.wait().await {
                    Ok(status) => {
                        if status.success() {
                            info!("Swarm completed successfully");
                        } else {
                            warn!("Swarm exited with status: {}", status);
                        }
                    }
                    Err(e) => {
                        error!("Error waiting for swarm process: {}", e);
                        let _ = self
                            .event_tx
                            .send(ScudEvent::Error(format!("Swarm process error: {}", e)))
                            .await;
                    }
                }
            }
            Err(e) => {
                error!("Failed to spawn swarm: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to start swarm: {}", e)))
                    .await;
            }
        }
    }

    /// Run a single task with an agent
    ///
    /// Uses `scud run` to execute the task with the specified harness
    async fn run_single_task(&mut self, task_id: &str, harness: &str) {
        info!("Running single task {} with harness {}", task_id, harness);

        // First, load the task details to get the prompt
        let task_id_clone = task_id.to_string();
        let task_result = tokio::task::spawn_blocking(move || -> Result<Task, String> {
            let storage = Storage::new(None);
            let tag = storage
                .get_active_group()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "No active task group".to_string())?;
            let phase = storage.load_group(&tag).map_err(|e| e.to_string())?;
            phase
                .get_task(&task_id_clone)
                .cloned()
                .ok_or_else(|| format!("Task '{}' not found", task_id_clone))
        })
        .await;

        let task = match task_result {
            Ok(Ok(t)) => t,
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

        // Construct the prompt for the agent
        let prompt = format!(
            "Complete the following task:\n\n## Task: {}\n\n{}\n\nImplement this task and commit your changes.",
            task.title, task.description
        );

        // Emit task started event
        let _ = self
            .event_tx
            .send(ScudEvent::TaskStarted {
                task_id: task_id.to_string(),
            })
            .await;

        // Run the agent using scud run
        let args = vec!["run", "-H", harness, &prompt];
        info!("Running: scud {}", args.join(" "));

        match Command::new("scud")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                // Stream stdout
                if let Some(stdout) = child.stdout.take() {
                    let event_tx = self.event_tx.clone();
                    let task_id_for_output = task_id.to_string();
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = event_tx
                            .send(ScudEvent::TaskOutput {
                                task_id: task_id_for_output.clone(),
                                text: line,
                            })
                            .await;
                    }
                }

                // Wait for completion
                match child.wait().await {
                    Ok(status) => {
                        let success = status.success();
                        let _ = self
                            .event_tx
                            .send(ScudEvent::TaskCompleted {
                                task_id: task_id.to_string(),
                                success,
                            })
                            .await;

                        if success {
                            info!("Task {} completed successfully", task_id);
                        } else {
                            warn!("Task {} failed with status: {}", task_id, status);
                        }
                    }
                    Err(e) => {
                        error!("Error waiting for task process: {}", e);
                        let _ = self
                            .event_tx
                            .send(ScudEvent::Error(format!("Task process error: {}", e)))
                            .await;
                    }
                }
            }
            Err(e) => {
                error!("Failed to spawn task agent: {}", e);
                let _ = self
                    .event_tx
                    .send(ScudEvent::Error(format!("Failed to start task: {}", e)))
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
        };

        let task_info: TaskInfo = json_task.into();
        assert_eq!(task_info.id, "1");
        assert_eq!(task_info.title, "Test task");
        assert_eq!(task_info.status, "Pending");
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
}
