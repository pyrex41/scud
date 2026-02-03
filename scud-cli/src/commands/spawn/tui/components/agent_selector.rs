//! Agent selector component for TUI
//!
//! Provides a UI widget for selecting and switching between agents.
//! Supports keyboard navigation, status display, and quick selection.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, StatefulWidget, Widget},
};

use super::super::theme::*;

/// Agent display information for the selector
#[derive(Debug, Clone, PartialEq)]
pub struct AgentInfo {
    /// Unique identifier for the agent
    pub id: String,
    /// Display name or task ID
    pub name: String,
    /// Current status of the agent
    pub status: AgentDisplayStatus,
    /// Task title if available
    pub task_title: Option<String>,
    /// Tag/phase the agent belongs to
    pub tag: Option<String>,
}

impl AgentInfo {
    /// Create a new agent info entry
    pub fn new(id: impl Into<String>, name: impl Into<String>, status: AgentDisplayStatus) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status,
            task_title: None,
            tag: None,
        }
    }

    /// Set task title
    pub fn with_task_title(mut self, title: impl Into<String>) -> Self {
        self.task_title = Some(title.into());
        self
    }

    /// Set tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// Display status for agents (simplified view)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentDisplayStatus {
    /// Agent is starting up
    Starting,
    /// Agent is actively running
    #[default]
    Running,
    /// Agent has completed its task
    Completed,
    /// Agent has failed or errored
    Failed,
    /// Agent is paused/idle
    Idle,
}

impl AgentDisplayStatus {
    /// Get display icon and color for the status
    pub fn icon_and_color(&self) -> (&'static str, ratatui::style::Color) {
        match self {
            Self::Starting => ("◐", STATUS_STARTING),
            Self::Running => ("●", STATUS_RUNNING),
            Self::Completed => ("✓", STATUS_COMPLETED),
            Self::Failed => ("✗", STATUS_FAILED),
            Self::Idle => ("○", TEXT_MUTED),
        }
    }

    /// Get a short status label
    pub fn label(&self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Completed => "Done",
            Self::Failed => "Failed",
            Self::Idle => "Idle",
        }
    }
}

/// State for the agent selector widget
#[derive(Debug, Default)]
pub struct AgentSelectorState {
    /// Currently selected index
    pub selected: usize,
    /// Scroll offset for long lists
    pub offset: usize,
    /// Filter to show only certain statuses (None = show all)
    pub status_filter: Option<AgentDisplayStatus>,
}

impl AgentSelectorState {
    /// Create new state with given selection
    pub fn new(selected: usize) -> Self {
        Self {
            selected,
            offset: 0,
            status_filter: None,
        }
    }

    /// Move selection up
    pub fn previous(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected = if self.selected > 0 {
            self.selected - 1
        } else {
            total - 1
        };
    }

    /// Move selection down
    pub fn next(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected = (self.selected + 1) % total;
    }

    /// Set status filter
    pub fn filter_by_status(&mut self, status: Option<AgentDisplayStatus>) {
        self.status_filter = status;
        self.selected = 0;
        self.offset = 0;
    }

    /// Adjust scroll offset to keep selection visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible_height {
            self.offset = self.selected.saturating_sub(visible_height - 1);
        }
    }
}

/// Agent selector widget for TUI
///
/// Displays a list of agents with their status and allows selection.
/// Supports filtering by status and keyboard navigation.
pub struct AgentSelector<'a> {
    /// Available agents
    agents: &'a [AgentInfo],
    /// Whether this widget is focused
    focused: bool,
    /// Title for the selector
    title: Option<String>,
    /// Show compact view (less details)
    compact: bool,
}

impl<'a> AgentSelector<'a> {
    /// Create a new agent selector with the given agents
    pub fn new(agents: &'a [AgentInfo]) -> Self {
        Self {
            agents,
            focused: false,
            title: None,
            compact: false,
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set custom title (auto-generates if None)
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Use compact rendering mode
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Filter agents based on state filter
    fn filtered_agents(&self, state: &AgentSelectorState) -> Vec<(usize, &'a AgentInfo)> {
        self.agents
            .iter()
            .enumerate()
            .filter(|(_, a)| match state.status_filter {
                Some(status) => a.status == status,
                None => true,
            })
            .collect()
    }

    /// Generate title with agent counts
    fn generate_title(&self, state: &AgentSelectorState) -> String {
        let total = self.agents.len();
        let running = self
            .agents
            .iter()
            .filter(|a| a.status == AgentDisplayStatus::Running)
            .count();

        if let Some(ref custom) = self.title {
            custom.clone()
        } else if let Some(filter) = state.status_filter {
            let count = self.filtered_agents(state).len();
            format!(" Agents ({} {}) ", count, filter.label().to_lowercase())
        } else {
            format!(" Agents ({} running / {} total) ", running, total)
        }
    }
}

impl StatefulWidget for AgentSelector<'_> {
    type State = AgentSelectorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_color = if self.focused {
            BORDER_ACTIVE
        } else {
            BORDER_DEFAULT
        };
        let title_color = if self.focused { ACCENT } else { TEXT_MUTED };

        let title = self.generate_title(state);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(title).fg(title_color))
            .style(Style::default().bg(BG_SECONDARY))
            .padding(Padding::new(1, 1, 0, 0));

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        // Get filtered agents
        let filtered = self.filtered_agents(state);

        // Clamp selection to valid range
        if !filtered.is_empty() && state.selected >= filtered.len() {
            state.selected = filtered.len() - 1;
        }

        // Adjust scroll to keep selection visible
        state.adjust_scroll(visible_height);

        // Render block
        Widget::render(block, area, buf);

        if filtered.is_empty() {
            let msg = if state.status_filter.is_some() {
                "No agents match filter"
            } else {
                "No agents spawned"
            };
            let line = Line::from(Span::styled(msg, Style::default().fg(TEXT_MUTED)));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        // Render items
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(visible_height)
            .map(|(display_idx, (_, agent))| {
                let is_selected = display_idx == state.selected && self.focused;
                let (icon, icon_color) = agent.status.icon_and_color();
                let prefix = if is_selected { "▸ " } else { "  " };

                let line = if self.compact {
                    // Compact: just icon + name
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(ACCENT)),
                        Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
                        Span::styled(
                            &agent.name,
                            Style::default()
                                .fg(if is_selected { ACCENT } else { TEXT_PRIMARY })
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                    ])
                } else {
                    // Full: icon + name + title
                    let title = agent
                        .task_title
                        .as_ref()
                        .map(|t| {
                            // Truncate long titles
                            if t.len() > 30 {
                                format!("{}...", &t[..27])
                            } else {
                                t.clone()
                            }
                        })
                        .unwrap_or_default();

                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(ACCENT)),
                        Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
                        Span::styled(format!("{}: ", agent.name), Style::default().fg(TEXT_MUTED)),
                        Span::styled(
                            title,
                            Style::default()
                                .fg(if is_selected { ACCENT } else { TEXT_PRIMARY })
                                .add_modifier(if is_selected {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                    ])
                };

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        Widget::render(list, inner, buf);
    }
}

/// Compact single-agent display widget
pub struct AgentBadge<'a> {
    /// Agent to display
    agent: &'a AgentInfo,
}

impl<'a> AgentBadge<'a> {
    /// Create a new agent badge
    pub fn new(agent: &'a AgentInfo) -> Self {
        Self { agent }
    }
}

impl Widget for AgentBadge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 5 || area.height < 1 {
            return;
        }

        let (icon, icon_color) = self.agent.status.icon_and_color();
        let text = format!("{} {}", icon, self.agent.name);

        let line = Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
            Span::styled(&self.agent.name, Style::default().fg(TEXT_PRIMARY)),
        ]);

        buf.set_line(area.x, area.y, &line, text.len() as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_info_creation() {
        let agent = AgentInfo::new("1", "task-1", AgentDisplayStatus::Running)
            .with_task_title("Implement feature")
            .with_tag("sprint-1");

        assert_eq!(agent.id, "1");
        assert_eq!(agent.name, "task-1");
        assert_eq!(agent.status, AgentDisplayStatus::Running);
        assert_eq!(agent.task_title, Some("Implement feature".to_string()));
        assert_eq!(agent.tag, Some("sprint-1".to_string()));
    }

    #[test]
    fn test_status_icon_and_color() {
        assert_eq!(AgentDisplayStatus::Running.icon_and_color().0, "●");
        assert_eq!(AgentDisplayStatus::Completed.icon_and_color().0, "✓");
        assert_eq!(AgentDisplayStatus::Failed.icon_and_color().0, "✗");
    }

    #[test]
    fn test_selector_state_navigation() {
        let mut state = AgentSelectorState::new(0);

        state.next(5);
        assert_eq!(state.selected, 1);

        state.previous(5);
        assert_eq!(state.selected, 0);

        state.previous(5); // Wraps around
        assert_eq!(state.selected, 4);
    }

    #[test]
    fn test_status_filter() {
        let mut state = AgentSelectorState::new(2);

        state.filter_by_status(Some(AgentDisplayStatus::Running));
        assert_eq!(state.selected, 0); // Reset on filter change
        assert_eq!(state.status_filter, Some(AgentDisplayStatus::Running));

        state.filter_by_status(None);
        assert!(state.status_filter.is_none());
    }
}
