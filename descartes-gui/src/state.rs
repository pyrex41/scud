//! Application state types

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Paused,
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
            default_tag: config.swarm.default_tag.clone().unwrap_or_else(|| "default".to_string()),
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

/// Main application state
#[derive(Debug, Default)]
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
}
