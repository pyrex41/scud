//! Application state for TUI monitor
//!
//! Split-view design: Agent list on left, live terminal output on right.
//! Press Enter for fullscreen terminal view, Esc to return.

use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::commands::spawn::monitor::{load_session, save_session, AgentState, AgentStatus, SpawnSession};
use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// View mode for the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    /// Split view: agent list + live output
    Split,
    /// Fullscreen: just the selected agent's terminal
    Fullscreen,
    /// Input mode: typing a command to send to agent
    Input,
}

/// Application state
pub struct App {
    /// Project root directory
    pub project_root: Option<PathBuf>,
    /// Session name being monitored
    pub session_name: String,
    /// Current spawn session data
    pub session: Option<SpawnSession>,
    /// Selected agent index
    pub selected: usize,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Show help overlay
    pub show_help: bool,
    /// Last refresh time
    last_refresh: Instant,
    /// Refresh interval
    refresh_interval: Duration,
    /// Error message to display
    pub error: Option<String>,
    /// Live terminal output for selected agent (cached)
    pub live_output: Vec<String>,
    /// Last output refresh
    last_output_refresh: Instant,
    /// Output refresh interval (faster than status refresh)
    output_refresh_interval: Duration,
    /// Input buffer for sending commands to agent
    pub input_buffer: String,
    /// Scroll offset for terminal output (0 = bottom, positive = scrolled up)
    pub scroll_offset: usize,
    /// Auto-scroll to bottom on new output
    pub auto_scroll: bool,
}

impl App {
    /// Create new app state
    pub fn new(project_root: Option<PathBuf>, session_name: &str) -> Result<Self> {
        let mut app = Self {
            project_root,
            session_name: session_name.to_string(),
            session: None,
            selected: 0,
            view_mode: ViewMode::Split,
            show_help: false,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(2),
            error: None,
            live_output: Vec::new(),
            last_output_refresh: Instant::now(),
            output_refresh_interval: Duration::from_millis(500),
            input_buffer: String::new(),
            scroll_offset: 0,
            auto_scroll: true,
        };
        app.refresh()?;
        app.refresh_live_output();
        Ok(app)
    }

    /// Refresh session data from disk and update agent statuses
    pub fn refresh(&mut self) -> Result<()> {
        match load_session(self.project_root.as_ref(), &self.session_name) {
            Ok(mut session) => {
                // Update agent statuses from tmux and SCUD task status
                self.refresh_agent_statuses(&mut session);

                // Save updated session back to disk
                let _ = save_session(self.project_root.as_ref(), &session);

                self.session = Some(session);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Failed to load session: {}", e));
            }
        }
        self.last_refresh = Instant::now();
        Ok(())
    }

    /// Refresh live output from the selected agent's tmux pane
    pub fn refresh_live_output(&mut self) {
        let agents = self.agents();
        if agents.is_empty() || self.selected >= agents.len() {
            self.live_output = vec!["No agent selected".to_string()];
            return;
        }

        let agent = &agents[self.selected];
        let session = match &self.session {
            Some(s) => s,
            None => {
                self.live_output = vec!["No session loaded".to_string()];
                return;
            }
        };

        // Get tmux windows to find the correct window index
        let tmux_windows = self.get_tmux_windows(&session.session_name);
        let matching_window = tmux_windows.iter().find(|(_, name)| {
            name.starts_with(&agent.window_name) || agent.window_name.starts_with(name)
        });

        let window_target = match matching_window {
            Some((index, _)) => format!("{}:{}", session.session_name, index),
            None => {
                self.live_output = vec![format!("Window '{}' not found", agent.window_name)];
                return;
            }
        };

        // Capture pane content with scrollback
        let output = Command::new("tmux")
            .args([
                "capture-pane",
                "-t", &window_target,
                "-p",           // print to stdout
                "-S", "-100",   // start from 100 lines back
            ])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let content = String::from_utf8_lossy(&out.stdout);
                self.live_output = content
                    .lines()
                    .map(|s| s.to_string())
                    .collect();

                // Remove trailing empty lines
                while self.live_output.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
                    self.live_output.pop();
                }
            }
            Ok(out) => {
                self.live_output = vec![format!(
                    "Error: {}",
                    String::from_utf8_lossy(&out.stderr)
                )];
            }
            Err(e) => {
                self.live_output = vec![format!("tmux error: {}", e)];
            }
        }

        self.last_output_refresh = Instant::now();
    }

    /// Refresh agent statuses by checking tmux windows and SCUD task status
    fn refresh_agent_statuses(&self, session: &mut SpawnSession) {
        let tmux_windows = self.get_tmux_windows(&session.session_name);
        let storage = Storage::new(self.project_root.clone());
        let all_phases = storage.load_tasks().ok();

        for agent in &mut session.agents {
            let window_exists = tmux_windows.iter().any(|(_, name)| {
                name.starts_with(&agent.window_name) || agent.window_name.starts_with(name)
            });

            let task_status = all_phases.as_ref().and_then(|phases| {
                phases.get(&agent.tag).and_then(|phase| {
                    phase.get_task(&agent.task_id).map(|task| task.status.clone())
                })
            });

            agent.status = match (&task_status, window_exists) {
                (Some(TaskStatus::Done), _) => AgentStatus::Completed,
                (Some(TaskStatus::Blocked), _) => AgentStatus::Failed,
                (Some(TaskStatus::InProgress), true) => AgentStatus::Running,
                (Some(TaskStatus::InProgress), false) => AgentStatus::Completed,
                (_, false) => AgentStatus::Completed,
                (_, true) => AgentStatus::Running,
            };
        }
    }

    /// Get list of tmux windows for a session: [(index, name), ...]
    fn get_tmux_windows(&self, session_name: &str) -> Vec<(usize, String)> {
        let output = Command::new("tmux")
            .args(["list-windows", "-t", session_name, "-F", "#{window_index}:#{window_name}"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            parts[0].parse().ok().map(|idx| (idx, parts[1].to_string()))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Periodic tick - refresh data as needed
    pub fn tick(&mut self) -> Result<()> {
        // Refresh session/status data periodically
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.refresh()?;
        }

        // Refresh live output more frequently
        if self.last_output_refresh.elapsed() >= self.output_refresh_interval {
            self.refresh_live_output();
        }

        Ok(())
    }

    /// Get agents list
    pub fn agents(&self) -> &[AgentState] {
        self.session
            .as_ref()
            .map(|s| s.agents.as_slice())
            .unwrap_or(&[])
    }

    /// Select next agent
    pub fn next_agent(&mut self) {
        let len = self.agents().len();
        if len > 0 {
            self.selected = (self.selected + 1) % len;
            self.reset_scroll();
            self.refresh_live_output();
        }
    }

    /// Select previous agent
    pub fn previous_agent(&mut self) {
        let len = self.agents().len();
        if len > 0 {
            self.selected = if self.selected > 0 {
                self.selected - 1
            } else {
                len - 1
            };
            self.reset_scroll();
            self.refresh_live_output();
        }
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Split => ViewMode::Fullscreen,
            ViewMode::Fullscreen => ViewMode::Split,
            ViewMode::Input => ViewMode::Fullscreen,
        };
    }

    /// Exit current mode (go back to split)
    pub fn exit_fullscreen(&mut self) {
        self.view_mode = ViewMode::Split;
        self.input_buffer.clear();
    }

    /// Enter input mode
    pub fn enter_input_mode(&mut self) {
        self.view_mode = ViewMode::Input;
        self.input_buffer.clear();
    }

    /// Add character to input buffer
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Delete last character from input buffer
    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    /// Send the input buffer to the selected agent's tmux pane
    pub fn send_input(&mut self) -> Result<()> {
        if self.input_buffer.is_empty() {
            return Ok(());
        }

        let session = match &self.session {
            Some(s) => s,
            None => {
                self.error = Some("No session loaded".to_string());
                return Ok(());
            }
        };

        let agents = self.agents();
        if agents.is_empty() || self.selected >= agents.len() {
            self.error = Some("No agent selected".to_string());
            return Ok(());
        }

        let agent = &agents[self.selected];

        // Find window index
        let tmux_windows = self.get_tmux_windows(&session.session_name);
        let matching_window = tmux_windows.iter().find(|(_, name)| {
            name.starts_with(&agent.window_name) || agent.window_name.starts_with(name)
        });

        let window_target = match matching_window {
            Some((index, _)) => format!("{}:{}", session.session_name, index),
            None => {
                self.error = Some(format!("Window not found for {}", agent.task_id));
                return Ok(());
            }
        };

        // Send the input to tmux
        let result = Command::new("tmux")
            .args(["send-keys", "-t", &window_target, &self.input_buffer, "Enter"])
            .output();

        match result {
            Ok(out) if out.status.success() => {
                self.error = None;
                self.input_buffer.clear();
                self.view_mode = ViewMode::Fullscreen; // Go to fullscreen to see result
                self.refresh_live_output();
            }
            Ok(out) => {
                self.error = Some(format!(
                    "Send failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Err(e) => {
                self.error = Some(format!("tmux error: {}", e));
            }
        }

        Ok(())
    }

    /// Restart the selected agent (kill and respawn claude)
    pub fn restart_agent(&mut self) -> Result<()> {
        let session = match &self.session {
            Some(s) => s,
            None => return Ok(()),
        };

        let agents = self.agents();
        if agents.is_empty() || self.selected >= agents.len() {
            return Ok(());
        }

        let agent = &agents[self.selected];

        // Find window
        let tmux_windows = self.get_tmux_windows(&session.session_name);
        let matching_window = tmux_windows.iter().find(|(_, name)| {
            name.starts_with(&agent.window_name) || agent.window_name.starts_with(name)
        });

        if let Some((index, _)) = matching_window {
            let target = format!("{}:{}", session.session_name, index);

            // Send Ctrl+C to interrupt current process
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", &target, "C-c"])
                .output();

            // Small delay
            std::thread::sleep(Duration::from_millis(200));

            // Clear and show message
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", &target, "echo 'Agent restarted by user'", "Enter"])
                .output();

            self.error = None;
            self.refresh_live_output();
        }

        Ok(())
    }

    /// Toggle help overlay
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Scroll terminal output up (show older content)
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.live_output.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        self.auto_scroll = false;
    }

    /// Scroll terminal output down (show newer content)
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// Jump to bottom of terminal output
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Reset scroll when switching agents
    fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Get status counts (starting, running, completed, failed)
    pub fn status_counts(&self) -> (usize, usize, usize, usize) {
        let agents = self.agents();
        let starting = agents.iter().filter(|a| a.status == AgentStatus::Starting).count();
        let running = agents.iter().filter(|a| a.status == AgentStatus::Running).count();
        let completed = agents.iter().filter(|a| a.status == AgentStatus::Completed).count();
        let failed = agents.iter().filter(|a| a.status == AgentStatus::Failed).count();
        (starting, running, completed, failed)
    }

    /// Get the selected agent (if any)
    pub fn selected_agent(&self) -> Option<&AgentState> {
        let agents = self.agents();
        if agents.is_empty() || self.selected >= agents.len() {
            None
        } else {
            Some(&agents[self.selected])
        }
    }
}
