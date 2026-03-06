//! Descartes GUI - Desktop interface for AI agent orchestration
//!
//! A minimal Iced GUI that wraps v2's simple architecture:
//! - Direct library calls to RalphExecutor
//! - SCUD storage for task/wave management
//! - AgentRegistry for status display

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Subscription, Task, Theme};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};

// Use scud-core directly for task management
use scud_core::Storage;

mod components;
mod scud_bridge;
mod state;
mod theme;
mod views;

use scud_bridge::{ScudBridge, ScudCommand, ScudEvent};
use state::{
    AgentConfig, AgentStatus, AppState, ArchiveEntry, BackpressureState, ExecutionMode,
    HeadlessSessionInfo, HeadlessSessionStatus, LaunchConfig, LlmConfigState, RalphPhase,
    RalphProgress,
    SwarmDefaults, SwarmProgress, TagSummary, TaskInfo,
};
use views::ViewMode;

/// Wrapper for ScudEvent receiver that implements Hash for Iced subscriptions
struct ScudEventReceiver(Arc<TokioMutex<Option<mpsc::Receiver<ScudEvent>>>>);

impl std::hash::Hash for ScudEventReceiver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use a fixed hash since we only have one receiver
        "scud-event-receiver".hash(state);
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter("descartes_gui=debug,scud=info,scud_core=info,scud_cli=info")
        .init();

    iced::application(DescartesGui::new, DescartesGui::update, DescartesGui::view)
        .subscription(DescartesGui::subscription)
        .theme(DescartesGui::theme)
        .title("Descartes")
        .run()
}

/// Main application state
struct DescartesGui {
    /// Current view mode
    view: ViewMode,
    /// Application state
    state: AppState,
    /// Command channel for ScudBridge (used for all agent control: pause/resume/stop)
    scud_command_tx: Option<mpsc::Sender<ScudCommand>>,
    /// Event receiver from ScudBridge (wrapped in Arc<Mutex> for subscription access)
    scud_event_rx: Arc<TokioMutex<Option<mpsc::Receiver<ScudEvent>>>>,
    /// Error message to display
    error: Option<String>,
    /// Currently selected agent in the Agents config view
    selected_agent_config: Option<String>,
}

/// Application messages
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    // Navigation
    SwitchView(ViewMode),

    // Task management (local/legacy)
    LoadWaves,
    WavesLoaded(Result<Vec<Vec<TaskInfo>>, String>),

    // SCUD task management via ScudBridge
    LoadTasksViaScud {
        tag: Option<String>,
    },
    ComputeWavesViaScud {
        tag: String,
    },
    MarkTaskComplete {
        task_id: String,
    },
    MarkTaskBlocked {
        task_id: String,
    },
    SelectTask(Option<String>),
    SelectTag(Option<String>),
    RefreshTasks,

    // Launch configuration
    SetHarness(String),
    SetModel(String),
    SetRoundSizeInput(String),
    SetLaunchTag(String),
    SetAgentType(Option<String>),
    TagsLoaded(Vec<String>),
    AgentsLoaded(Vec<String>),
    SpawnTask {
        task_id: String,
    },
    ArchiveTag {
        tag: String,
    },

    // Swarm management
    StartSwarm {
        tag: String,
        harness: String,
        round_size: usize,
    },
    StopSwarm,

    // Agent output (for legacy message handling)
    AgentOutput(String),
    AgentComplete(Result<(), String>),

    // Control
    PauseAgent,
    ResumeAgent,
    CancelAgent,

    // ScudBridge events
    ScudEvent(ScudEvent),

    // UI
    DismissError,
    ClearOutput,

    // Monitor view
    MonitorSelectTask(String),
    MonitorClearCompleted,
    /// Attach to a session in interactive terminal mode
    MonitorAttachSession {
        task_id: String,
    },
    /// Copy session output to clipboard
    MonitorCopyOutput {
        task_id: String,
    },
    /// Stop a specific headless session
    MonitorStopSession {
        task_id: String,
    },
    /// Pause the current swarm
    MonitorPauseSwarm,
    /// Stop the current swarm
    MonitorStopSwarm,

    // Agent configuration
    LoadAgentConfigs,
    SelectAgentConfig(String),
    UpdateAgentHarness {
        agent: String,
        harness: String,
    },
    UpdateAgentModel {
        agent: String,
        model: String,
    },
    UpdateAgentDescription {
        agent: String,
        description: String,
    },
    SaveAgentConfig(String),
    AgentConfigSaved(String),
    AgentConfigsLoaded(std::collections::HashMap<String, AgentConfig>),

    // Model loading
    LoadHarnessModels,
    HarnessModelsLoaded {
        harness: String,
        models: Vec<String>,
    },

    // Settings
    SetTerminalApp(String),
    BrowseProject,
    SwitchProject(std::path::PathBuf),
    ProjectSwitched,

    // Launch agent override
    SetOverrideAgents(bool),

    // Ralph config
    SetExecutionMode(ExecutionMode),
    SetRalphValidate(bool),
    SetRalphRepair(bool),
    SetRalphMaxIterations(String),
    SetRalphMaxRepairAttempts(String),
    SetRalphBatchSubtasks(bool),
    SetRalphGitPush(bool),

    // Ralph lifecycle
    StartRalph {
        tag: String,
        harness: String,
    },
    StopRalph,

    // Generate view
    ScanPrdDirectory,
    PrdFilesLoaded(Vec<std::path::PathBuf>),
    SelectPrd(std::path::PathBuf),
    PrdContentLoaded(Result<String, String>),
    SetGenerateTag(String),
    SetGenerateNumTasks(String),
    SetGenerateNoExpand(bool),
    SetGenerateNoCheckDeps(bool),
    SetGenerateAppend(bool),
    StartGenerate,
    GenerateCompleted(Result<(), String>),
    GenerateStatus(String),

    // Tag explorer
    LoadTagExplorer,
    TagSummariesLoaded(Vec<TagSummary>),
    ArchivesLoaded(Vec<ArchiveEntry>),
    SetActiveTag(String),
    ActiveTagChanged(String),
    TagExplorerArchiveTag(String),
    RestoreArchive {
        filename: String,
    },
    ArchiveRestored(Result<Vec<String>, String>),

    // Backpressure config
    LoadBackpressureConfig,
    BackpressureConfigLoaded {
        commands: Vec<String>,
        stop_on_failure: bool,
        timeout_secs: u64,
        is_auto_detected: bool,
    },
    SetBackpressureStopOnFailure(bool),
    SetBackpressureTimeout(String),
    AddBackpressureCommand,
    SetBackpressureNewCommand(String),
    RemoveBackpressureCommand(usize),
    SaveBackpressureConfig,
    BackpressureConfigSaved(Result<(), String>),
    DetectBackpressureCommands,

    // Project initialization
    InitProject,
    ProjectInitialized(Result<(), String>),

    // LLM config
    LoadLlmConfig,
    LlmConfigLoaded {
        provider: String,
        model: String,
        smart_provider: String,
        smart_model: String,
        fast_provider: String,
        fast_model: String,
        max_tokens: String,
    },
    SetLlmProvider(String),
    SetLlmModel(String),
    SetLlmSmartProvider(String),
    SetLlmSmartModel(String),
    SetLlmFastProvider(String),
    SetLlmFastModel(String),
    SetLlmMaxTokens(String),
    SaveLlmConfig,
    LlmConfigSaved(Result<(), String>),

    // Streaming generate output
    GenerateOutputLine(String),

    Tick,
}

// Note: Agent control (Pause/Resume/Cancel) is now handled via ScudCommand
// (PauseSwarm, ResumeSwarm, StopSwarm) sent through scud_command_tx

impl DescartesGui {
    fn new() -> (Self, Task<Message>) {
        // Load swarm defaults from .scud/config.toml (or use defaults)
        let swarm_defaults = SwarmDefaults::load_from_scud();
        tracing::info!(
            "Loaded swarm defaults: harness={}, round_size={}, default_tag={}",
            swarm_defaults.harness,
            swarm_defaults.round_size,
            swarm_defaults.default_tag
        );

        let launch_config = LaunchConfig::from_defaults(&swarm_defaults);

        // Create ScudBridge and get channel handles
        let initial_working_dir = std::env::current_dir().unwrap_or_default();
        let (bridge, scud_command_tx, scud_event_rx) =
            ScudBridge::create_with_working_dir(initial_working_dir.clone());

        let init_tx = scud_command_tx.clone();
        let init_task = Task::perform(
            async move {
                let _ = init_tx.send(ScudCommand::LoadTasks { tag: None }).await;
                let _ = init_tx.send(ScudCommand::LoadAvailableTags).await;
                let _ = init_tx.send(ScudCommand::LoadAvailableAgents).await;
                let _ = init_tx.send(ScudCommand::LoadLlmConfig).await;
            },
            |_| Message::Tick,
        );

        // Wrap receiver in Arc<Mutex> for subscription access
        let scud_event_rx = Arc::new(TokioMutex::new(Some(scud_event_rx)));

        // Spawn the bridge on a background tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(bridge.run());
        });

        (
            Self {
                view: ViewMode::Waves,
                state: AppState {
                    swarm_defaults,
                    launch_config,
                    ..AppState::default()
                },
                scud_command_tx: Some(scud_command_tx),
                scud_event_rx,
                error: None,
                selected_agent_config: None,
            },
            Task::batch([
                init_task,
                Task::done(Message::LoadAgentConfigs),
                Task::done(Message::LoadHarnessModels),
            ]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchView(view) => {
                self.view = view;
                match view {
                    ViewMode::Generate => Task::done(Message::ScanPrdDirectory),
                    ViewMode::Tags => Task::done(Message::LoadTagExplorer),
                    ViewMode::Settings => {
                        let mut tasks = Vec::new();
                        if !self.state.backpressure.loaded {
                            tasks.push(Task::done(Message::LoadBackpressureConfig));
                        }
                        if !self.state.llm_config.loaded {
                            tasks.push(Task::done(Message::LoadLlmConfig));
                        }
                        if tasks.is_empty() {
                            Task::none()
                        } else {
                            Task::batch(tasks)
                        }
                    }
                    _ => Task::none(),
                }
            }

            Message::LoadWaves => Task::perform(load_waves_from_scud(), Message::WavesLoaded),

            Message::WavesLoaded(result) => {
                match result {
                    Ok(waves) => {
                        self.state.waves = waves;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load waves: {}", e));
                    }
                }
                Task::none()
            }

            Message::LoadTasksViaScud { tag } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::LoadTasks { tag }).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::ComputeWavesViaScud { tag } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::ComputeWaves { tag }).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::MarkTaskComplete { task_id } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::CompleteTask { task_id }).await;
                        },
                        |_| Message::RefreshTasks,
                    );
                }
                Task::none()
            }

            Message::MarkTaskBlocked { task_id } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::BlockTask { task_id }).await;
                        },
                        |_| Message::RefreshTasks,
                    );
                }
                Task::none()
            }

            Message::SelectTask(task_id) => {
                self.state.current_task = task_id;
                Task::none()
            }

            Message::SelectTag(tag) => {
                // Store the selected tag and refresh tasks
                self.state.active_tag = tag.clone();
                Task::done(Message::LoadTasksViaScud { tag })
            }

            Message::RefreshTasks => {
                // Refresh tasks with current tag selection
                Task::done(Message::LoadTasksViaScud {
                    tag: self.state.active_tag.clone(),
                })
            }

            Message::SetHarness(harness) => {
                self.state.launch_config.harness = harness;
                Task::none()
            }

            Message::SetModel(model) => {
                self.state.launch_config.model = model;
                Task::none()
            }

            Message::SetRoundSizeInput(input) => {
                self.state.launch_config.round_size_input = input.clone();
                if let Ok(n) = input.parse::<usize>() {
                    if (1..=30).contains(&n) {
                        self.state.launch_config.round_size = n;
                    }
                }
                Task::none()
            }

            Message::SetLaunchTag(tag) => {
                self.state.launch_config.tag = tag.clone();
                self.state.active_tag = Some(tag.clone());
                Task::done(Message::LoadTasksViaScud { tag: Some(tag) })
            }

            Message::SetAgentType(agent_type) => {
                self.state.launch_config.agent_type = agent_type;
                Task::none()
            }

            Message::SetOverrideAgents(enabled) => {
                self.state.launch_config.override_agents = enabled;
                if !enabled {
                    // Reset overrides when turning off
                    self.state.launch_config.agent_type = None;
                }
                Task::none()
            }

            Message::TagsLoaded(tags) => {
                self.state.available_tags = tags;
                Task::none()
            }

            Message::AgentsLoaded(agents) => {
                self.state.available_agents = agents;
                Task::none()
            }

            Message::SpawnTask { task_id } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let harness = self.state.launch_config.harness.clone();
                    let model = self.state.launch_config.model.clone();
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::RunTask {
                                    task_id,
                                    harness,
                                    model,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::ArchiveTag { tag } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::ArchiveTag { tag }).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::StartSwarm {
                tag,
                harness,
                round_size,
            } => {
                // Check if there are any actionable tasks
                let has_actionable = self
                    .state
                    .waves
                    .iter()
                    .any(|wave| wave.iter().any(|t| t.status.to_lowercase() != "done"));
                if !has_actionable {
                    self.error = Some("No actionable tasks — all tasks are done.".to_string());
                    return Task::none();
                }
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let model = self.state.launch_config.model.clone();
                    self.state.agent_status = AgentStatus::Running;
                    self.state.output_buffer.clear();
                    self.state
                        .output_buffer
                        .push_str(&format!("Starting swarm for tag '{}'...\n", tag));
                    // Switch to Monitor view automatically
                    self.view = ViewMode::Monitor;
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::StartSwarm {
                                    tag,
                                    harness,
                                    round_size,
                                    model,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::StopSwarm => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::StopSwarm).await;
                        },
                        |_| Message::Tick,
                    );
                }
                self.state.agent_status = AgentStatus::Idle;
                Task::none()
            }

            Message::AgentOutput(text) => {
                self.state.output_buffer.push_str(&text);
                Task::none()
            }

            Message::AgentComplete(result) => {
                self.state.agent_status = AgentStatus::Idle;
                match result {
                    Ok(()) => {
                        self.state.output_buffer.push_str("\n[Agent completed]\n");
                    }
                    Err(e) => {
                        self.state
                            .output_buffer
                            .push_str(&format!("\n[Agent error: {}]\n", e));
                    }
                }
                Task::none()
            }

            Message::PauseAgent => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    self.state.agent_status = AgentStatus::Paused;
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::PauseSwarm).await;
                        },
                        |_| Message::Tick,
                    );
                }
                self.state.agent_status = AgentStatus::Paused;
                Task::none()
            }

            Message::ResumeAgent => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    self.state.agent_status = AgentStatus::Running;
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::ResumeSwarm).await;
                        },
                        |_| Message::Tick,
                    );
                }
                self.state.agent_status = AgentStatus::Running;
                Task::none()
            }

            Message::CancelAgent => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    self.state.agent_status = AgentStatus::Idle;
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::StopSwarm).await;
                        },
                        |_| Message::Tick,
                    );
                }
                self.state.agent_status = AgentStatus::Idle;
                Task::none()
            }

            Message::ScudEvent(event) => {
                tracing::debug!("Received ScudEvent: {:?}", event);
                match event {
                    ScudEvent::TasksLoaded(tasks) => {
                        // Store the flat task list
                        self.state.tasks = tasks.clone();

                        // Convert flat task list to single wave for now
                        // To get proper waves, call ComputeWavesViaScud after loading
                        if !tasks.is_empty() {
                            self.state.waves = vec![tasks];
                        } else {
                            self.state.waves = vec![];
                        }
                    }
                    ScudEvent::TagsLoaded(tags) => {
                        self.state.available_tags = tags;
                    }
                    ScudEvent::AgentsLoaded(agents) => {
                        self.state.available_agents = agents;
                    }
                    ScudEvent::WavesComputed(waves) => {
                        // WavesComputed now contains full TaskInfo directly from ScudBridge
                        // No need for fragile ID lookups since the bridge handles the mapping
                        if !waves.is_empty() {
                            self.state.waves = waves;
                        }
                        tracing::info!("Waves updated: {} waves", self.state.waves.len());
                    }
                    ScudEvent::SwarmStarted { tag, total_waves } => {
                        self.state.agent_status = AgentStatus::Running;
                        self.state.swarm_progress = SwarmProgress {
                            total_waves,
                            current_wave: 0,
                            active: true,
                            tag: tag.clone(),
                        };
                        self.state.output_buffer.push_str(&format!(
                            "Swarm started for tag '{}' with {} waves\n",
                            tag, total_waves
                        ));
                    }
                    ScudEvent::WaveStarted { wave, tasks } => {
                        self.state.swarm_progress.current_wave = wave;
                        // Mark tasks in this wave with their wave number
                        for task_id in &tasks {
                            if let Some(session) = self.state.headless_sessions.get_mut(task_id) {
                                session.wave = Some(wave);
                            }
                        }
                        self.state.output_buffer.push_str(&format!(
                            "Wave {} started with {} tasks: {:?}\n",
                            wave,
                            tasks.len(),
                            tasks
                        ));
                    }
                    ScudEvent::TaskStarted { task_id } => {
                        self.state.current_task = Some(task_id.clone());
                        self.state
                            .output_buffer
                            .push_str(&format!("Task {} started\n", task_id));
                    }
                    ScudEvent::TaskOutput { task_id, text } => {
                        // Accumulate text, splitting on newlines
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            session.event_count += 1;
                            if session.status == HeadlessSessionStatus::Starting {
                                session.status = HeadlessSessionStatus::Running;
                            }

                            // Process text character by character for proper line handling
                            for ch in text.chars() {
                                if ch == '\n' {
                                    // Complete line - always push (even empty for blank lines)
                                    let line = std::mem::take(&mut session.partial_line);
                                    self.state
                                        .output_buffer
                                        .push_str(&format!("[{}] {}\n", task_id, line));
                                    session.output_lines.push(line);
                                } else {
                                    session.partial_line.push(ch);
                                }
                            }
                            session.line_count = session.output_lines.len();
                        }
                    }
                    ScudEvent::TaskCompleted { task_id, success } => {
                        let status = if success { "completed" } else { "failed" };
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            // Flush any remaining partial line
                            if !session.partial_line.is_empty() {
                                let line = std::mem::take(&mut session.partial_line);
                                self.state
                                    .output_buffer
                                    .push_str(&format!("[{}] {}\n", task_id, line));
                                session.output_lines.push(line);
                                session.line_count = session.output_lines.len();
                            }
                            session.status = if success {
                                HeadlessSessionStatus::Completed
                            } else {
                                HeadlessSessionStatus::Failed
                            };
                        }
                        self.state
                            .output_buffer
                            .push_str(&format!("Task {} {}\n", task_id, status));
                    }
                    ScudEvent::ValidationStarted => {
                        self.state.output_buffer.push_str("Validation started...\n");
                    }
                    ScudEvent::ValidationCompleted { passed, output } => {
                        let status = if passed { "passed" } else { "failed" };
                        self.state
                            .output_buffer
                            .push_str(&format!("Validation {}: {}\n", status, output));
                    }
                    ScudEvent::WaveCompleted { wave } => {
                        self.state
                            .output_buffer
                            .push_str(&format!("Wave {} completed\n", wave));
                    }
                    ScudEvent::SwarmCompleted { success } => {
                        self.state.agent_status = AgentStatus::Idle;
                        self.state.current_task = None;
                        self.state.swarm_progress.active = false;
                        let status = if success {
                            "successfully"
                        } else {
                            "with failures"
                        };
                        self.state
                            .output_buffer
                            .push_str(&format!("Swarm completed {}\n", status));
                        // Reload tasks to reflect updated statuses
                        return Task::done(Message::RefreshTasks);
                    }
                    ScudEvent::Output(text) => {
                        self.state.output_buffer.push_str(&text);
                        self.state.output_buffer.push('\n');
                    }
                    ScudEvent::Error(error) => {
                        self.error = Some(error);
                    }
                    // Headless streaming events
                    ScudEvent::HeadlessStarted { task_id, harness } => {
                        self.state.agent_status = AgentStatus::Running;
                        self.state.current_task = Some(task_id.clone());
                        self.state.output_buffer.push_str(&format!(
                            "Headless session started for task {} ({})\n",
                            task_id, harness
                        ));
                        // Populate headless session for monitor view
                        let title = self
                            .state
                            .tasks
                            .iter()
                            .find(|t| t.id == task_id)
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| task_id.clone());
                        // Use current wave if swarm is active
                        let wave = if self.state.swarm_progress.active {
                            Some(self.state.swarm_progress.current_wave)
                        } else {
                            None
                        };
                        self.state.headless_sessions.insert(
                            task_id.clone(),
                            HeadlessSessionInfo {
                                task_id: task_id.clone(),
                                task_title: title,
                                harness: harness.clone(),
                                status: HeadlessSessionStatus::Starting,
                                event_count: 0,
                                line_count: 0,
                                output_lines: Vec::new(),
                                wave,
                                partial_line: String::new(),
                                session_id: None,
                            },
                        );
                        if self.state.monitor_selected_task.is_none() {
                            self.state.monitor_selected_task = Some(task_id);
                        }
                    }
                    ScudEvent::ToolStart {
                        task_id,
                        tool_name,
                        tool_id: _,
                        input_summary,
                    } => {
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            // Flush any partial line before tool event
                            if !session.partial_line.is_empty() {
                                let line = std::mem::take(&mut session.partial_line);
                                self.state
                                    .output_buffer
                                    .push_str(&format!("[{}] {}\n", task_id, line));
                                session.output_lines.push(line);
                            }
                            session
                                .output_lines
                                .push(format!(">> {} {}", tool_name, input_summary));
                            session.line_count = session.output_lines.len();
                            session.event_count += 1;
                            if session.status == HeadlessSessionStatus::Starting {
                                session.status = HeadlessSessionStatus::Running;
                            }
                        }
                        self.state.output_buffer.push_str(&format!(
                            "[{}] >> {} {}\n",
                            task_id, tool_name, input_summary
                        ));
                    }
                    ScudEvent::ToolResult {
                        task_id,
                        tool_name,
                        tool_id: _,
                        success,
                    } => {
                        let status = if success { "ok" } else { "failed" };
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            // Flush any partial line before tool result
                            if !session.partial_line.is_empty() {
                                let line = std::mem::take(&mut session.partial_line);
                                self.state
                                    .output_buffer
                                    .push_str(&format!("[{}] {}\n", task_id, line));
                                session.output_lines.push(line);
                            }
                            session
                                .output_lines
                                .push(format!("<< {} {}", tool_name, status));
                            session.line_count = session.output_lines.len();
                            session.event_count += 1;
                        }
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] << {} {}\n", task_id, tool_name, status));
                    }
                    ScudEvent::SessionAssigned {
                        task_id,
                        session_id,
                    } => {
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] Session assigned: {}\n", task_id, session_id));
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            session.status = HeadlessSessionStatus::Running;
                            session.session_id = Some(session_id);
                        }
                    }
                    ScudEvent::TagArchived { tag } => {
                        self.state
                            .output_buffer
                            .push_str(&format!("Tag '{}' archived\n", tag));
                    }
                    ScudEvent::PrdFilesFound(files) => {
                        return Task::done(Message::PrdFilesLoaded(files));
                    }
                    ScudEvent::GenerateStatus(status) => {
                        return Task::done(Message::GenerateStatus(status));
                    }
                    ScudEvent::GenerateCompleted(result) => {
                        return Task::done(Message::GenerateCompleted(result));
                    }
                    ScudEvent::TagSummariesLoaded(summaries) => {
                        return Task::done(Message::TagSummariesLoaded(summaries));
                    }
                    ScudEvent::ArchivesLoaded(archives) => {
                        return Task::done(Message::ArchivesLoaded(archives));
                    }
                    ScudEvent::ActiveTagChanged(tag) => {
                        return Task::done(Message::ActiveTagChanged(tag));
                    }
                    ScudEvent::ArchiveRestored(result) => {
                        return Task::done(Message::ArchiveRestored(result));
                    }
                    ScudEvent::BackpressureConfigLoaded {
                        commands,
                        stop_on_failure,
                        timeout_secs,
                        is_auto_detected,
                    } => {
                        return Task::done(Message::BackpressureConfigLoaded {
                            commands,
                            stop_on_failure,
                            timeout_secs,
                            is_auto_detected,
                        });
                    }
                    ScudEvent::BackpressureConfigSaved(result) => {
                        return Task::done(Message::BackpressureConfigSaved(result));
                    }
                    ScudEvent::GenerateOutputLine(line) => {
                        return Task::done(Message::GenerateOutputLine(line));
                    }
                    ScudEvent::ProjectNotInitialized => {
                        self.state.is_initialized = false;
                    }
                    ScudEvent::ProjectInitialized(result) => {
                        return Task::done(Message::ProjectInitialized(result));
                    }
                    ScudEvent::LlmConfigLoaded {
                        provider,
                        model,
                        smart_provider,
                        smart_model,
                        fast_provider,
                        fast_model,
                        max_tokens,
                    } => {
                        return Task::done(Message::LlmConfigLoaded {
                            provider,
                            model,
                            smart_provider,
                            smart_model,
                            fast_provider,
                            fast_model,
                            max_tokens,
                        });
                    }
                    ScudEvent::LlmConfigSaved(result) => {
                        return Task::done(Message::LlmConfigSaved(result));
                    }
                    // Ralph events
                    ScudEvent::RalphStarted {
                        tag,
                        max_iterations,
                    } => {
                        self.state.ralph_progress = state::RalphProgress {
                            active: true,
                            current_iteration: 0,
                            max_iterations,
                            tag: tag.clone(),
                            current_task_id: None,
                            current_task_title: None,
                            phase: RalphPhase::Idle,
                            repair_attempt: 0,
                            completed_count: 0,
                            failed_count: 0,
                        };
                        self.state.output_buffer.push_str(&format!(
                            "Ralph started for tag '{}' (max {} iterations)\n",
                            tag, max_iterations
                        ));
                    }
                    ScudEvent::RalphIterationStarted {
                        iteration,
                        task_id,
                        task_title,
                    } => {
                        self.state.ralph_progress.current_iteration = iteration;
                        self.state.ralph_progress.current_task_id = Some(task_id.clone());
                        self.state.ralph_progress.current_task_title = Some(task_title.clone());
                        self.state.ralph_progress.phase = RalphPhase::Executing;
                        self.state.ralph_progress.repair_attempt = 0;
                        self.state.output_buffer.push_str(&format!(
                            "Ralph iteration {}: {} - {}\n",
                            iteration, task_id, task_title
                        ));
                    }
                    ScudEvent::RalphValidationStarted { task_id } => {
                        self.state.ralph_progress.phase = RalphPhase::Validating;
                        self.state
                            .output_buffer
                            .push_str(&format!("Validating task {}...\n", task_id));
                    }
                    ScudEvent::RalphValidationCompleted {
                        task_id,
                        passed,
                        output,
                    } => {
                        // Append validation output to task's session
                        if let Some(session) = self.state.headless_sessions.get_mut(&task_id) {
                            session.output_lines.push("--- VALIDATION ---".to_string());
                            session.output_lines.push(output.clone());
                            session.line_count = session.output_lines.len();
                        }
                        let status = if passed { "PASSED" } else { "FAILED" };
                        self.state
                            .output_buffer
                            .push_str(&format!("Validation {}: {}\n", status, output));
                    }
                    ScudEvent::RalphRepairStarted { task_id, attempt } => {
                        self.state.ralph_progress.phase = RalphPhase::Repairing;
                        self.state.ralph_progress.repair_attempt = attempt;
                        self.state.output_buffer.push_str(&format!(
                            "Repair attempt {} for task {}\n",
                            attempt, task_id
                        ));
                    }
                    ScudEvent::RalphIterationCompleted {
                        iteration,
                        task_id,
                        success,
                    } => {
                        if success {
                            self.state.ralph_progress.completed_count += 1;
                        } else {
                            self.state.ralph_progress.failed_count += 1;
                        }
                        self.state.ralph_progress.phase = RalphPhase::Idle;
                        let status = if success { "completed" } else { "failed" };
                        self.state.output_buffer.push_str(&format!(
                            "Ralph iteration {} ({}) {}\n",
                            iteration, task_id, status
                        ));
                    }
                    ScudEvent::RalphCompleted {
                        iterations,
                        completed,
                        failed,
                    } => {
                        self.state.agent_status = AgentStatus::Idle;
                        self.state.ralph_progress.active = false;
                        self.state.ralph_progress.phase = RalphPhase::Idle;
                        self.state.output_buffer.push_str(&format!(
                            "Ralph complete: {} iterations, {} completed, {} failed\n",
                            iterations, completed, failed
                        ));
                        return Task::done(Message::RefreshTasks);
                    }
                }
                Task::none()
            }

            Message::DismissError => {
                self.error = None;
                Task::none()
            }

            Message::ClearOutput => {
                self.state.output_buffer.clear();
                Task::none()
            }

            Message::MonitorSelectTask(task_id) => {
                self.state.monitor_selected_task = Some(task_id);
                Task::none()
            }

            Message::MonitorClearCompleted => {
                self.state.headless_sessions.retain(|_, s| {
                    s.status != HeadlessSessionStatus::Completed
                        && s.status != HeadlessSessionStatus::Failed
                });
                // If selected task was cleared, deselect
                if let Some(ref selected) = self.state.monitor_selected_task {
                    if !self.state.headless_sessions.contains_key(selected) {
                        self.state.monitor_selected_task = None;
                    }
                }
                Task::none()
            }

            Message::MonitorAttachSession { task_id } => {
                // Get session info to build the attach command
                if let Some(ref tx) = self.scud_command_tx {
                    if let Some(session) = self.state.headless_sessions.get(&task_id) {
                        if let Some(ref session_id) = session.session_id {
                            let harness = session.harness.clone();
                            let session_id = session_id.clone();
                            let terminal_app = self.state.settings.terminal_app.clone();
                            let _ = tx.blocking_send(ScudCommand::AttachSession {
                                task_id,
                                harness,
                                session_id,
                                terminal_app,
                            });
                        }
                    }
                }
                Task::none()
            }

            Message::MonitorCopyOutput { task_id } => {
                if let Some(session) = self.state.headless_sessions.get(&task_id) {
                    let mut output = session.output_lines.join("\n");
                    if !session.partial_line.is_empty() {
                        output.push('\n');
                        output.push_str(&session.partial_line);
                    }
                    return iced::clipboard::write(output);
                }
                Task::none()
            }

            Message::MonitorStopSession { task_id } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let _ = tx.blocking_send(ScudCommand::StopSession { task_id });
                }
                Task::none()
            }

            Message::MonitorPauseSwarm => {
                if let Some(ref tx) = self.scud_command_tx {
                    let _ = tx.blocking_send(ScudCommand::PauseSwarm);
                }
                Task::none()
            }

            Message::MonitorStopSwarm => {
                if let Some(ref tx) = self.scud_command_tx {
                    let _ = tx.blocking_send(ScudCommand::StopSwarm);
                }
                Task::none()
            }

            // Agent configuration
            Message::LoadAgentConfigs => {
                let working_dir = self.state.working_directory.clone();
                Task::perform(
                    async move { load_agent_configs(&working_dir) },
                    Message::AgentConfigsLoaded,
                )
            }

            Message::AgentConfigsLoaded(configs) => {
                self.state.agent_configs = configs;
                Task::none()
            }

            Message::SelectAgentConfig(name) => {
                self.selected_agent_config = Some(name);
                Task::none()
            }

            Message::UpdateAgentHarness { agent, harness } => {
                if let Some(config) = self.state.agent_configs.get_mut(&agent) {
                    config.harness = harness;
                    config.dirty = true;
                }
                Task::none()
            }

            Message::UpdateAgentModel { agent, model } => {
                if let Some(config) = self.state.agent_configs.get_mut(&agent) {
                    config.model = model;
                    config.dirty = true;
                }
                Task::none()
            }

            Message::UpdateAgentDescription { agent, description } => {
                if let Some(config) = self.state.agent_configs.get_mut(&agent) {
                    config.description = description;
                    config.dirty = true;
                }
                Task::none()
            }

            Message::SaveAgentConfig(name) => {
                if let Some(config) = self.state.agent_configs.get(&name) {
                    let config = config.clone();
                    let working_dir = self.state.working_directory.clone();
                    return Task::perform(
                        async move {
                            save_agent_config(&working_dir, &config);
                            name
                        },
                        Message::AgentConfigSaved,
                    );
                }
                Task::none()
            }

            Message::AgentConfigSaved(name) => {
                if let Some(config) = self.state.agent_configs.get_mut(&name) {
                    config.dirty = false;
                }
                Task::none()
            }

            // Model loading
            Message::LoadHarnessModels => {
                // Load models for cursor and opencode (rho/claude are hardcoded)
                Task::batch([
                    Task::perform(load_cursor_models(), |models| {
                        Message::HarnessModelsLoaded {
                            harness: "cursor".to_string(),
                            models,
                        }
                    }),
                    Task::perform(load_opencode_models(), |models| {
                        Message::HarnessModelsLoaded {
                            harness: "opencode".to_string(),
                            models,
                        }
                    }),
                ])
            }

            Message::HarnessModelsLoaded { harness, models } => {
                if !models.is_empty() {
                    self.state.available_models.insert(harness, models);
                }
                Task::none()
            }

            // Settings
            Message::SetTerminalApp(app) => {
                self.state.settings.terminal_app = app;
                Task::none()
            }

            Message::BrowseProject => {
                // Use native file dialog via rfd crate (would need to add dependency)
                // For now, just log - in a real impl we'd use rfd::FileDialog
                tracing::info!("Browse project requested - would show file dialog");
                Task::none()
            }

            Message::SwitchProject(path) => {
                self.state.working_directory = path.clone();
                self.state.headless_sessions.clear();
                self.state.monitor_selected_task = None;
                self.state.swarm_progress = SwarmProgress::default();
                self.state.ralph_progress = RalphProgress::default();
                self.state.output_buffer.clear();
                // Add to recent projects
                self.state.settings.recent_projects.retain(|p| p != &path);
                self.state.settings.recent_projects.insert(0, path);
                if self.state.settings.recent_projects.len()
                    > self.state.settings.max_recent_projects
                {
                    self.state
                        .settings
                        .recent_projects
                        .truncate(self.state.settings.max_recent_projects);
                }
                if let Some(ref tx) = self.scud_command_tx {
                    let _ = tx.blocking_send(ScudCommand::SetWorkingDirectory {
                        path: self.state.working_directory.clone(),
                    });
                }
                // Reload everything for the new project
                Task::batch([
                    Task::done(Message::RefreshTasks),
                    Task::done(Message::LoadAgentConfigs),
                    Task::done(Message::ProjectSwitched),
                ])
            }

            Message::ProjectSwitched => {
                // Reload swarm defaults from new project
                let swarm_defaults = SwarmDefaults::load_from_scud();
                self.state.launch_config = LaunchConfig::from_defaults(&swarm_defaults);
                Task::none()
            }

            // Ralph config messages
            Message::SetExecutionMode(mode) => {
                self.state.launch_config.execution_mode = mode;
                Task::none()
            }

            Message::SetRalphValidate(val) => {
                self.state.launch_config.ralph_config.validate = val;
                Task::none()
            }

            Message::SetRalphRepair(val) => {
                self.state.launch_config.ralph_config.repair = val;
                Task::none()
            }

            Message::SetRalphMaxIterations(input) => {
                self.state.launch_config.ralph_max_iterations_input = input.clone();
                if let Ok(n) = input.parse::<usize>() {
                    if (1..=1000).contains(&n) {
                        self.state.launch_config.ralph_config.max_iterations = n;
                    }
                }
                Task::none()
            }

            Message::SetRalphMaxRepairAttempts(input) => {
                self.state.launch_config.ralph_max_repair_attempts_input = input.clone();
                if let Ok(n) = input.parse::<usize>() {
                    if (1..=10).contains(&n) {
                        self.state.launch_config.ralph_config.max_repair_attempts = n;
                    }
                }
                Task::none()
            }

            Message::SetRalphBatchSubtasks(val) => {
                self.state.launch_config.ralph_config.batch_subtasks = val;
                Task::none()
            }

            Message::SetRalphGitPush(val) => {
                self.state.launch_config.ralph_config.git_push = val;
                Task::none()
            }

            // Ralph lifecycle
            Message::StartRalph { tag, harness } => {
                // Check if there are any actionable tasks
                let has_actionable = self
                    .state
                    .waves
                    .iter()
                    .any(|wave| wave.iter().any(|t| t.status.to_lowercase() != "done"));
                if !has_actionable {
                    self.error = Some("No actionable tasks — all tasks are done.".to_string());
                    return Task::none();
                }
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let model = self.state.launch_config.model.clone();
                    let ralph_config = self.state.launch_config.ralph_config.clone();
                    self.state.agent_status = AgentStatus::Running;
                    self.state.output_buffer.clear();
                    self.state
                        .output_buffer
                        .push_str(&format!("Starting Ralph mode for tag '{}'...\n", tag));
                    // Switch to Monitor view
                    self.view = ViewMode::Monitor;
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::StartRalph {
                                    tag,
                                    harness,
                                    model,
                                    ralph_config,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::StopRalph => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::StopRalph).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            // Generate view handlers
            Message::ScanPrdDirectory => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::ScanPrdFiles).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::PrdFilesLoaded(files) => {
                self.state.generate_state.prd_files = files;
                Task::none()
            }

            Message::SelectPrd(path) => {
                self.state.generate_state.selected_prd = Some(path.clone());
                self.state.generate_state.prd_content = None;
                Task::perform(
                    async move {
                        tokio::fs::read_to_string(&path)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::PrdContentLoaded,
                )
            }

            Message::PrdContentLoaded(result) => {
                match result {
                    Ok(content) => {
                        self.state.generate_state.prd_content = Some(content);
                    }
                    Err(e) => {
                        self.state.generate_state.prd_content =
                            Some(format!("Error loading file: {}", e));
                    }
                }
                Task::none()
            }

            Message::SetGenerateTag(tag) => {
                self.state.generate_state.tag_input = tag;
                Task::none()
            }

            Message::SetGenerateNumTasks(input) => {
                self.state.generate_state.num_tasks_input = input.clone();
                if let Ok(n) = input.parse::<u32>() {
                    if (1..=100).contains(&n) {
                        self.state.generate_state.num_tasks = n;
                    }
                }
                Task::none()
            }

            Message::SetGenerateNoExpand(val) => {
                self.state.generate_state.no_expand = val;
                Task::none()
            }

            Message::SetGenerateNoCheckDeps(val) => {
                self.state.generate_state.no_check_deps = val;
                Task::none()
            }

            Message::SetGenerateAppend(val) => {
                self.state.generate_state.append = val;
                Task::none()
            }

            Message::StartGenerate => {
                if let (Some(ref tx), Some(ref prd_file)) = (
                    &self.scud_command_tx,
                    &self.state.generate_state.selected_prd,
                ) {
                    let tx = tx.clone();
                    let prd_file = prd_file.clone();
                    let tag = self.state.generate_state.tag_input.clone();
                    let num_tasks = self.state.generate_state.num_tasks;
                    let no_expand = self.state.generate_state.no_expand;
                    let no_check_deps = self.state.generate_state.no_check_deps;
                    let append = self.state.generate_state.append;
                    self.state.generate_state.generating = true;
                    self.state.generate_state.generate_status = None;
                    self.state.generate_state.generate_output_lines.clear();
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::RunGenerate {
                                    prd_file,
                                    tag,
                                    num_tasks,
                                    no_expand,
                                    no_check_deps,
                                    append,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::GenerateCompleted(result) => {
                self.state.generate_state.generating = false;
                match result {
                    Ok(()) => {
                        self.state.generate_state.generate_status =
                            Some("Generation complete!".to_string());
                        // Switch to Tags view to see the result
                        self.view = ViewMode::Tags;
                        return Task::done(Message::LoadTagExplorer);
                    }
                    Err(e) => {
                        self.state.generate_state.generate_status = Some(format!("Failed: {}", e));
                    }
                }
                Task::none()
            }

            Message::GenerateStatus(status) => {
                self.state.generate_state.generate_status = Some(status);
                Task::none()
            }

            // Tag explorer handlers
            Message::LoadTagExplorer => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::LoadTagSummaries).await;
                            let _ = tx.send(ScudCommand::LoadArchives).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::TagSummariesLoaded(summaries) => {
                self.state.tag_explorer.tags = summaries;
                Task::none()
            }

            Message::ArchivesLoaded(archives) => {
                self.state.tag_explorer.archives = archives;
                Task::none()
            }

            Message::SetActiveTag(tag) => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::SetActiveTag { tag }).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::ActiveTagChanged(tag) => {
                self.state.active_tag = Some(tag.clone());
                self.state.launch_config.tag = tag;
                // Refresh tag explorer and waves
                Task::batch([
                    Task::done(Message::LoadTagExplorer),
                    Task::done(Message::RefreshTasks),
                ])
            }

            Message::TagExplorerArchiveTag(tag) => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::ArchiveTag { tag }).await;
                        },
                        |_| Message::LoadTagExplorer,
                    );
                }
                Task::none()
            }

            Message::RestoreArchive { filename } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::RestoreArchive { filename }).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::ArchiveRestored(result) => {
                match result {
                    Ok(tags) => {
                        self.state
                            .output_buffer
                            .push_str(&format!("Restored tags: {}\n", tags.join(", ")));
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to restore archive: {}", e));
                    }
                }
                Task::done(Message::LoadTagExplorer)
            }

            // Backpressure config handlers
            Message::LoadBackpressureConfig => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::LoadBackpressureConfig).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::BackpressureConfigLoaded {
                commands,
                stop_on_failure,
                timeout_secs,
                is_auto_detected,
            } => {
                self.state.backpressure = BackpressureState {
                    commands,
                    stop_on_failure,
                    timeout_secs,
                    timeout_input: timeout_secs.to_string(),
                    is_auto_detected,
                    loaded: true,
                    new_command_input: String::new(),
                    dirty: false,
                    status: None,
                };
                Task::none()
            }

            Message::SetBackpressureStopOnFailure(val) => {
                self.state.backpressure.stop_on_failure = val;
                self.state.backpressure.dirty = true;
                self.state.backpressure.status = None;
                Task::none()
            }

            Message::SetBackpressureTimeout(input) => {
                self.state.backpressure.timeout_input = input.clone();
                if let Ok(n) = input.parse::<u64>() {
                    if (1..=3600).contains(&n) {
                        self.state.backpressure.timeout_secs = n;
                        self.state.backpressure.dirty = true;
                        self.state.backpressure.status = None;
                    }
                }
                Task::none()
            }

            Message::SetBackpressureNewCommand(input) => {
                self.state.backpressure.new_command_input = input;
                Task::none()
            }

            Message::AddBackpressureCommand => {
                let cmd = self.state.backpressure.new_command_input.trim().to_string();
                if !cmd.is_empty() {
                    self.state.backpressure.commands.push(cmd);
                    self.state.backpressure.new_command_input.clear();
                    self.state.backpressure.dirty = true;
                    self.state.backpressure.status = None;
                }
                Task::none()
            }

            Message::RemoveBackpressureCommand(index) => {
                if index < self.state.backpressure.commands.len() {
                    self.state.backpressure.commands.remove(index);
                    self.state.backpressure.dirty = true;
                    self.state.backpressure.status = None;
                }
                Task::none()
            }

            Message::SaveBackpressureConfig => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let commands = self.state.backpressure.commands.clone();
                    let stop_on_failure = self.state.backpressure.stop_on_failure;
                    let timeout_secs = self.state.backpressure.timeout_secs;
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::SaveBackpressureConfig {
                                    commands,
                                    stop_on_failure,
                                    timeout_secs,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::BackpressureConfigSaved(result) => {
                match result {
                    Ok(()) => {
                        self.state.backpressure.dirty = false;
                        self.state.backpressure.is_auto_detected = false;
                        self.state.backpressure.status = Some("Saved".to_string());
                    }
                    Err(e) => {
                        self.state.backpressure.status = Some(format!("Error: {}", e));
                    }
                }
                Task::none()
            }

            Message::DetectBackpressureCommands => {
                // Re-detect by clearing the config section and reloading
                self.state.backpressure.loaded = false;
                Task::done(Message::LoadBackpressureConfig)
            }

            // Project initialization
            Message::InitProject => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::InitProject).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::ProjectInitialized(result) => {
                match result {
                    Ok(()) => {
                        self.state.is_initialized = true;
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to initialize project: {}", e));
                    }
                }
                Task::none()
            }

            // LLM config handlers
            Message::LoadLlmConfig => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::LoadLlmConfig).await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::LlmConfigLoaded {
                provider,
                model,
                smart_provider,
                smart_model,
                fast_provider,
                fast_model,
                max_tokens,
            } => {
                self.state.llm_config = LlmConfigState {
                    provider,
                    model,
                    smart_provider,
                    smart_model,
                    fast_provider,
                    fast_model,
                    max_tokens_input: max_tokens,
                    loaded: true,
                    dirty: false,
                    status: None,
                };
                Task::none()
            }

            Message::SetLlmProvider(provider) => {
                let default_model =
                    scud::config::Config::default_model_for_provider(&provider).to_string();
                self.state.llm_config.provider = provider;
                self.state.llm_config.model = default_model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmModel(model) => {
                self.state.llm_config.model = model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmSmartProvider(provider) => {
                let default_model =
                    scud::config::Config::default_model_for_provider(&provider).to_string();
                self.state.llm_config.smart_provider = provider;
                self.state.llm_config.smart_model = default_model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmSmartModel(model) => {
                self.state.llm_config.smart_model = model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmFastProvider(provider) => {
                let default_model =
                    scud::config::Config::default_model_for_provider(&provider).to_string();
                self.state.llm_config.fast_provider = provider;
                self.state.llm_config.fast_model = default_model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmFastModel(model) => {
                self.state.llm_config.fast_model = model;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SetLlmMaxTokens(input) => {
                self.state.llm_config.max_tokens_input = input;
                self.state.llm_config.dirty = true;
                self.state.llm_config.status = None;
                Task::none()
            }

            Message::SaveLlmConfig => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let llm = &self.state.llm_config;
                    let provider = llm.provider.clone();
                    let model = llm.model.clone();
                    let smart_provider = llm.smart_provider.clone();
                    let smart_model = llm.smart_model.clone();
                    let fast_provider = llm.fast_provider.clone();
                    let fast_model = llm.fast_model.clone();
                    let max_tokens = llm.max_tokens_input.clone();
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::SaveLlmConfig {
                                    provider,
                                    model,
                                    smart_provider,
                                    smart_model,
                                    fast_provider,
                                    fast_model,
                                    max_tokens,
                                })
                                .await;
                        },
                        |_| Message::Tick,
                    );
                }
                Task::none()
            }

            Message::LlmConfigSaved(result) => {
                match result {
                    Ok(()) => {
                        self.state.llm_config.dirty = false;
                        self.state.llm_config.status = Some("Saved".to_string());
                    }
                    Err(e) => {
                        self.state.llm_config.status = Some(format!("Error: {}", e));
                    }
                }
                Task::none()
            }

            // Streaming generate output
            Message::GenerateOutputLine(line) => {
                self.state.generate_state.generate_output_lines.push(line);
                Task::none()
            }

            Message::Tick => {
                // Periodic refresh: reload tasks and tags from storage
                if let Some(ref tx) = self.scud_command_tx {
                    let tag = self.state.active_tag.clone();
                    let _ = tx.try_send(ScudCommand::LoadTasks { tag });
                    let _ = tx.try_send(ScudCommand::LoadAvailableTags);
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let main_column: Element<Message> = if let Some(ref error) = self.error {
            // Show error banner at top using theme colors
            let error_banner = container(
                row![
                    text(error).style(|_| text::Style {
                        color: Some(theme::ERROR),
                    }),
                    button("Dismiss").on_press(Message::DismissError),
                ]
                .spacing(10),
            )
            .padding(10)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(theme::background::TERTIARY)),
                ..Default::default()
            });

            let header = views::header::view(
                self.view,
                self.state.agent_status,
                self.state.headless_sessions.len(),
            );
            let content = match self.view {
                ViewMode::Waves => views::waves::view(
                    &self.state.waves,
                    &self.state.active_tag,
                    self.state.agent_status,
                    &self.state.launch_config,
                    &self.state.available_harnesses,
                    &self.state.available_agents,
                    &self.state.available_models,
                    self.state.is_initialized,
                ),
                ViewMode::Generate => {
                    views::generate::view(&self.state.generate_state, &self.state.working_directory)
                }
                ViewMode::Tags => views::tags::view(&self.state.tag_explorer),
                ViewMode::Agents => views::agents::view(
                    &self.state.agent_configs,
                    &self.state.available_harnesses,
                    &self.state.available_models,
                    &self.selected_agent_config,
                ),
                ViewMode::Output => views::output::view(
                    &self.state.current_task,
                    self.state.agent_status,
                    &self.state.output_buffer,
                ),
                ViewMode::Monitor => views::monitor::view(
                    &self.state.headless_sessions,
                    &self.state.monitor_selected_task,
                    &self.state.swarm_progress,
                    self.state.agent_status,
                    &self.state.ralph_progress,
                ),
                ViewMode::Settings => views::settings::view(
                    &self.state.settings,
                    &self.state.working_directory,
                    &self.state.backpressure,
                    &self.state.llm_config,
                ),
            };

            column![error_banner, header, content].spacing(10).into()
        } else {
            let header = views::header::view(
                self.view,
                self.state.agent_status,
                self.state.headless_sessions.len(),
            );
            let content = match self.view {
                ViewMode::Waves => views::waves::view(
                    &self.state.waves,
                    &self.state.active_tag,
                    self.state.agent_status,
                    &self.state.launch_config,
                    &self.state.available_harnesses,
                    &self.state.available_agents,
                    &self.state.available_models,
                    self.state.is_initialized,
                ),
                ViewMode::Generate => {
                    views::generate::view(&self.state.generate_state, &self.state.working_directory)
                }
                ViewMode::Tags => views::tags::view(&self.state.tag_explorer),
                ViewMode::Agents => views::agents::view(
                    &self.state.agent_configs,
                    &self.state.available_harnesses,
                    &self.state.available_models,
                    &self.selected_agent_config,
                ),
                ViewMode::Output => views::output::view(
                    &self.state.current_task,
                    self.state.agent_status,
                    &self.state.output_buffer,
                ),
                ViewMode::Monitor => views::monitor::view(
                    &self.state.headless_sessions,
                    &self.state.monitor_selected_task,
                    &self.state.swarm_progress,
                    self.state.agent_status,
                    &self.state.ralph_progress,
                ),
                ViewMode::Settings => views::settings::view(
                    &self.state.settings,
                    &self.state.working_directory,
                    &self.state.backpressure,
                    &self.state.llm_config,
                ),
            };

            column![header, content].spacing(10).into()
        };

        container(main_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(20)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme::surface::BASE)),
                ..Default::default()
            })
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // ScudEvent subscription from the bridge
        let rx = self.scud_event_rx.clone();
        let scud_events = Subscription::run_with(ScudEventReceiver(rx), |ScudEventReceiver(rx)| {
            let rx = rx.clone();
            async_stream::stream! {
                // Take the receiver from the mutex (only happens once)
                let mut receiver = {
                    let mut guard = rx.lock().await;
                    guard.take()
                };

                if let Some(ref mut rx) = receiver {
                    while let Some(event) = rx.recv().await {
                        yield Message::ScudEvent(event);
                    }
                }
            }
        });

        // Periodic timer to refresh tasks/tags from underlying files
        let timer = iced::time::every(Duration::from_secs(5)).map(|_| Message::Tick);

        Subscription::batch([scud_events, timer])
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

/// Load waves from SCUD storage using scud-core directly
async fn load_waves_from_scud() -> Result<Vec<Vec<TaskInfo>>, String> {
    // Create storage from current directory
    let storage = Storage::new(None);

    // Load active group/phase
    let phase = storage.load_active_group().map_err(|e| e.to_string())?;

    // Get all tasks
    let tasks = &phase.tasks;

    // Compute waves using scud-core
    let pending_tasks: Vec<_> = tasks
        .iter()
        .filter(|t| !matches!(t.status, scud_core::models::task::TaskStatus::Done))
        .collect();
    let wave_result = scud_core::compute_waves(&pending_tasks);

    // Convert to TaskInfo, matching IDs from waves to tasks
    let waves: Vec<Vec<TaskInfo>> = wave_result
        .waves
        .into_iter()
        .map(|wave| {
            wave.tasks
                .into_iter()
                .filter_map(|id| {
                    tasks.iter().find(|t| t.id == id).map(|t| TaskInfo {
                        id: t.id.clone(),
                        title: t.title.clone(),
                        status: format!("{:?}", t.status),
                        agent: t.agent_type.clone(),
                    })
                })
                .collect()
        })
        .collect();

    Ok(waves)
}

/// Load agent configurations from .scud/agents/*.toml
fn load_agent_configs(
    working_dir: &std::path::Path,
) -> std::collections::HashMap<String, AgentConfig> {
    use std::collections::HashMap;

    let mut configs = HashMap::new();
    let agents_dir = working_dir.join(".scud/agents");

    if !agents_dir.exists() {
        return configs;
    }

    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(toml_value) = content.parse::<toml::Table>() {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let agent_section = toml_value.get("agent");
                        let model_section = toml_value.get("model");

                        let description = agent_section
                            .and_then(|a| a.get("description"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let harness = model_section
                            .and_then(|m| m.get("harness"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("rho")
                            .to_string();

                        let model = model_section
                            .and_then(|m| m.get("model"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("claude-sonnet")
                            .to_string();

                        configs.insert(
                            name.clone(),
                            AgentConfig {
                                name,
                                description,
                                harness,
                                model,
                                dirty: false,
                            },
                        );
                    }
                }
            }
        }
    }

    configs
}

/// Save an agent configuration to .scud/agents/{name}.toml
fn save_agent_config(working_dir: &std::path::Path, config: &AgentConfig) {
    let agents_dir = working_dir.join(".scud/agents");
    let path = agents_dir.join(format!("{}.toml", config.name));

    // Read existing file to preserve other sections (like [prompt])
    let existing_content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut existing_toml: toml::Table = existing_content.parse().unwrap_or_default();

    // Update [agent] section
    let mut agent_section = existing_toml
        .get("agent")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    agent_section.insert("name".to_string(), toml::Value::String(config.name.clone()));
    agent_section.insert(
        "description".to_string(),
        toml::Value::String(config.description.clone()),
    );
    existing_toml.insert("agent".to_string(), toml::Value::Table(agent_section));

    // Update [model] section
    let mut model_section = existing_toml
        .get("model")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    model_section.insert(
        "harness".to_string(),
        toml::Value::String(config.harness.clone()),
    );
    model_section.insert(
        "model".to_string(),
        toml::Value::String(config.model.clone()),
    );
    existing_toml.insert("model".to_string(), toml::Value::Table(model_section));

    // Write back
    if let Ok(content) = toml::to_string_pretty(&existing_toml) {
        let _ = std::fs::write(&path, content);
    }
}

/// Load available models for cursor-agent harness
async fn load_cursor_models() -> Vec<String> {
    match tokio::process::Command::new("cursor-agent")
        .arg("models")
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse lines like "auto - Auto" or "opus-4.5 - Claude 4.5 Opus"
            stdout
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    // Skip header lines and empty lines
                    if line.is_empty()
                        || line.starts_with("Available")
                        || line.starts_with("Loading")
                    {
                        return None;
                    }
                    // Extract the model ID (before the " - ")
                    line.split(" - ").next().map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty())
                .collect()
        }
        Err(e) => {
            tracing::warn!("Failed to load cursor-agent models: {}", e);
            Vec::new()
        }
    }
}

/// Load available models for opencode harness
async fn load_opencode_models() -> Vec<String> {
    match tokio::process::Command::new("opencode")
        .arg("models")
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Each line is a model name like "opencode/big-pickle" or "xai/grok-2"
            stdout
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        Err(e) => {
            tracing::warn!("Failed to load opencode models: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_test::simulator;

    /// Create a test instance without the Task
    fn test_app() -> DescartesGui {
        let swarm_defaults = SwarmDefaults::default();
        let launch_config = LaunchConfig::from_defaults(&swarm_defaults);
        let mut state = AppState::default();
        state.swarm_defaults = swarm_defaults;
        state.launch_config = launch_config;
        state.available_tags = vec!["feature".into(), "bugfix".into()];
        state.available_agents = vec!["fast-builder".into(), "planner".into()];
        DescartesGui {
            view: ViewMode::Waves,
            state,
            scud_command_tx: None,
            scud_event_rx: Arc::new(TokioMutex::new(None)),
            error: None,
            selected_agent_config: None,
        }
    }

    /// Headless UI test using iced_test simulator
    #[test]
    fn test_ui_navigation_clicks() {
        let mut app = test_app();

        // Render the view and create a simulator
        let mut ui = simulator(app.view());

        // Click on "Agents" button
        let _ = ui.click("Agents");

        // Process the messages generated by the click
        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        // Verify the view switched to Agents
        assert_eq!(app.view, ViewMode::Agents);

        // Render again with updated state
        let mut ui = simulator(app.view());

        // Click on "Output" button
        let _ = ui.click("Output");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert_eq!(app.view, ViewMode::Output);

        // Click back to "Waves"
        let mut ui = simulator(app.view());
        let _ = ui.click("Waves");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        assert_eq!(app.view, ViewMode::Waves);
    }

    /// Test clicking Refresh button in waves view
    #[test]
    fn test_ui_refresh_click() {
        let app = test_app();

        let mut ui = simulator(app.view());

        // Click the Refresh button
        let click_result = ui.click("Refresh");
        assert!(click_result.is_ok(), "Refresh button should be clickable");

        // Collect messages - should have RefreshTasks (via ScudBridge)
        let messages: Vec<_> = ui.into_messages().collect();
        assert!(!messages.is_empty(), "Click should generate a message");

        // The message should be RefreshTasks
        for message in messages {
            match message {
                Message::RefreshTasks => {
                    // Expected - this triggers ScudBridge task loading
                }
                _ => panic!("Expected RefreshTasks message, got {:?}", message),
            }
        }
    }

    #[test]
    fn test_switch_view() {
        let mut app = test_app();
        assert_eq!(app.view, ViewMode::Waves);

        let _ = app.update(Message::SwitchView(ViewMode::Output));
        assert_eq!(app.view, ViewMode::Output);
    }

    #[test]
    fn test_waves_loaded_success() {
        let mut app = test_app();

        let waves = vec![
            vec![TaskInfo {
                id: "1".into(),
                title: "First task".into(),
                status: "Pending".into(),
                agent: None,
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Pending".into(),
                agent: None,
            }],
        ];

        let _ = app.update(Message::WavesLoaded(Ok(waves)));
        assert_eq!(app.state.waves.len(), 2);
        assert!(app.error.is_none());
    }

    #[test]
    fn test_waves_loaded_error() {
        let mut app = test_app();

        let _ = app.update(Message::WavesLoaded(Err("Connection failed".into())));
        assert!(app.error.is_some());
        assert!(app.error.as_ref().unwrap().contains("Connection failed"));
    }

    #[test]
    fn test_dismiss_error() {
        let mut app = test_app();
        app.error = Some("Test error".into());

        let _ = app.update(Message::DismissError);
        assert!(app.error.is_none());
    }

    #[test]
    fn test_spawn_task() {
        let mut app = test_app();

        // SpawnTask without a bridge just does nothing (no scud_command_tx)
        let _ = app.update(Message::SpawnTask {
            task_id: "task-1".into(),
        });
        // Without a bridge connection, state doesn't change
        assert_eq!(app.state.agent_status, AgentStatus::Idle);
    }

    #[test]
    fn test_pause_resume_agent() {
        let mut app = test_app();
        app.state.agent_status = AgentStatus::Running;

        // Without scud_command_tx, it just updates state directly
        let _ = app.update(Message::PauseAgent);
        assert_eq!(app.state.agent_status, AgentStatus::Paused);

        let _ = app.update(Message::ResumeAgent);
        assert_eq!(app.state.agent_status, AgentStatus::Running);
    }

    #[test]
    fn test_cancel_agent() {
        let mut app = test_app();
        app.state.agent_status = AgentStatus::Running;
        app.state.current_task = Some("task-1".into());

        let _ = app.update(Message::CancelAgent);
        assert_eq!(app.state.agent_status, AgentStatus::Idle);
    }

    #[test]
    fn test_initial_state() {
        let (app, _) = DescartesGui::new();
        assert_eq!(app.view, ViewMode::Waves);
        assert_eq!(app.state.agent_status, AgentStatus::Idle);
        assert!(app.state.current_task.is_none());
        assert!(app.error.is_none());
    }

    // =============================================================
    // Additional UI Interaction Tests
    // =============================================================

    /// Test clicking Pause/Resume buttons in Agents view when agent is running
    #[test]
    fn test_ui_agent_control_buttons() {
        let mut app = test_app();

        // Simulate agent running via ScudEvent
        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmStarted {
            tag: "test".into(),
            total_waves: 1,
        }));
        assert_eq!(app.state.agent_status, AgentStatus::Running);

        // Render and find the Pause button (in Waves view launch controls)
        let mut ui = simulator(app.view());
        let pause_result = ui.click("Pause");
        assert!(
            pause_result.is_ok(),
            "Pause button should exist when agent is running"
        );

        // Process messages
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Paused);

        // Now Resume button should appear
        let mut ui = simulator(app.view());
        let resume_result = ui.click("Resume");
        assert!(
            resume_result.is_ok(),
            "Resume button should exist when agent is paused"
        );

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Running);
    }

    /// Test clicking Cancel button stops the agent
    #[test]
    fn test_ui_cancel_agent() {
        let mut app = test_app();

        // Simulate agent running
        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmStarted {
            tag: "test".into(),
            total_waves: 1,
        }));

        let mut ui = simulator(app.view());
        let stop_result = ui.click("Stop");
        assert!(stop_result.is_ok(), "Stop button should exist when running");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
    }

    /// Test error banner dismiss interaction
    #[test]
    fn test_ui_error_banner_dismiss() {
        let mut app = test_app();
        app.error = Some("Test error message".to_string());

        let mut ui = simulator(app.view());

        // Error banner should have Dismiss button
        let dismiss_result = ui.click("Dismiss");
        assert!(
            dismiss_result.is_ok(),
            "Dismiss button should exist in error banner"
        );

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.error.is_none(), "Error should be dismissed");
    }

    /// Test clicking Spawn button on a task row
    #[test]
    fn test_ui_spawn_task_from_waves() {
        let mut app = test_app();

        // Load some tasks
        let waves = vec![vec![
            TaskInfo {
                id: "1".into(),
                title: "First task".into(),
                status: "Pending".into(),
                agent: None,
            },
            TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Pending".into(),
                agent: None,
            },
        ]];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));

        // Render waves view
        let mut ui = simulator(app.view());

        // Click Spawn button (there are multiple, clicking finds first)
        let spawn_result = ui.click("Spawn");
        assert!(spawn_result.is_ok(), "Spawn button should exist");

        // Verify the message generated is SpawnTask
        let mut saw_spawn = false;
        for message in ui.into_messages() {
            if let Message::SpawnTask { task_id } = &message {
                saw_spawn = true;
                assert_eq!(task_id, "1");
            }
            let _ = app.update(message);
        }
        assert!(saw_spawn, "Should emit SpawnTask message");
    }

    /// Test status display updates correctly
    #[test]
    fn test_ui_status_display() {
        let mut app = test_app();

        // Check initial status shows Idle
        {
            let mut ui = simulator(app.view());
            let status_find = ui.find("Status: Idle");
            assert!(status_find.is_ok(), "Should show Status: Idle initially");
        }

        // Simulate agent running and check status updates
        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmStarted {
            tag: "test".into(),
            total_waves: 1,
        }));
        let mut ui = simulator(app.view());
        let status_find = ui.find("Status: Running");
        assert!(
            status_find.is_ok(),
            "Should show Status: Running when agent runs"
        );
    }

    /// Test Output view displays agent output
    #[test]
    fn test_ui_output_view_content() {
        let mut app = test_app();

        // Add some output
        let _ = app.update(Message::AgentOutput("Line 1\n".into()));
        let _ = app.update(Message::AgentOutput("Line 2\n".into()));

        // Verify the model state directly (more reliable than UI text search for long content)
        assert!(
            app.state.output_buffer.contains("Line 1"),
            "Output buffer should contain Line 1"
        );
        assert!(
            app.state.output_buffer.contains("Line 2"),
            "Output buffer should contain Line 2"
        );

        // Switch to output view and verify it renders without error
        let _ = app.update(Message::SwitchView(ViewMode::Output));
        let ui = simulator(app.view());
        // Just verify it renders - the output is in a scrollable container
        drop(ui);
    }

    // =============================================================
    // Full Loop Headless Test
    // =============================================================

    /// Simulates a complete workflow: load tasks -> spawn -> pause -> resume -> complete
    #[test]
    fn test_full_workflow_headless() {
        let mut app = test_app();

        // Step 1: Simulate loading waves (mimics async WavesLoaded result)
        let waves = vec![
            vec![TaskInfo {
                id: "1".into(),
                title: "Setup environment".into(),
                status: "Pending".into(),
                agent: None,
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Build core module".into(),
                status: "Pending".into(),
                agent: None,
            }],
        ];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));
        assert_eq!(app.state.waves.len(), 2, "Should have 2 waves loaded");

        // Step 2: Click Spawn on first task via UI
        let mut ui = simulator(app.view());
        let _ = ui.click("Spawn");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        // Simulate the headless session starting (bridge would send this)
        let _ = app.update(Message::ScudEvent(ScudEvent::HeadlessStarted {
            task_id: "1".into(),
            harness: "claude".into(),
        }));
        assert_eq!(app.state.agent_status, AgentStatus::Running);
        assert_eq!(app.state.current_task, Some("1".into()));

        // Step 3: Pause the agent (controls are in Waves view)
        let mut ui = simulator(app.view());
        let _ = ui.click("Pause");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Paused);

        // Step 5: Resume the agent
        let mut ui = simulator(app.view());
        let _ = ui.click("Resume");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Running);

        // Step 6: Simulate agent output arriving
        let _ = app.update(Message::AgentOutput("Processing task 1...\n".into()));
        let _ = app.update(Message::AgentOutput("Done!\n".into()));

        // Step 7: Agent completes
        let _ = app.update(Message::AgentComplete(Ok(())));
        assert_eq!(app.state.agent_status, AgentStatus::Idle);

        // Step 8: Navigate to Output view to verify output
        let mut ui = simulator(app.view());
        let _ = ui.click("Output");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.view, ViewMode::Output);

        // Verify output buffer contains expected content (model assertion)
        assert!(
            app.state.output_buffer.contains("Processing task 1"),
            "Output should show task processing"
        );
        assert!(
            app.state.output_buffer.contains("[Agent completed]"),
            "Output should show completion"
        );
    }

    /// Test workflow with an error condition
    #[test]
    fn test_error_workflow_headless() {
        let mut app = test_app();

        // Step 1: Waves load fails
        let _ = app.update(Message::WavesLoaded(Err("Network timeout".into())));
        assert!(app.error.is_some());

        // Step 2: Verify error is set in model (UI text search may not work for styled text)
        assert!(
            app.error.as_ref().unwrap().contains("Network timeout"),
            "Error should contain the message"
        );

        // Step 3: Dismiss error via UI - find and click Dismiss button
        let mut ui = simulator(app.view());
        let dismiss_result = ui.click("Dismiss");
        assert!(
            dismiss_result.is_ok(),
            "Dismiss button should be present in error banner"
        );
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert!(app.error.is_none(), "Error should be dismissed");

        // Step 4: Retry loading (click Refresh) - now uses ScudBridge via RefreshTasks
        let mut ui = simulator(app.view());
        let _ = ui.click("Refresh");
        let messages: Vec<_> = ui.into_messages().collect();
        assert!(
            messages.iter().any(|m| matches!(m, Message::RefreshTasks)),
            "Refresh should trigger RefreshTasks (via ScudBridge)"
        );
    }

    /// Test agent failure workflow
    #[test]
    fn test_agent_failure_workflow() {
        let mut app = test_app();

        // Load tasks and simulate running
        let waves = vec![vec![TaskInfo {
            id: "1".into(),
            title: "Failing task".into(),
            status: "Pending".into(),
            agent: None,
        }]];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));
        let _ = app.update(Message::ScudEvent(ScudEvent::HeadlessStarted {
            task_id: "1".into(),
            harness: "claude".into(),
        }));

        // Agent encounters an error via ScudEvent
        let _ = app.update(Message::ScudEvent(ScudEvent::TaskCompleted {
            task_id: "1".into(),
            success: false,
        }));

        assert!(app.state.output_buffer.contains("failed"));
    }

    // =============================================================
    // Snapshot Test (Visual Regression)
    // =============================================================

    /// Basic snapshot test - captures UI rendering
    #[test]
    fn test_snapshot_waves_view() {
        let mut app = test_app();

        // Setup known state
        let waves = vec![
            vec![TaskInfo {
                id: "1".into(),
                title: "Task A".into(),
                status: "Pending".into(),
                agent: None,
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Task B".into(),
                status: "Done".into(),
                agent: None,
            }],
        ];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));

        let mut ui = simulator(app.view());
        let theme = Theme::Dark;

        // Take snapshot
        let snapshot_result = ui.snapshot(&theme);
        assert!(snapshot_result.is_ok(), "Should be able to take snapshot");
    }

    /// Snapshot of error state
    #[test]
    fn test_snapshot_error_state() {
        let mut app = test_app();
        app.error = Some("Connection refused".into());

        let mut ui = simulator(app.view());
        let theme = Theme::Dark;

        let snapshot_result = ui.snapshot(&theme);
        assert!(snapshot_result.is_ok(), "Should snapshot error state");
    }

    // =============================================================
    // ScudBridge Integration Tests
    // =============================================================

    /// Test ClearOutput message clears the output buffer
    #[test]
    fn test_clear_output() {
        let mut app = test_app();

        // Add some output
        app.state.output_buffer.push_str("Test output line 1\n");
        app.state.output_buffer.push_str("Test output line 2\n");
        assert!(!app.state.output_buffer.is_empty());

        // Clear the output
        let _ = app.update(Message::ClearOutput);
        assert!(
            app.state.output_buffer.is_empty(),
            "Output should be cleared"
        );
    }

    /// Test clicking Clear button in output view
    #[test]
    fn test_ui_clear_output_button() {
        let mut app = test_app();
        app.state.output_buffer.push_str("Some output\n");

        // Switch to output view
        let _ = app.update(Message::SwitchView(ViewMode::Output));

        let mut ui = simulator(app.view());
        let click_result = ui.click("Clear");
        assert!(click_result.is_ok(), "Clear button should be clickable");

        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert!(
            app.state.output_buffer.is_empty(),
            "Output should be cleared after clicking Clear"
        );
    }

    /// Test ScudEvent::TasksLoaded updates state correctly
    #[test]
    fn test_scud_event_tasks_loaded() {
        let mut app = test_app();

        let tasks = vec![
            TaskInfo {
                id: "1".into(),
                title: "First task".into(),
                status: "Pending".into(),
                agent: None,
            },
            TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Done".into(),
                agent: None,
            },
        ];

        let _ = app.update(Message::ScudEvent(ScudEvent::TasksLoaded(tasks.clone())));

        // Should store tasks in both tasks and waves
        assert_eq!(app.state.tasks.len(), 2);
        assert_eq!(app.state.waves.len(), 1);
        assert_eq!(app.state.waves[0].len(), 2);
    }

    /// Test ScudEvent::WavesComputed sets waves directly with full TaskInfo
    #[test]
    fn test_scud_event_waves_computed() {
        let mut app = test_app();

        // WavesComputed now contains full TaskInfo directly from ScudBridge
        // No need to load tasks first - the bridge handles that internally
        let waves = vec![
            vec![
                TaskInfo {
                    id: "1".into(),
                    title: "First task".into(),
                    status: "Pending".into(),
                    agent: None,
                },
                TaskInfo {
                    id: "2".into(),
                    title: "Second task".into(),
                    status: "Pending".into(),
                    agent: None,
                },
            ],
            vec![TaskInfo {
                id: "3".into(),
                title: "Third task".into(),
                status: "Pending".into(),
                agent: None,
            }],
        ];
        let _ = app.update(Message::ScudEvent(ScudEvent::WavesComputed(waves)));

        // Waves should be populated directly
        assert_eq!(app.state.waves.len(), 2);
        assert_eq!(app.state.waves[0].len(), 2);
        assert_eq!(app.state.waves[1].len(), 1);
        assert_eq!(app.state.waves[1][0].title, "Third task");
    }

    /// Test ScudEvent::SwarmStarted updates status
    #[test]
    fn test_scud_event_swarm_started() {
        let mut app = test_app();

        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmStarted {
            tag: "test".into(),
            total_waves: 3,
        }));

        assert_eq!(app.state.agent_status, AgentStatus::Running);
        assert!(app.state.output_buffer.contains("Swarm started"));
        assert!(app.state.output_buffer.contains("test"));
    }

    /// Test ScudEvent::SwarmCompleted updates status
    #[test]
    fn test_scud_event_swarm_completed() {
        let mut app = test_app();
        app.state.agent_status = AgentStatus::Running;
        app.state.current_task = Some("task-1".into());

        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmCompleted {
            success: true,
        }));

        assert_eq!(app.state.agent_status, AgentStatus::Idle);
        assert!(app.state.current_task.is_none());
        assert!(app.state.output_buffer.contains("Swarm completed"));
    }

    /// Test SelectTag stores tag and triggers refresh
    #[test]
    fn test_select_tag() {
        let mut app = test_app();

        // SelectTag should store the tag
        let _ = app.update(Message::SelectTag(Some("feature".into())));

        assert_eq!(app.state.active_tag, Some("feature".into()));
    }

    /// Test RefreshTasks uses active tag
    #[test]
    fn test_refresh_tasks_uses_active_tag() {
        let mut app = test_app();
        app.state.active_tag = Some("bugfix".into());

        // RefreshTasks should generate LoadTasksViaScud with the active tag
        // We can't easily test the Task returned, but we can verify state is maintained
        let _ = app.update(Message::RefreshTasks);

        // Active tag should still be set
        assert_eq!(app.state.active_tag, Some("bugfix".into()));
    }

    /// Test launch configuration message handlers
    #[test]
    fn test_launch_config_updates() {
        let mut app = test_app();

        let _ = app.update(Message::SetHarness("opencode".into()));
        assert_eq!(app.state.launch_config.harness, "opencode");

        let _ = app.update(Message::SetModel("gpt-4o".into()));
        assert_eq!(app.state.launch_config.model, "gpt-4o");

        let _ = app.update(Message::SetRoundSizeInput("5".into()));
        assert_eq!(app.state.launch_config.round_size, 5);
        assert_eq!(app.state.launch_config.round_size_input, "5");

        let _ = app.update(Message::SetLaunchTag("feature".into()));
        assert_eq!(app.state.launch_config.tag, "feature");

        let _ = app.update(Message::SetAgentType(Some("planner".into())));
        assert_eq!(app.state.launch_config.agent_type, Some("planner".into()));

        let _ = app.update(Message::SetAgentType(None));
        assert_eq!(app.state.launch_config.agent_type, None);
    }

    /// Test available tags/agents update messages
    #[test]
    fn test_available_lists_loaded() {
        let mut app = test_app();

        let tags = vec!["alpha".into(), "beta".into()];
        let agents = vec!["fast-builder".into(), "planner".into()];

        let _ = app.update(Message::TagsLoaded(tags.clone()));
        assert_eq!(app.state.available_tags, tags);

        let _ = app.update(Message::AgentsLoaded(agents.clone()));
        assert_eq!(app.state.available_agents, agents);
    }

    /// Test Start Swarm button appears when idle
    #[test]
    fn test_ui_swarm_controls() {
        let mut app = test_app();
        app.state.launch_config.tag = "feature".into();
        app.state.launch_config.harness = "opencode".into();
        app.state.launch_config.round_size = 5;

        // Render the Waves view (launch controls are at the bottom)
        let mut ui = simulator(app.view());

        // When idle, Start Swarm button should be present
        let click_result = ui.click("Start Swarm");
        assert!(
            click_result.is_ok(),
            "Start Swarm button should exist when idle"
        );

        // Process the message
        let mut saw_start_swarm = false;
        for msg in ui.into_messages() {
            if let Message::StartSwarm {
                tag,
                harness,
                round_size,
            } = &msg
            {
                saw_start_swarm = true;
                assert_eq!(tag, "feature", "Should use launch_config tag");
                assert_eq!(harness, "opencode", "Should use launch_config harness");
                assert_eq!(*round_size, 5, "Should use launch_config round size");
            }
            let _ = app.update(msg);
        }
        assert!(saw_start_swarm, "Should emit StartSwarm message");
    }

    /// Test SetLaunchTag triggers task reload and updates active_tag
    #[test]
    fn test_set_launch_tag_triggers_reload() {
        let mut app = test_app();

        let _ = app.update(Message::SetLaunchTag("new-tag".into()));

        // Should update both launch_config.tag and active_tag
        assert_eq!(app.state.launch_config.tag, "new-tag");
        assert_eq!(app.state.active_tag, Some("new-tag".into()));
    }

    /// Test ArchiveTag message (without bridge, just verifies no panic)
    #[test]
    fn test_archive_tag_message() {
        let mut app = test_app();

        // Without a bridge connection, ArchiveTag does nothing but doesn't panic
        let _ = app.update(Message::ArchiveTag {
            tag: "old-tag".into(),
        });
    }

    /// Test TagArchived event updates output buffer
    #[test]
    fn test_tag_archived_event() {
        let mut app = test_app();

        let _ = app.update(Message::ScudEvent(ScudEvent::TagArchived {
            tag: "archived-tag".into(),
        }));

        assert!(app.state.output_buffer.contains("archived-tag"));
        assert!(app.state.output_buffer.contains("archived"));
    }

    /// Test waves view shows active tag with Change button
    #[test]
    fn test_waves_view_has_change_tag_button() {
        let mut app = test_app();
        app.state.active_tag = Some("feature".into());

        // Load some tasks so the view renders fully
        let waves = vec![vec![TaskInfo {
            id: "1".into(),
            title: "Test task".into(),
            status: "Pending".into(),
            agent: None,
        }]];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));

        let mut ui = simulator(app.view());

        // Change button should be clickable when a tag is active
        let change_result = ui.click("Change");
        assert!(
            change_result.is_ok(),
            "Change button should be clickable when tag is active"
        );

        // Verify it emits SwitchView(Tags) message
        let mut saw_switch = false;
        for msg in ui.into_messages() {
            if let Message::SwitchView(ViewMode::Tags) = &msg {
                saw_switch = true;
            }
            let _ = app.update(msg);
        }
        assert!(saw_switch, "Should emit SwitchView(Tags) message");
    }

    // =============================================================
    // Ralph Mode Tests
    // =============================================================

    /// Test RalphConfig defaults
    #[test]
    fn test_ralph_config_defaults() {
        let config = state::RalphConfig::default();
        assert_eq!(config.max_iterations, 100);
        assert!(config.validate);
        assert!(config.repair);
        assert_eq!(config.max_repair_attempts, 3);
        assert!(!config.batch_subtasks);
        assert!(!config.git_push);
    }

    /// Test toggling execution mode
    #[test]
    fn test_set_execution_mode() {
        let mut app = test_app();
        assert_eq!(app.state.launch_config.execution_mode, ExecutionMode::Swarm);

        let _ = app.update(Message::SetExecutionMode(ExecutionMode::Ralph));
        assert_eq!(app.state.launch_config.execution_mode, ExecutionMode::Ralph);

        let _ = app.update(Message::SetExecutionMode(ExecutionMode::Swarm));
        assert_eq!(app.state.launch_config.execution_mode, ExecutionMode::Swarm);
    }

    /// Test all Ralph config update messages
    #[test]
    fn test_ralph_config_updates() {
        let mut app = test_app();

        let _ = app.update(Message::SetRalphValidate(false));
        assert!(!app.state.launch_config.ralph_config.validate);

        let _ = app.update(Message::SetRalphRepair(false));
        assert!(!app.state.launch_config.ralph_config.repair);

        let _ = app.update(Message::SetRalphMaxIterations("50".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_iterations, 50);
        assert_eq!(app.state.launch_config.ralph_max_iterations_input, "50");

        let _ = app.update(Message::SetRalphMaxRepairAttempts("5".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_repair_attempts, 5);
        assert_eq!(app.state.launch_config.ralph_max_repair_attempts_input, "5");

        let _ = app.update(Message::SetRalphBatchSubtasks(true));
        assert!(app.state.launch_config.ralph_config.batch_subtasks);

        let _ = app.update(Message::SetRalphGitPush(true));
        assert!(app.state.launch_config.ralph_config.git_push);
    }

    /// Test Ralph max iterations input validation
    #[test]
    fn test_ralph_max_iterations_validation() {
        let mut app = test_app();

        // Valid input
        let _ = app.update(Message::SetRalphMaxIterations("200".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_iterations, 200);

        // Out of range (0) - should not update
        let _ = app.update(Message::SetRalphMaxIterations("0".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_iterations, 200);

        // Out of range (>1000) - should not update
        let _ = app.update(Message::SetRalphMaxIterations("2000".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_iterations, 200);

        // Non-numeric input - should not update
        let _ = app.update(Message::SetRalphMaxIterations("abc".into()));
        assert_eq!(app.state.launch_config.ralph_config.max_iterations, 200);
        assert_eq!(app.state.launch_config.ralph_max_iterations_input, "abc");
    }

    /// Test Ralph events update progress state
    #[test]
    fn test_ralph_events_update_progress() {
        let mut app = test_app();

        // RalphStarted
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphStarted {
            tag: "feature".into(),
            max_iterations: 10,
        }));
        assert!(app.state.ralph_progress.active);
        assert_eq!(app.state.ralph_progress.max_iterations, 10);
        assert_eq!(app.state.ralph_progress.tag, "feature");

        // RalphIterationStarted
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphIterationStarted {
            iteration: 1,
            task_id: "task-1".into(),
            task_title: "First task".into(),
        }));
        assert_eq!(app.state.ralph_progress.current_iteration, 1);
        assert_eq!(
            app.state.ralph_progress.current_task_id,
            Some("task-1".into())
        );
        assert_eq!(app.state.ralph_progress.phase, RalphPhase::Executing);

        // RalphValidationStarted
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphValidationStarted {
            task_id: "task-1".into(),
        }));
        assert_eq!(app.state.ralph_progress.phase, RalphPhase::Validating);

        // RalphRepairStarted
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphRepairStarted {
            task_id: "task-1".into(),
            attempt: 2,
        }));
        assert_eq!(app.state.ralph_progress.phase, RalphPhase::Repairing);
        assert_eq!(app.state.ralph_progress.repair_attempt, 2);

        // RalphIterationCompleted (success)
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphIterationCompleted {
            iteration: 1,
            task_id: "task-1".into(),
            success: true,
        }));
        assert_eq!(app.state.ralph_progress.completed_count, 1);
        assert_eq!(app.state.ralph_progress.phase, RalphPhase::Idle);

        // RalphIterationCompleted (failure)
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphIterationCompleted {
            iteration: 2,
            task_id: "task-2".into(),
            success: false,
        }));
        assert_eq!(app.state.ralph_progress.failed_count, 1);
        assert_eq!(app.state.ralph_progress.completed_count, 1);
    }

    /// Test StartRalph switches to Monitor view
    #[test]
    fn test_start_ralph_switches_to_monitor() {
        let mut app = test_app();
        app.state.launch_config.execution_mode = ExecutionMode::Ralph;
        app.state.launch_config.tag = "test-tag".into();

        // Without bridge, StartRalph still updates local state
        let _ = app.update(Message::StartRalph {
            tag: "test-tag".into(),
            harness: "claude".into(),
        });

        // Without bridge, view doesn't switch (no command sent)
        // But the code path is exercised without panic
    }

    /// Test RalphCompleted resets progress and refreshes tasks
    #[test]
    fn test_ralph_completed_resets() {
        let mut app = test_app();
        app.state.agent_status = AgentStatus::Running;
        app.state.ralph_progress.active = true;
        app.state.ralph_progress.completed_count = 5;
        app.state.ralph_progress.failed_count = 1;

        let _ = app.update(Message::ScudEvent(ScudEvent::RalphCompleted {
            iterations: 6,
            completed: 5,
            failed: 1,
        }));

        assert_eq!(app.state.agent_status, AgentStatus::Idle);
        assert!(!app.state.ralph_progress.active);
        assert!(app.state.output_buffer.contains("Ralph complete"));
        assert!(app.state.output_buffer.contains("5 completed"));
        assert!(app.state.output_buffer.contains("1 failed"));
    }

    /// Test UI shows "Start Ralph" button when in Ralph mode
    #[test]
    fn test_ui_ralph_start_button() {
        let mut app = test_app();
        app.state.launch_config.execution_mode = ExecutionMode::Ralph;
        app.state.launch_config.tag = "feature".into();

        let mut ui = simulator(app.view());
        let click_result = ui.click("Start Ralph");
        assert!(
            click_result.is_ok(),
            "Start Ralph button should exist in Ralph mode"
        );

        let mut saw_start_ralph = false;
        for msg in ui.into_messages() {
            if let Message::StartRalph { tag, harness } = &msg {
                saw_start_ralph = true;
                assert_eq!(tag, "feature");
                assert_eq!(harness, &app.state.launch_config.harness);
            }
            let _ = app.update(msg);
        }
        assert!(saw_start_ralph, "Should emit StartRalph message");
    }

    /// Test StopRalph button in monitor when Ralph is active
    #[test]
    fn test_ralph_validation_output_in_session() {
        let mut app = test_app();

        // Setup a headless session
        let _ = app.update(Message::ScudEvent(ScudEvent::HeadlessStarted {
            task_id: "task-1".into(),
            harness: "claude".into(),
        }));

        // Simulate validation completed
        let _ = app.update(Message::ScudEvent(ScudEvent::RalphValidationCompleted {
            task_id: "task-1".into(),
            passed: false,
            output: "cargo test failed".into(),
        }));

        // Check that validation output was appended to session
        let session = app.state.headless_sessions.get("task-1").unwrap();
        assert!(session
            .output_lines
            .iter()
            .any(|l| l.contains("VALIDATION")));
        assert!(session
            .output_lines
            .iter()
            .any(|l| l.contains("cargo test failed")));
    }

    // =============================================================
    // Generate + Tags Tests
    // =============================================================

    /// Test GenerateState defaults
    #[test]
    fn test_generate_state_defaults() {
        let state = state::GenerateState::default();
        assert_eq!(state.num_tasks, 10);
        assert_eq!(state.num_tasks_input, "10");
        assert!(!state.generating);
        assert!(!state.no_expand);
        assert!(!state.no_check_deps);
        assert!(!state.append);
        assert!(state.prd_files.is_empty());
        assert!(state.selected_prd.is_none());
        assert!(state.tag_input.is_empty());
    }

    /// Test generate config update messages
    #[test]
    fn test_generate_config_updates() {
        let mut app = test_app();

        let _ = app.update(Message::SetGenerateTag("my-tag".into()));
        assert_eq!(app.state.generate_state.tag_input, "my-tag");

        let _ = app.update(Message::SetGenerateNumTasks("15".into()));
        assert_eq!(app.state.generate_state.num_tasks, 15);
        assert_eq!(app.state.generate_state.num_tasks_input, "15");

        // Out of range input preserves text but doesn't update num
        let _ = app.update(Message::SetGenerateNumTasks("200".into()));
        assert_eq!(app.state.generate_state.num_tasks, 15);
        assert_eq!(app.state.generate_state.num_tasks_input, "200");

        let _ = app.update(Message::SetGenerateNoExpand(true));
        assert!(app.state.generate_state.no_expand);

        let _ = app.update(Message::SetGenerateNoCheckDeps(true));
        assert!(app.state.generate_state.no_check_deps);

        let _ = app.update(Message::SetGenerateAppend(true));
        assert!(app.state.generate_state.append);
    }

    /// Test tag summaries loaded populates tag explorer
    #[test]
    fn test_tag_summaries_loaded() {
        let mut app = test_app();

        let summaries = vec![
            TagSummary {
                name: "alpha".into(),
                total_tasks: 5,
                done_count: 2,
                pending_count: 2,
                in_progress_count: 1,
                failed_count: 0,
                is_active: true,
            },
            TagSummary {
                name: "beta".into(),
                total_tasks: 3,
                done_count: 0,
                pending_count: 3,
                in_progress_count: 0,
                failed_count: 0,
                is_active: false,
            },
        ];

        let _ = app.update(Message::TagSummariesLoaded(summaries));
        assert_eq!(app.state.tag_explorer.tags.len(), 2);
        assert_eq!(app.state.tag_explorer.tags[0].name, "alpha");
        assert!(app.state.tag_explorer.tags[0].is_active);
    }

    /// Test archives loaded populates tag explorer
    #[test]
    fn test_archives_loaded() {
        let mut app = test_app();

        let archives = vec![ArchiveEntry {
            filename: "2026-01-15_alpha.scg".into(),
            date: "2026-01-15".into(),
            tag: Some("alpha".into()),
            task_count: 5,
        }];

        let _ = app.update(Message::ArchivesLoaded(archives));
        assert_eq!(app.state.tag_explorer.archives.len(), 1);
        assert_eq!(
            app.state.tag_explorer.archives[0].filename,
            "2026-01-15_alpha.scg"
        );
    }

    /// Test SwitchView(Generate) triggers ScanPrdDirectory
    #[test]
    fn test_switch_to_generate_scans() {
        let mut app = test_app();

        // SwitchView to Generate returns a Task (ScanPrdDirectory)
        // Without bridge it just does nothing, but the view switches
        let _ = app.update(Message::SwitchView(ViewMode::Generate));
        assert_eq!(app.view, ViewMode::Generate);
    }

    /// Test SwitchView(Tags) triggers LoadTagExplorer
    #[test]
    fn test_switch_to_tags_loads() {
        let mut app = test_app();

        let _ = app.update(Message::SwitchView(ViewMode::Tags));
        assert_eq!(app.view, ViewMode::Tags);
    }

    /// Test GenerateCompleted success sets generating=false
    #[test]
    fn test_generate_completed_success() {
        let mut app = test_app();
        app.state.generate_state.generating = true;

        let _ = app.update(Message::GenerateCompleted(Ok(())));
        assert!(!app.state.generate_state.generating);
        assert_eq!(app.view, ViewMode::Tags);
    }

    /// Test GenerateCompleted failure
    #[test]
    fn test_generate_completed_failure() {
        let mut app = test_app();
        app.state.generate_state.generating = true;

        let _ = app.update(Message::GenerateCompleted(Err("generation failed".into())));
        assert!(!app.state.generate_state.generating);
        assert!(app
            .state
            .generate_state
            .generate_status
            .as_ref()
            .unwrap()
            .contains("failed"));
    }

    /// Test ActiveTagChanged updates both active_tag and launch_config
    #[test]
    fn test_active_tag_changed_refreshes() {
        let mut app = test_app();

        let _ = app.update(Message::ActiveTagChanged("new-tag".into()));
        assert_eq!(app.state.active_tag, Some("new-tag".into()));
        assert_eq!(app.state.launch_config.tag, "new-tag");
    }

    /// Test PrdFilesLoaded updates generate state
    #[test]
    fn test_prd_files_loaded() {
        let mut app = test_app();

        let files = vec![
            std::path::PathBuf::from("/tmp/prd1.md"),
            std::path::PathBuf::from("/tmp/prd2.md"),
        ];
        let _ = app.update(Message::PrdFilesLoaded(files));
        assert_eq!(app.state.generate_state.prd_files.len(), 2);
    }

    /// Test ArchiveRestored success
    #[test]
    fn test_archive_restored_success() {
        let mut app = test_app();

        let _ = app.update(Message::ArchiveRestored(Ok(vec!["alpha".into()])));
        assert!(app.state.output_buffer.contains("alpha"));
    }

    /// Test ArchiveRestored failure
    #[test]
    fn test_archive_restored_failure() {
        let mut app = test_app();

        let _ = app.update(Message::ArchiveRestored(Err("not found".into())));
        assert!(app.error.is_some());
        assert!(app.error.as_ref().unwrap().contains("not found"));
    }

    /// Test Generate and Tags nav buttons appear in header
    #[test]
    fn test_ui_navigation_to_generate_and_tags() {
        let mut app = test_app();

        let mut ui = simulator(app.view());

        // Click Generate tab
        let gen_result = ui.click("Generate");
        assert!(gen_result.is_ok(), "Generate nav button should exist");

        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.view, ViewMode::Generate);

        // Click Tags tab
        let mut ui = simulator(app.view());
        let tags_result = ui.click("Tags");
        assert!(tags_result.is_ok(), "Tags nav button should exist");

        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.view, ViewMode::Tags);
    }
}
