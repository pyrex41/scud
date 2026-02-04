//! Agent selector component for GUI
//!
//! Provides a widget for selecting and switching between agents.
//! Supports keyboard navigation, status display, and quick selection.

use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Element, Length};

use crate::theme;

/// Agent display information for the selector
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Get display icon for the status
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Starting => "◐",
            Self::Running => "●",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::Idle => "○",
        }
    }

    /// Get display color for the status
    pub fn color(&self) -> iced::Color {
        match self {
            Self::Starting => theme::text::SECONDARY,
            Self::Running => theme::SUCCESS,
            Self::Completed => theme::ACCENT,
            Self::Failed => theme::ERROR,
            Self::Idle => theme::text::MUTED,
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
#[derive(Debug, Clone, Default)]
pub struct AgentSelectorState {
    /// Currently selected agent ID
    pub selected: Option<String>,
    /// Filter to show only certain statuses (None = show all)
    pub status_filter: Option<AgentDisplayStatus>,
}

impl AgentSelectorState {
    /// Create new state with given selection
    pub fn new(selected: Option<String>) -> Self {
        Self {
            selected,
            status_filter: None,
        }
    }

    /// Set status filter
    pub fn filter_by_status(&mut self, status: Option<AgentDisplayStatus>) {
        self.status_filter = status;
        self.selected = None;
    }

    /// Get filtered agents based on state filter
    pub fn filter_agents<'a>(&self, agents: &'a [AgentInfo]) -> Vec<&'a AgentInfo> {
        agents
            .iter()
            .filter(|a| match self.status_filter {
                Some(status) => a.status == status,
                None => true,
            })
            .collect()
    }

    /// Select the next agent in the list
    pub fn select_next(&mut self, agents: &[AgentInfo]) {
        let filtered = self.filter_agents(agents);
        if filtered.is_empty() {
            self.selected = None;
            return;
        }

        let current_idx = self
            .selected
            .as_ref()
            .and_then(|id| filtered.iter().position(|a| &a.id == id));

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % filtered.len(),
            None => 0,
        };

        self.selected = Some(filtered[next_idx].id.clone());
    }

    /// Select the previous agent in the list
    pub fn select_previous(&mut self, agents: &[AgentInfo]) {
        let filtered = self.filter_agents(agents);
        if filtered.is_empty() {
            self.selected = None;
            return;
        }

        let current_idx = self
            .selected
            .as_ref()
            .and_then(|id| filtered.iter().position(|a| &a.id == id));

        let prev_idx = match current_idx {
            Some(idx) => {
                if idx == 0 {
                    filtered.len() - 1
                } else {
                    idx - 1
                }
            }
            None => filtered.len() - 1,
        };

        self.selected = Some(filtered[prev_idx].id.clone());
    }
}

/// Message type for agent selector events
#[derive(Debug, Clone)]
pub enum AgentSelectorMessage {
    /// An agent was selected
    Selected(String),
    /// Filter changed
    FilterChanged(Option<AgentDisplayStatus>),
}

/// Render an agent selector widget
///
/// Returns an Element that displays available agents with their status.
pub fn view<'a, Message>(
    agents: &'a [AgentInfo],
    state: &AgentSelectorState,
    title: Option<&'a str>,
    compact: bool,
    on_select: impl Fn(String) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let filtered = state.filter_agents(agents);

    // Generate title with agent counts
    let total = agents.len();
    let running = agents
        .iter()
        .filter(|a| a.status == AgentDisplayStatus::Running)
        .count();

    let generated_title: String;
    let title_str: &str = if let Some(t) = title {
        t
    } else {
        generated_title = if let Some(filter) = state.status_filter {
            format!(
                "Agents ({} {})",
                filtered.len(),
                filter.label().to_lowercase()
            )
        } else {
            format!("Agents ({} running / {} total)", running, total)
        };
        &generated_title
    };

    // Clone the title for use in the returned element
    let title_owned = title_str.to_string();
    let header = text(title_owned).size(18);

    let mut agents_column = Column::new().spacing(4);

    if filtered.is_empty() {
        let msg = if state.status_filter.is_some() {
            "No agents match filter"
        } else {
            "No agents spawned"
        };
        agents_column = agents_column.push(text(msg).style(|_| text::Style {
            color: Some(theme::text::MUTED),
        }));
    } else {
        for agent in filtered {
            let is_selected = state.selected.as_ref() == Some(&agent.id);

            let icon = text(agent.status.icon()).style(move |_| text::Style {
                color: Some(agent.status.color()),
            });

            let name_text = text(&agent.name).style(move |_| text::Style {
                color: Some(if is_selected {
                    theme::ACCENT
                } else {
                    theme::text::PRIMARY
                }),
            });

            let row_content = if compact {
                // Compact: just icon + name
                row![icon, name_text].spacing(8)
            } else {
                // Full: icon + name + title
                let title_display = agent
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

                let title_text = text(title_display).style(|_| text::Style {
                    color: Some(theme::text::SECONDARY),
                });

                row![icon, name_text, text(": "), title_text].spacing(4)
            };

            let row_container =
                container(row_content)
                    .padding(6)
                    .width(Length::Fill)
                    .style(move |_| container::Style {
                        background: if is_selected {
                            Some(iced::Background::Color(theme::background::SECONDARY))
                        } else {
                            None
                        },
                        border: iced::Border {
                            color: if is_selected {
                                theme::ACCENT
                            } else {
                                iced::Color::TRANSPARENT
                            },
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    });

            // Wrap in button for selection
            let agent_id = agent.id.clone();
            let on_select_clone = on_select.clone();
            let select_button = iced::widget::button(row_container)
                .on_press(on_select_clone(agent_id))
                .padding(0)
                .style(|_, _| iced::widget::button::Style {
                    background: None,
                    border: iced::Border::default(),
                    ..Default::default()
                });

            agents_column = agents_column.push(select_button);
        }
    }

    let content = column![header, scrollable(agents_column).height(Length::Fill)]
        .spacing(10)
        .padding(10);

    container(content)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::PRIMARY)),
            border: iced::Border {
                color: theme::text::MUTED,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Render a single agent badge (compact inline display)
pub fn badge<'a>(agent: &'a AgentInfo) -> Element<'a, ()> {
    let status_color = agent.status.color();
    let icon = text(agent.status.icon()).style(move |_| text::Style {
        color: Some(status_color),
    });

    let name = text(agent.name.clone()).style(|_| text::Style {
        color: Some(theme::text::PRIMARY),
    });

    container(row![icon, name].spacing(4))
        .padding(4)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::SECONDARY)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
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
    fn test_status_icon() {
        assert_eq!(AgentDisplayStatus::Running.icon(), "●");
        assert_eq!(AgentDisplayStatus::Completed.icon(), "✓");
        assert_eq!(AgentDisplayStatus::Failed.icon(), "✗");
    }

    #[test]
    fn test_selector_state_navigation() {
        let agents = vec![
            AgentInfo::new("1", "agent-1", AgentDisplayStatus::Running),
            AgentInfo::new("2", "agent-2", AgentDisplayStatus::Running),
            AgentInfo::new("3", "agent-3", AgentDisplayStatus::Completed),
        ];

        let mut state = AgentSelectorState::new(None);

        state.select_next(&agents);
        assert_eq!(state.selected, Some("1".to_string()));

        state.select_next(&agents);
        assert_eq!(state.selected, Some("2".to_string()));

        state.select_previous(&agents);
        assert_eq!(state.selected, Some("1".to_string()));

        // Wrap around
        state.select_previous(&agents);
        assert_eq!(state.selected, Some("3".to_string()));
    }

    #[test]
    fn test_status_filter() {
        let agents = vec![
            AgentInfo::new("1", "agent-1", AgentDisplayStatus::Running),
            AgentInfo::new("2", "agent-2", AgentDisplayStatus::Running),
            AgentInfo::new("3", "agent-3", AgentDisplayStatus::Completed),
        ];

        let mut state = AgentSelectorState::new(Some("2".to_string()));

        state.filter_by_status(Some(AgentDisplayStatus::Running));
        assert_eq!(state.selected, None); // Reset on filter change
        assert_eq!(state.status_filter, Some(AgentDisplayStatus::Running));

        let filtered = state.filter_agents(&agents);
        assert_eq!(filtered.len(), 2);

        state.filter_by_status(None);
        assert!(state.status_filter.is_none());
    }
}
