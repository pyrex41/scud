//! Application state for TUI monitor
//!
//! Three-panel design:
//! - Top: Waves/Tasks panel showing tasks by execution wave
//! - Middle: Agents panel showing running agents
//! - Bottom: Live terminal output from selected agent
//!
//! Tab switches focus between panels. Space toggles task selection for spawning.

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::commands::spawn::monitor::{load_session, save_session, AgentState, AgentStatus, SpawnSession};
use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

/// View mode for the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    /// Three-panel view: waves + agents + output
    Split,
    /// Fullscreen: just the selected agent's terminal
    Fullscreen,
    /// Input mode: typing a command to send to agent
    Input,
}

/// Which panel is focused
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPanel {
    Waves,
    Agents,
    Output,
}

/// Task state in the waves view
#[derive(Debug, Clone, PartialEq)]
pub enum WaveTaskState {
    /// Ready to spawn (dependencies met, pending)
    Ready,
    /// Currently running (agent spawned)
    Running,
    /// Completed
    Done,
    /// Blocked by dependencies
    Blocked,
    /// In progress but no agent visible
    InProgress,
}

/// Information about a task in a wave
#[derive(Debug, Clone)]
pub struct WaveTask {
    pub id: String,
    pub title: String,
    pub tag: String,
    pub state: WaveTaskState,
    pub complexity: u32,
    pub dependencies: Vec<String>,
}

/// A wave of tasks that can run in parallel
#[derive(Debug, Clone)]
pub struct Wave {
    pub number: usize,
    pub tasks: Vec<WaveTask>,
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

    // === New three-panel state ===
    /// Which panel is currently focused
    pub focused_panel: FocusedPanel,
    /// Execution waves with tasks
    pub waves: Vec<Wave>,
    /// Selected task IDs for batch spawning
    pub selected_tasks: HashSet<String>,
    /// Selected task index within waves panel
    pub wave_task_index: usize,
    /// Scroll offset for waves panel (first visible line)
    pub wave_scroll_offset: usize,
    /// Scroll offset for agents panel (first visible line)
    pub agents_scroll_offset: usize,
    /// Active tag for loading waves
    pub active_tag: Option<String>,
    /// All phases data (cached)
    phases: HashMap<String, Phase>,
}

impl App {
    /// Create new app state
    pub fn new(project_root: Option<PathBuf>, session_name: &str) -> Result<Self> {
        // Load storage to get active tag and phases
        let storage = Storage::new(project_root.clone());
        let active_tag = storage.get_active_group().ok().flatten();
        let phases = storage.load_tasks().unwrap_or_default();

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
            // New fields
            focused_panel: FocusedPanel::Waves,
            waves: Vec::new(),
            selected_tasks: HashSet::new(),
            wave_task_index: 0,
            wave_scroll_offset: 0,
            agents_scroll_offset: 0,
            active_tag,
            phases,
        };
        app.refresh()?;
        app.refresh_waves();
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
            self.refresh_waves();
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
            self.adjust_agents_scroll();
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
            self.adjust_agents_scroll();
            self.reset_scroll();
            self.refresh_live_output();
        }
    }

    /// Adjust agents scroll offset to keep selected agent visible
    /// Assumes roughly 8 visible lines in the agents panel
    pub fn adjust_agents_scroll(&mut self) {
        const VISIBLE_LINES: usize = 8;

        // Scroll up if selected is above visible area
        if self.selected < self.agents_scroll_offset {
            self.agents_scroll_offset = self.selected;
        }
        // Scroll down if selected is below visible area
        else if self.selected >= self.agents_scroll_offset + VISIBLE_LINES {
            self.agents_scroll_offset = self.selected.saturating_sub(VISIBLE_LINES - 1);
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

    // === Wave-related methods ===

    /// Refresh waves data from phases
    pub fn refresh_waves(&mut self) {
        // Reload phases from storage
        let storage = Storage::new(self.project_root.clone());
        self.phases = storage.load_tasks().unwrap_or_default();

        // Get running agent task IDs
        let running_task_ids: HashSet<String> = self
            .agents()
            .iter()
            .filter(|a| a.status == AgentStatus::Running || a.status == AgentStatus::Starting)
            .map(|a| a.task_id.clone())
            .collect();

        // Determine which tag to use
        let tag = self.active_tag.clone().or_else(|| {
            // Try to get tag from session
            self.session.as_ref().map(|s| s.tag.clone())
        });

        let Some(tag) = tag else {
            self.waves = Vec::new();
            return;
        };

        let Some(phase) = self.phases.get(&tag) else {
            self.waves = Vec::new();
            return;
        };

        // Build waves using topological sort
        self.waves = self.compute_waves(phase, &running_task_ids);
    }

    /// Compute execution waves for a phase
    fn compute_waves(&self, phase: &Phase, running_task_ids: &HashSet<String>) -> Vec<Wave> {
        // Collect actionable tasks
        let mut actionable: Vec<&Task> = Vec::new();
        for task in &phase.tasks {
            if task.status == TaskStatus::Done
                || task.status == TaskStatus::Expanded
                || task.status == TaskStatus::Cancelled
            {
                continue;
            }

            // Skip parent tasks that have subtasks - only subtasks should be spawned
            if !task.subtasks.is_empty() {
                continue;
            }

            // If subtask, only include if parent is expanded
            if let Some(ref parent_id) = task.parent_id {
                let parent_expanded = phase
                    .get_task(parent_id)
                    .map(|p| p.is_expanded())
                    .unwrap_or(false);
                if !parent_expanded {
                    continue;
                }
            }

            actionable.push(task);
        }

        if actionable.is_empty() {
            return Vec::new();
        }

        // Build dependency graph
        let task_ids: HashSet<String> = actionable.iter().map(|t| t.id.clone()).collect();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for task in &actionable {
            in_degree.entry(task.id.clone()).or_insert(0);

            for dep in &task.dependencies {
                if task_ids.contains(dep) {
                    // Internal dependency - track in graph
                    *in_degree.entry(task.id.clone()).or_insert(0) += 1;
                    dependents
                        .entry(dep.clone())
                        .or_default()
                        .push(task.id.clone());
                } else {
                    // External dependency - check if satisfied
                    // If not satisfied (e.g., Expanded with incomplete subtasks), block this task
                    if !self.is_dependency_satisfied(dep, phase) {
                        // Mark as blocked by setting very high in_degree
                        *in_degree.entry(task.id.clone()).or_insert(0) += 1000;
                    }
                }
            }
        }

        // Kahn's algorithm with wave tracking
        let mut waves: Vec<Wave> = Vec::new();
        let mut remaining = in_degree.clone();
        let mut wave_number = 1;

        while !remaining.is_empty() {
            let mut ready: Vec<String> = remaining
                .iter()
                .filter(|(_, &deg)| deg == 0)
                .map(|(id, _)| id.clone())
                .collect();

            if ready.is_empty() {
                break; // Circular dependency
            }

            // Sort for stable display order
            ready.sort();

            // Build wave tasks with state
            let mut wave_tasks: Vec<WaveTask> = ready
                .iter()
                .filter_map(|task_id| {
                    actionable.iter().find(|t| &t.id == task_id).map(|task| {
                        let state = if task.status == TaskStatus::Done {
                            WaveTaskState::Done
                        } else if running_task_ids.contains(&task.id) {
                            WaveTaskState::Running
                        } else if task.status == TaskStatus::InProgress {
                            WaveTaskState::InProgress
                        } else if task.status == TaskStatus::Blocked {
                            WaveTaskState::Blocked
                        } else if self.is_task_ready(task, phase) {
                            WaveTaskState::Ready
                        } else {
                            WaveTaskState::Blocked
                        };

                        WaveTask {
                            id: task.id.clone(),
                            title: task.title.clone(),
                            tag: self.active_tag.clone().unwrap_or_default(),
                            state,
                            complexity: task.complexity,
                            dependencies: task.dependencies.clone(),
                        }
                    })
                })
                .collect();

            // Remove ready tasks and update dependents
            for task_id in &ready {
                remaining.remove(task_id);
                if let Some(deps) = dependents.get(task_id) {
                    for dep_id in deps {
                        if let Some(deg) = remaining.get_mut(dep_id) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
            }

            if !wave_tasks.is_empty() {
                // Sort tasks by ID for stable display
                wave_tasks.sort_by(|a, b| a.id.cmp(&b.id));
                waves.push(Wave {
                    number: wave_number,
                    tasks: wave_tasks,
                });
            }
            wave_number += 1;
        }

        waves
    }

    /// Check if a task is ready (dependencies met, pending)
    fn is_task_ready(&self, task: &Task, phase: &Phase) -> bool {
        if task.status != TaskStatus::Pending {
            return false;
        }

        // Check all dependencies are satisfied
        for dep_id in &task.dependencies {
            if !self.is_dependency_satisfied(dep_id, phase) {
                return false;
            }
        }

        true
    }

    /// Check if a dependency is satisfied (Done, or Expanded with all subtasks done)
    fn is_dependency_satisfied(&self, dep_id: &str, phase: &Phase) -> bool {
        let Some(dep) = phase.get_task(dep_id) else {
            return true; // If dep not found, assume external/done
        };

        match dep.status {
            TaskStatus::Done => true,
            TaskStatus::Expanded => {
                // Expanded task is only satisfied if all its subtasks are done
                if dep.subtasks.is_empty() {
                    false // Expanded with no subtasks = not done yet
                } else {
                    dep.subtasks.iter().all(|subtask_id| {
                        phase
                            .get_task(subtask_id)
                            .map(|st| st.status == TaskStatus::Done)
                            .unwrap_or(false)
                    })
                }
            }
            _ => false, // Pending, InProgress, Blocked, Cancelled = not satisfied
        }
    }

    /// Get flat list of all tasks in waves for navigation
    pub fn all_wave_tasks(&self) -> Vec<&WaveTask> {
        self.waves.iter().flat_map(|w| w.tasks.iter()).collect()
    }

    /// Get currently selected wave task
    pub fn selected_wave_task(&self) -> Option<&WaveTask> {
        let all_tasks = self.all_wave_tasks();
        all_tasks.get(self.wave_task_index).copied()
    }

    // === Panel navigation ===

    /// Switch focus to next panel
    pub fn next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Waves => FocusedPanel::Agents,
            FocusedPanel::Agents => FocusedPanel::Output,
            FocusedPanel::Output => FocusedPanel::Waves,
        };
    }

    /// Switch focus to previous panel
    pub fn previous_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Waves => FocusedPanel::Output,
            FocusedPanel::Agents => FocusedPanel::Waves,
            FocusedPanel::Output => FocusedPanel::Agents,
        };
    }

    /// Move selection up in current panel
    pub fn move_up(&mut self) {
        match self.focused_panel {
            FocusedPanel::Waves => {
                if self.wave_task_index > 0 {
                    self.wave_task_index -= 1;
                    self.adjust_wave_scroll();
                }
            }
            FocusedPanel::Agents => self.previous_agent(),
            FocusedPanel::Output => self.scroll_up(1),
        }
    }

    /// Move selection down in current panel
    pub fn move_down(&mut self) {
        match self.focused_panel {
            FocusedPanel::Waves => {
                let max = self.all_wave_tasks().len().saturating_sub(1);
                if self.wave_task_index < max {
                    self.wave_task_index += 1;
                    self.adjust_wave_scroll();
                }
            }
            FocusedPanel::Agents => self.next_agent(),
            FocusedPanel::Output => self.scroll_down(1),
        }
    }

    /// Adjust wave scroll offset to keep selected item visible
    /// Assumes visible height of ~4 items (plus wave headers)
    fn adjust_wave_scroll(&mut self) {
        // Calculate the line index of the current task
        // Each wave has 1 header line, then tasks
        let mut line_idx = 0;
        let mut found = false;
        let mut task_counter = 0;

        for wave in &self.waves {
            line_idx += 1; // wave header
            for _ in &wave.tasks {
                if task_counter == self.wave_task_index {
                    found = true;
                    break;
                }
                line_idx += 1;
                task_counter += 1;
            }
            if found {
                break;
            }
        }

        // Visible height is approximately 4-5 lines in the waves panel
        let visible_height = 4;

        // Scroll to keep current line visible
        if line_idx < self.wave_scroll_offset {
            self.wave_scroll_offset = line_idx;
        } else if line_idx >= self.wave_scroll_offset + visible_height {
            self.wave_scroll_offset = line_idx.saturating_sub(visible_height - 1);
        }
    }

    // === Task selection for spawning ===

    /// Toggle selection of currently highlighted task
    pub fn toggle_task_selection(&mut self) {
        if let Some(task) = self.selected_wave_task() {
            let task_id = task.id.clone();
            if self.selected_tasks.contains(&task_id) {
                self.selected_tasks.remove(&task_id);
            } else {
                // Only allow selecting ready tasks
                if task.state == WaveTaskState::Ready {
                    self.selected_tasks.insert(task_id);
                }
            }
        }
    }

    /// Select all ready tasks in waves
    pub fn select_all_ready(&mut self) {
        for wave in &self.waves {
            for task in &wave.tasks {
                if task.state == WaveTaskState::Ready {
                    self.selected_tasks.insert(task.id.clone());
                }
            }
        }
    }

    /// Clear all task selections
    pub fn clear_selection(&mut self) {
        self.selected_tasks.clear();
    }

    /// Get count of ready tasks
    pub fn ready_task_count(&self) -> usize {
        self.waves
            .iter()
            .flat_map(|w| &w.tasks)
            .filter(|t| t.state == WaveTaskState::Ready)
            .count()
    }

    /// Get count of selected tasks
    pub fn selected_task_count(&self) -> usize {
        self.selected_tasks.len()
    }

    /// Get selected tasks for spawning
    pub fn get_selected_tasks(&self) -> Vec<&WaveTask> {
        self.all_wave_tasks()
            .into_iter()
            .filter(|t| self.selected_tasks.contains(&t.id))
            .collect()
    }

    /// Spawn the selected tasks
    /// Returns the number of tasks successfully spawned
    pub fn spawn_selected_tasks(&mut self) -> Result<usize> {
        use crate::commands::spawn::{agent, terminal};

        let tasks_to_spawn: Vec<(String, String, String)> = self
            .get_selected_tasks()
            .iter()
            .map(|t| (t.id.clone(), t.title.clone(), t.tag.clone()))
            .collect();

        if tasks_to_spawn.is_empty() {
            return Ok(0);
        }

        // Get working directory
        let working_dir = self
            .project_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // Get session info
        let session = match &self.session {
            Some(s) => s,
            None => {
                self.error = Some("No session loaded".to_string());
                return Ok(0);
            }
        };

        let session_name = session.session_name.clone();
        let mut spawned_count = 0;

        // Load phase to get full task data
        let storage = Storage::new(self.project_root.clone());

        for (task_id, task_title, tag) in &tasks_to_spawn {
            // Get full task from phase
            let phase = match self.phases.get(tag) {
                Some(p) => p,
                None => continue,
            };

            let task = match phase.get_task(task_id) {
                Some(t) => t,
                None => continue,
            };

            // Generate prompt
            let prompt = agent::generate_prompt(task, tag);

            // Spawn in tmux (always use tmux from TUI since we're in a session)
            match terminal::spawn_terminal(
                &terminal::Terminal::Tmux,
                task_id,
                &prompt,
                &working_dir,
                &session_name,
            ) {
                Ok(()) => {
                    spawned_count += 1;

                    // Add to session agents
                    if let Some(ref mut session) = self.session {
                        session.add_agent(task_id, task_title, tag);
                    }

                    // Mark task as in-progress
                    if let Ok(mut phase) = storage.load_group(tag) {
                        if let Some(task) = phase.get_task_mut(task_id) {
                            task.set_status(TaskStatus::InProgress);
                            let _ = storage.update_group(tag, &phase);
                        }
                    }
                }
                Err(e) => {
                    self.error = Some(format!("Failed to spawn {}: {}", task_id, e));
                }
            }

            // Small delay between spawns
            if spawned_count < tasks_to_spawn.len() {
                std::thread::sleep(Duration::from_millis(300));
            }
        }

        // Save updated session
        if spawned_count > 0 {
            if let Some(ref session) = self.session {
                let _ = crate::commands::spawn::monitor::save_session(
                    self.project_root.as_ref(),
                    session,
                );
            }

            // Clear selection and refresh
            self.selected_tasks.clear();
            self.refresh()?;
            self.refresh_waves();
        }

        Ok(spawned_count)
    }
}
