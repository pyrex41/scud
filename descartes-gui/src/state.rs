//! Application state types

use std::collections::HashMap;

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Paused,
}

/// Status of a headless session (mirrors StreamStore's SessionStatus)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadlessSessionStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Per-task headless session info for the GUI monitor
#[derive(Debug, Clone)]
pub struct HeadlessSessionInfo {
    pub task_id: String,
    pub task_title: String,
    pub harness: String,
    pub status: HeadlessSessionStatus,
    pub event_count: usize,
    pub line_count: usize,
    pub output_lines: Vec<String>,
}

/// Task information for display
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// Swarm execution defaults loaded from config
#[derive(Debug, Clone)]
pub struct SwarmDefaults {
    /// Harness to use for swarm execution (e.g., "claude-code", "opencode")
    pub harness: String,
    /// Number of agents to run in parallel per wave
    pub round_size: usize,
    /// Default tag to use when none is specified
    pub default_tag: String,
}

impl Default for SwarmDefaults {
    fn default() -> Self {
        Self {
            harness: "claude-code".to_string(),
            round_size: 3,
            default_tag: "refactor".to_string(),
        }
    }
}

impl SwarmDefaults {
    /// Create SwarmDefaults from scud Config loaded from .scud/config.toml
    pub fn from_scud_config(config: &scud::config::Config) -> Self {
        Self {
            harness: config.swarm.harness.clone(),
            round_size: config.swarm.round_size,
            default_tag: config
                .swarm
                .default_tag
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        }
    }

    /// Load SwarmDefaults from .scud/config.toml
    pub fn load_from_scud() -> Self {
        let config_path = std::path::Path::new(".scud/config.toml");
        match scud::config::Config::load(config_path) {
            Ok(config) => Self::from_scud_config(&config),
            Err(_) => Self::default(),
        }
    }
}

/// Launch configuration selected in the GUI
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Harness to use for launch (e.g., "claude-code", "opencode")
    pub harness: String,
    /// Model override to use (empty means harness default)
    pub model: String,
    /// Parallel agent count per wave
    pub round_size: usize,
    /// Tag to execute
    pub tag: String,
    /// Optional agent type override (maps to .scud/agents/)
    pub agent_type: Option<String>,
}

impl LaunchConfig {
    /// Build a launch config from swarm defaults
    pub fn from_defaults(defaults: &SwarmDefaults) -> Self {
        Self {
            harness: defaults.harness.clone(),
            model: String::new(),
            round_size: defaults.round_size,
            tag: defaults.default_tag.clone(),
            agent_type: None,
        }
    }
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self::from_defaults(&SwarmDefaults::default())
    }
}

/// Main application state
#[derive(Debug)]
pub struct AppState {
    /// Task waves (parallel execution groups)
    pub waves: Vec<Vec<TaskInfo>>,
    /// All loaded tasks (flat list for reference)
    pub tasks: Vec<TaskInfo>,
    /// Currently active tag filter
    pub active_tag: Option<String>,
    /// Current agent status
    pub agent_status: AgentStatus,
    /// Currently executing task
    pub current_task: Option<String>,
    /// Output buffer from agent
    pub output_buffer: String,
    /// Swarm execution defaults
    pub swarm_defaults: SwarmDefaults,
    /// Launch configuration for swarm/task execution
    pub launch_config: LaunchConfig,
    /// Available harnesses to select from
    pub available_harnesses: Vec<String>,
    /// Available tags loaded from storage
    pub available_tags: Vec<String>,
    /// Available agent types loaded from .scud/agents
    pub available_agents: Vec<String>,
    /// Headless sessions for monitoring (task_id -> session info)
    pub headless_sessions: HashMap<String, HeadlessSessionInfo>,
    /// Currently selected task in the monitor view
    pub monitor_selected_task: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        let swarm_defaults = SwarmDefaults::default();
        Self {
            waves: Vec::new(),
            tasks: Vec::new(),
            active_tag: None,
            agent_status: AgentStatus::Idle,
            current_task: None,
            output_buffer: String::new(),
            launch_config: LaunchConfig::from_defaults(&swarm_defaults),
            swarm_defaults,
            available_harnesses: vec![
                "claude-code".to_string(),
                "opencode".to_string(),
                "cursor".to_string(),
            ],
            available_tags: Vec::new(),
            available_agents: Vec::new(),
            headless_sessions: HashMap::new(),
            monitor_selected_task: None,
        }
    }
}
