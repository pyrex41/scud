//! Application state types

use std::collections::HashMap;
use std::fmt;

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Paused,
}

/// Execution mode: Swarm (parallel waves) or Ralph (sequential with backpressure)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Swarm,
    Ralph,
}

impl fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionMode::Swarm => write!(f, "Swarm"),
            ExecutionMode::Ralph => write!(f, "Ralph"),
        }
    }
}

/// Configuration for Ralph mode execution
#[derive(Debug, Clone)]
pub struct RalphConfig {
    pub max_iterations: usize,
    pub validate: bool,
    pub repair: bool,
    pub max_repair_attempts: usize,
    pub batch_subtasks: bool,
    pub git_push: bool,
}

impl Default for RalphConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            validate: true,
            repair: true,
            max_repair_attempts: 3,
            batch_subtasks: false,
            git_push: false,
        }
    }
}

/// Current phase within a Ralph iteration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RalphPhase {
    #[default]
    Idle,
    Executing,
    Validating,
    Repairing,
}

impl fmt::Display for RalphPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RalphPhase::Idle => write!(f, "Idle"),
            RalphPhase::Executing => write!(f, "Executing"),
            RalphPhase::Validating => write!(f, "Validating"),
            RalphPhase::Repairing => write!(f, "Repairing"),
        }
    }
}

/// Progress tracking for Ralph mode execution
#[derive(Debug, Clone, Default)]
pub struct RalphProgress {
    pub active: bool,
    pub current_iteration: usize,
    pub max_iterations: usize,
    pub tag: String,
    pub current_task_id: Option<String>,
    pub current_task_title: Option<String>,
    pub phase: RalphPhase,
    pub repair_attempt: usize,
    pub completed_count: usize,
    pub failed_count: usize,
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
    /// Wave number this task belongs to (0-indexed)
    pub wave: Option<usize>,
    /// Partial line buffer for accumulating streaming text
    pub partial_line: String,
    /// Session ID for resuming in interactive mode
    pub session_id: Option<String>,
}

/// Swarm progress tracking for the monitor view
#[derive(Debug, Clone, Default)]
pub struct SwarmProgress {
    /// Total number of waves in this swarm
    pub total_waves: usize,
    /// Current wave being executed (0-indexed)
    pub current_wave: usize,
    /// Whether the swarm is actively running
    pub active: bool,
    /// Tag being executed
    pub tag: String,
}

/// Task information for display
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub status: String,
    /// Agent type assigned to this task (e.g., "builder", "reviewer")
    pub agent: Option<String>,
}

/// Swarm execution defaults loaded from config
#[derive(Debug, Clone)]
pub struct SwarmDefaults {
    /// Harness to use for swarm execution (e.g., "rho", "claude")
    pub harness: String,
    /// Number of agents to run in parallel per wave
    pub round_size: usize,
    /// Default tag to use when none is specified
    pub default_tag: String,
}

impl Default for SwarmDefaults {
    fn default() -> Self {
        Self {
            harness: "rho".to_string(),
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
    /// Harness to use for launch (e.g., "rho", "claude")
    pub harness: String,
    /// Model override to use (empty means harness default)
    pub model: String,
    /// Parallel agent count per wave
    pub round_size: usize,
    /// Text input value for round size (kept in sync with round_size)
    pub round_size_input: String,
    /// Tag to execute
    pub tag: String,
    /// Optional agent type override (maps to .scud/agents/)
    pub agent_type: Option<String>,
    /// Whether to override per-task agent configs with launch-level harness/model/agent
    pub override_agents: bool,
    /// Execution mode: Swarm or Ralph
    pub execution_mode: ExecutionMode,
    /// Ralph-specific configuration
    pub ralph_config: RalphConfig,
    /// Text input for max iterations (kept in sync with ralph_config.max_iterations)
    pub ralph_max_iterations_input: String,
    /// Text input for max repair attempts (kept in sync with ralph_config.max_repair_attempts)
    pub ralph_max_repair_attempts_input: String,
}

impl LaunchConfig {
    /// Build a launch config from swarm defaults
    pub fn from_defaults(defaults: &SwarmDefaults) -> Self {
        let ralph_config = RalphConfig::default();
        let max_iter = ralph_config.max_iterations.to_string();
        let max_repair = ralph_config.max_repair_attempts.to_string();
        Self {
            harness: defaults.harness.clone(),
            model: String::new(),
            round_size: defaults.round_size,
            round_size_input: defaults.round_size.to_string(),
            tag: defaults.default_tag.clone(),
            agent_type: None,
            override_agents: false,
            execution_mode: ExecutionMode::default(),
            ralph_max_iterations_input: max_iter,
            ralph_max_repair_attempts_input: max_repair,
            ralph_config,
        }
    }
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self::from_defaults(&SwarmDefaults::default())
    }
}

/// State for the PRD viewer and generate pipeline
#[derive(Debug, Clone)]
pub struct GenerateState {
    /// Discovered PRD files from scanning
    pub prd_files: Vec<std::path::PathBuf>,
    pub selected_prd: Option<std::path::PathBuf>,
    pub prd_content: Option<String>,
    pub tag_input: String,
    pub num_tasks_input: String,
    pub num_tasks: u32,
    pub generating: bool,
    pub generate_status: Option<String>,
    pub no_expand: bool,
    pub no_check_deps: bool,
    pub append: bool,
    /// Streaming output lines from generate subprocess
    pub generate_output_lines: Vec<String>,
}

impl Default for GenerateState {
    fn default() -> Self {
        Self {
            prd_files: Vec::new(),
            selected_prd: None,
            prd_content: None,
            tag_input: String::new(),
            num_tasks_input: "10".to_string(),
            num_tasks: 10,
            generating: false,
            generate_status: None,
            no_expand: false,
            no_check_deps: false,
            append: false,
            generate_output_lines: Vec::new(),
        }
    }
}

/// Tag summary for the tag explorer
#[derive(Debug, Clone)]
pub struct TagSummary {
    pub name: String,
    pub total_tasks: usize,
    pub done_count: usize,
    pub pending_count: usize,
    pub in_progress_count: usize,
    pub failed_count: usize,
    pub is_active: bool,
}

/// Archive entry for the tag explorer
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub filename: String,
    pub date: String,
    pub tag: Option<String>,
    pub task_count: usize,
}

/// State for the tag explorer
#[derive(Debug, Clone, Default)]
pub struct TagExplorerState {
    pub tags: Vec<TagSummary>,
    pub archives: Vec<ArchiveEntry>,
}

/// State for LLM configuration in settings
#[derive(Debug, Clone, Default)]
pub struct LlmConfigState {
    pub provider: String,
    pub model: String,
    pub smart_provider: String,
    pub smart_model: String,
    pub fast_provider: String,
    pub fast_model: String,
    pub max_tokens_input: String,
    pub loaded: bool,
    pub dirty: bool,
    pub status: Option<String>,
}


/// State for backpressure configuration in settings
#[derive(Debug, Clone)]
pub struct BackpressureState {
    /// The commands currently configured/detected
    pub commands: Vec<String>,
    /// Whether to stop on first failure
    pub stop_on_failure: bool,
    /// Timeout per command in seconds
    pub timeout_secs: u64,
    /// Text input for timeout
    pub timeout_input: String,
    /// Whether the config was auto-detected (vs explicitly configured)
    pub is_auto_detected: bool,
    /// Whether the config has been loaded yet
    pub loaded: bool,
    /// Text input for adding a new command
    pub new_command_input: String,
    /// Whether the config has unsaved changes
    pub dirty: bool,
    /// Status message (e.g., "Saved" or error)
    pub status: Option<String>,
}

impl Default for BackpressureState {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            stop_on_failure: true,
            timeout_secs: 300,
            timeout_input: "300".to_string(),
            is_auto_detected: true,
            loaded: false,
            new_command_input: String::new(),
            dirty: false,
            status: None,
        }
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
    /// Swarm execution defaults (stored for reference; launch_config is actively used)
    #[allow(dead_code)]
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
    /// Swarm progress for the monitor view
    pub swarm_progress: SwarmProgress,
    /// Ralph progress for the monitor view
    pub ralph_progress: RalphProgress,
    /// Agent configurations (name -> config)
    pub agent_configs: HashMap<String, AgentConfig>,
    /// Application settings
    pub settings: AppSettings,
    /// Current working directory (project root)
    pub working_directory: std::path::PathBuf,
    /// Available models per harness (harness_name -> list of model names)
    pub available_models: HashMap<String, Vec<String>>,
    /// Generate tab state
    pub generate_state: GenerateState,
    /// Tag explorer state
    pub tag_explorer: TagExplorerState,
    /// Backpressure configuration state
    pub backpressure: BackpressureState,
    /// LLM configuration state
    pub llm_config: LlmConfigState,
    /// Whether the project has been initialized (has .scud/ directory)
    pub is_initialized: bool,
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
                "rho".to_string(),
                "claude".to_string(),
                "opencode".to_string(),
                "cursor".to_string(),
            ],
            available_tags: Vec::new(),
            available_agents: Vec::new(),
            headless_sessions: HashMap::new(),
            monitor_selected_task: None,
            swarm_progress: SwarmProgress::default(),
            ralph_progress: RalphProgress::default(),
            agent_configs: HashMap::new(),
            settings: AppSettings::default(),
            working_directory: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".")),
            available_models: {
                let mut models = HashMap::new();
                // Claude models are hardcoded
                models.insert(
                    "claude".to_string(),
                    vec![
                        "sonnet".to_string(),
                        "opus".to_string(),
                        "haiku".to_string(),
                    ],
                );
                models.insert(
                    "rho".to_string(),
                    vec![
                        "claude-sonnet".to_string(),
                        "claude-opus".to_string(),
                        "claude-haiku".to_string(),
                        "xai/grok-code-fast-1".to_string(),
                        "xai/grok-4-1-fast".to_string(),
                    ],
                );
                // Others will be populated on startup
                models.insert("opencode".to_string(), Vec::new());
                models.insert("cursor".to_string(), Vec::new());
                models
            },
            generate_state: GenerateState::default(),
            tag_explorer: TagExplorerState::default(),
            backpressure: BackpressureState::default(),
            llm_config: LlmConfigState::default(),
            is_initialized: true,
        }
    }
}

/// Configuration for a single agent type (loaded from .scud/agents/*.toml)
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub harness: String,
    pub model: String,
    /// Whether this config has been modified from disk
    pub dirty: bool,
}

impl AgentConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            harness: "rho".to_string(),
            model: "claude-sonnet".to_string(),
            dirty: false,
        }
    }
}

/// Application settings (stored in user preferences)
#[derive(Debug, Clone)]
pub struct AppSettings {
    /// Terminal application to use for attach (macOS: "Terminal", "iTerm", "Warp")
    pub terminal_app: String,
    /// Recent working directories for quick switching
    pub recent_projects: Vec<std::path::PathBuf>,
    /// Maximum number of recent projects to remember
    pub max_recent_projects: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            terminal_app: "Terminal".to_string(),
            recent_projects: Vec::new(),
            max_recent_projects: 10,
        }
    }
}
