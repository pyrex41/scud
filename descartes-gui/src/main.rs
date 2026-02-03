//! Descartes GUI - Desktop interface for AI agent orchestration
//!
//! A minimal Iced GUI that wraps v2's simple architecture:
//! - Direct library calls to RalphExecutor
//! - SCUD storage for task/wave management
//! - AgentRegistry for status display

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Subscription, Task, Theme};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as TokioMutex};

// Use scud-core directly for task management
use scud_core::Storage;

mod components;
mod scud_bridge;
mod state;
mod theme;
mod views;

use scud_bridge::{ScudBridge, ScudCommand, ScudEvent};
use state::{AgentStatus, AppState, SwarmDefaults, TaskInfo};
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
    LoadTasksViaScud { tag: Option<String> },
    ComputeWavesViaScud { tag: String },
    MarkTaskComplete { task_id: String },
    MarkTaskBlocked { task_id: String },
    SelectTask(Option<String>),
    SelectTag(Option<String>),
    RefreshTasks,

    // Swarm management
    StartSwarm { tag: String, harness: String, round_size: usize },
    StopSwarm,

    // Agent management (legacy - for single task execution)
    StartAgent(String), // task_id
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

        // Create ScudBridge and get channel handles
        let (bridge, scud_command_tx, scud_event_rx) = ScudBridge::create();

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
                    ..AppState::default()
                },
                scud_command_tx: Some(scud_command_tx),
                scud_event_rx,
                error: None,
            },
            // Use ScudBridge for initial task loading
            Task::done(Message::LoadTasksViaScud { tag: None }),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchView(view) => {
                self.view = view;
                Task::none()
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
                Task::done(Message::LoadTasksViaScud { tag: self.state.active_tag.clone() })
            }

            Message::StartSwarm { tag, harness, round_size } => {
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    self.state.agent_status = AgentStatus::Running;
                    self.state.output_buffer.clear();
                    self.state
                        .output_buffer
                        .push_str(&format!("Starting swarm for tag '{}'...\n", tag));
                    return Task::perform(
                        async move {
                            let _ = tx
                                .send(ScudCommand::StartSwarm {
                                    tag,
                                    harness,
                                    round_size,
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

            Message::StartAgent(task_id) => {
                self.state.agent_status = AgentStatus::Running;
                self.state.current_task = Some(task_id.clone());
                self.state.output_buffer.clear();
                self.state
                    .output_buffer
                    .push_str(&format!("Starting agent for task {}...\n", task_id));

                // Spawn the agent via ScudBridge RunTask command
                if let Some(ref tx) = self.scud_command_tx {
                    let tx = tx.clone();
                    let harness = self.state.swarm_defaults.harness.clone();
                    return Task::perform(
                        async move {
                            let _ = tx.send(ScudCommand::RunTask { task_id, harness }).await;
                        },
                        |_| Message::Tick,
                    );
                }

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
                        self.state.output_buffer.push_str(&format!(
                            "Swarm started for tag '{}' with {} waves\n",
                            tag, total_waves
                        ));
                    }
                    ScudEvent::WaveStarted { wave, tasks } => {
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
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] {}\n", task_id, text));
                    }
                    ScudEvent::TaskCompleted { task_id, success } => {
                        let status = if success { "completed" } else { "failed" };
                        self.state
                            .output_buffer
                            .push_str(&format!("Task {} {}\n", task_id, status));
                    }
                    ScudEvent::ValidationStarted => {
                        self.state
                            .output_buffer
                            .push_str("Validation started...\n");
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
                        let status = if success {
                            "successfully"
                        } else {
                            "with failures"
                        };
                        self.state
                            .output_buffer
                            .push_str(&format!("Swarm completed {}\n", status));
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
                        self.state
                            .output_buffer
                            .push_str(&format!("Headless session started for task {} ({})\n", task_id, harness));
                    }
                    ScudEvent::ToolStart {
                        task_id,
                        tool_name,
                        tool_id: _,
                        input_summary,
                    } => {
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] >> {} {}\n", task_id, tool_name, input_summary));
                    }
                    ScudEvent::ToolResult {
                        task_id,
                        tool_name,
                        tool_id: _,
                        success,
                    } => {
                        let status = if success { "ok" } else { "failed" };
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] << {} {}\n", task_id, tool_name, status));
                    }
                    ScudEvent::SessionAssigned { task_id, session_id } => {
                        self.state
                            .output_buffer
                            .push_str(&format!("[{}] Session assigned: {}\n", task_id, session_id));
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

            Message::Tick => Task::none(),
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

            let header = views::header::view(self.view, self.state.agent_status);
            let content = match self.view {
                ViewMode::Waves => views::waves::view(&self.state.waves, &self.state.active_tag),
                ViewMode::Agents => views::agents::view(
                    self.state.agent_status,
                    &self.state.current_task,
                    &self.state.active_tag,
                    &self.state.swarm_defaults,
                ),
                ViewMode::Output => views::output::view(
                    &self.state.current_task,
                    self.state.agent_status,
                    &self.state.output_buffer,
                ),
            };

            column![error_banner, header, content].spacing(10).into()
        } else {
            let header = views::header::view(self.view, self.state.agent_status);
            let content = match self.view {
                ViewMode::Waves => views::waves::view(&self.state.waves, &self.state.active_tag),
                ViewMode::Agents => views::agents::view(
                    self.state.agent_status,
                    &self.state.current_task,
                    &self.state.active_tag,
                    &self.state.swarm_defaults,
                ),
                ViewMode::Output => views::output::view(
                    &self.state.current_task,
                    self.state.agent_status,
                    &self.state.output_buffer,
                ),
            };

            column![header, content].spacing(10).into()
        };

        container(main_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(20)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Create subscription to receive ScudEvents from the bridge
        let rx = self.scud_event_rx.clone();
        Subscription::run_with(ScudEventReceiver(rx), |ScudEventReceiver(rx)| {
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
        })
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
    let phase = storage
        .load_active_group()
        .map_err(|e| e.to_string())?;

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
                    })
                })
                .collect()
        })
        .collect();

    Ok(waves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_test::simulator;

    /// Create a test instance without the Task
    fn test_app() -> DescartesGui {
        DescartesGui {
            view: ViewMode::Waves,
            state: AppState::default(),
            scud_command_tx: None,
            scud_event_rx: Arc::new(TokioMutex::new(None)),
            error: None,
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

        let _ = app.update(Message::SwitchView(ViewMode::Agents));
        assert_eq!(app.view, ViewMode::Agents);

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
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Pending".into(),
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
    fn test_start_agent() {
        let mut app = test_app();

        let _ = app.update(Message::StartAgent("task-1".into()));
        assert_eq!(app.state.agent_status, AgentStatus::Running);
        assert_eq!(app.state.current_task, Some("task-1".into()));
        assert!(!app.state.output_buffer.is_empty());
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

        // Start an agent first
        let _ = app.update(Message::StartAgent("task-1".into()));
        assert_eq!(app.state.agent_status, AgentStatus::Running);

        // Switch to Agents view
        let _ = app.update(Message::SwitchView(ViewMode::Agents));

        // Render and find the Pause button
        let mut ui = simulator(app.view());
        let pause_result = ui.click("Pause");
        assert!(pause_result.is_ok(), "Pause button should exist when agent is running");

        // Process messages
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Paused);

        // Now Resume button should appear
        let mut ui = simulator(app.view());
        let resume_result = ui.click("Resume");
        assert!(resume_result.is_ok(), "Resume button should exist when agent is paused");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Running);
    }

    /// Test clicking Cancel button stops the agent
    #[test]
    fn test_ui_cancel_agent() {
        let mut app = test_app();

        // Start an agent
        let _ = app.update(Message::StartAgent("task-1".into()));
        let _ = app.update(Message::SwitchView(ViewMode::Agents));

        let mut ui = simulator(app.view());
        let cancel_result = ui.click("Cancel");
        assert!(cancel_result.is_ok(), "Cancel button should exist");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Idle);
    }

    /// Test error banner dismiss interaction
    #[test]
    fn test_ui_error_banner_dismiss() {
        let mut app = test_app();
        app.error = Some("Test error message".to_string());

        let mut ui = simulator(app.view());

        // Error banner should have Dismiss button
        let dismiss_result = ui.click("Dismiss");
        assert!(dismiss_result.is_ok(), "Dismiss button should exist in error banner");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.error.is_none(), "Error should be dismissed");
    }

    /// Test clicking Start button on a task row
    #[test]
    fn test_ui_start_task_from_waves() {
        let mut app = test_app();

        // Load some tasks
        let waves = vec![vec![
            TaskInfo {
                id: "1".into(),
                title: "First task".into(),
                status: "Pending".into(),
            },
            TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Pending".into(),
            },
        ]];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));

        // Render waves view
        let mut ui = simulator(app.view());

        // Click Start button (there are multiple, clicking finds first)
        let start_result = ui.click("Start");
        assert!(start_result.is_ok(), "Start button should exist");

        for message in ui.into_messages() {
            let _ = app.update(message);
        }

        // Agent should be started
        assert_eq!(app.state.agent_status, AgentStatus::Running);
        assert!(app.state.current_task.is_some());
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

        // Start agent and check status updates
        let _ = app.update(Message::StartAgent("task-1".into()));
        let mut ui = simulator(app.view());
        let status_find = ui.find("Status: Running");
        assert!(status_find.is_ok(), "Should show Status: Running when agent runs");
    }

    /// Test Output view displays agent output
    #[test]
    fn test_ui_output_view_content() {
        let mut app = test_app();

        // Add some output
        let _ = app.update(Message::StartAgent("task-1".into()));
        let _ = app.update(Message::AgentOutput("Line 1\n".into()));
        let _ = app.update(Message::AgentOutput("Line 2\n".into()));

        // Verify the model state directly (more reliable than UI text search for long content)
        assert!(app.state.output_buffer.contains("Line 1"), "Output buffer should contain Line 1");
        assert!(app.state.output_buffer.contains("Line 2"), "Output buffer should contain Line 2");

        // Switch to output view and verify it renders without error
        let _ = app.update(Message::SwitchView(ViewMode::Output));
        let ui = simulator(app.view());
        // Just verify it renders - the output is in a scrollable container
        drop(ui);
    }

    // =============================================================
    // Full Loop Headless Test
    // =============================================================

    /// Simulates a complete workflow: load tasks -> start agent -> pause -> resume -> complete
    #[test]
    fn test_full_workflow_headless() {
        let mut app = test_app();

        // Step 1: Simulate loading waves (mimics async WavesLoaded result)
        let waves = vec![
            vec![TaskInfo {
                id: "1".into(),
                title: "Setup environment".into(),
                status: "Pending".into(),
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Build core module".into(),
                status: "Pending".into(),
            }],
        ];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));
        assert_eq!(app.state.waves.len(), 2, "Should have 2 waves loaded");

        // Step 2: Click Start on first task via UI
        let mut ui = simulator(app.view());
        let _ = ui.click("Start");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.state.agent_status, AgentStatus::Running);
        assert_eq!(app.state.current_task, Some("1".into()));

        // Step 3: Navigate to Agents view
        let mut ui = simulator(app.view());
        let _ = ui.click("Agents");
        for msg in ui.into_messages() {
            let _ = app.update(msg);
        }
        assert_eq!(app.view, ViewMode::Agents);

        // Step 4: Pause the agent
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
        assert!(dismiss_result.is_ok(), "Dismiss button should be present in error banner");
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

        // Load tasks and start agent
        let waves = vec![vec![TaskInfo {
            id: "1".into(),
            title: "Failing task".into(),
            status: "Pending".into(),
        }]];
        let _ = app.update(Message::WavesLoaded(Ok(waves)));
        let _ = app.update(Message::StartAgent("1".into()));

        // Agent encounters an error
        let _ = app.update(Message::AgentComplete(Err("Build failed with exit code 1".into())));

        assert_eq!(app.state.agent_status, AgentStatus::Idle);
        assert!(app.state.output_buffer.contains("Agent error"));
        assert!(app.state.output_buffer.contains("Build failed"));
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
            }],
            vec![TaskInfo {
                id: "2".into(),
                title: "Task B".into(),
                status: "Done".into(),
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
        assert!(app.state.output_buffer.is_empty(), "Output should be cleared");
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
        assert!(app.state.output_buffer.is_empty(), "Output should be cleared after clicking Clear");
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
            },
            TaskInfo {
                id: "2".into(),
                title: "Second task".into(),
                status: "Done".into(),
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
                },
                TaskInfo {
                    id: "2".into(),
                    title: "Second task".into(),
                    status: "Pending".into(),
                },
            ],
            vec![
                TaskInfo {
                    id: "3".into(),
                    title: "Third task".into(),
                    status: "Pending".into(),
                },
            ],
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

        let _ = app.update(Message::ScudEvent(ScudEvent::SwarmCompleted { success: true }));

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

    /// Test Start Swarm button appears when idle
    #[test]
    fn test_ui_swarm_controls() {
        let mut app = test_app();
        app.state.active_tag = Some("feature".into());

        // Switch to agents view
        let _ = app.update(Message::SwitchView(ViewMode::Agents));

        let mut ui = simulator(app.view());

        // When idle, Start Swarm button should be present
        let click_result = ui.click("Start Swarm");
        assert!(click_result.is_ok(), "Start Swarm button should exist when idle");

        // Process the message
        for msg in ui.into_messages() {
            if let Message::StartSwarm { tag, .. } = &msg {
                assert_eq!(tag, "feature", "Should use active tag");
            }
            let _ = app.update(msg);
        }
    }
}
