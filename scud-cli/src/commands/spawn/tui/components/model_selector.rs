//! Model selector component for TUI
//!
//! Provides a UI widget for selecting AI models/harnesses when spawning agents.
//! Displays available models with their descriptions and allows selection via keyboard.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, StatefulWidget, Widget},
};

use super::super::theme::*;

/// Available AI model/harness options
#[derive(Debug, Clone, PartialEq)]
pub struct ModelOption {
    /// Unique identifier for the model
    pub id: String,
    /// Display name for the model
    pub name: String,
    /// Short description of the model
    pub description: String,
    /// Whether this model is available/enabled
    pub available: bool,
}

impl ModelOption {
    /// Create a new model option
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            available: true,
        }
    }

    /// Set availability status
    pub fn with_available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }
}

/// Default model options for SCUD swarm execution
pub fn default_models() -> Vec<ModelOption> {
    vec![
        ModelOption::new(
            "claude-code",
            "Claude Code",
            "Anthropic's Claude with code tools",
        ),
        ModelOption::new("opencode", "OpenCode", "OpenAI-based code assistant"),
        ModelOption::new("codex", "Codex", "OpenAI Codex for code completion")
            .with_available(false),
    ]
}

/// State for the model selector widget
#[derive(Debug, Default)]
pub struct ModelSelectorState {
    /// Currently selected index
    pub selected: usize,
    /// Scroll offset for long lists
    pub offset: usize,
}

impl ModelSelectorState {
    /// Create new state with given selection
    pub fn new(selected: usize) -> Self {
        Self {
            selected,
            offset: 0,
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

    /// Adjust scroll offset to keep selection visible
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible_height {
            self.offset = self.selected.saturating_sub(visible_height - 1);
        }
    }
}

/// Model selector widget for TUI
///
/// Displays a list of AI models with selection support.
/// Renders as a bordered list with icons indicating availability.
pub struct ModelSelector<'a> {
    /// Available model options
    models: &'a [ModelOption],
    /// Whether this widget is focused
    focused: bool,
    /// Title for the selector
    title: String,
}

impl<'a> ModelSelector<'a> {
    /// Create a new model selector with the given options
    pub fn new(models: &'a [ModelOption]) -> Self {
        Self {
            models,
            focused: false,
            title: "Select Model".to_string(),
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set custom title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

impl StatefulWidget for ModelSelector<'_> {
    type State = ModelSelectorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_color = if self.focused {
            BORDER_ACTIVE
        } else {
            BORDER_DEFAULT
        };
        let title_color = if self.focused { ACCENT } else { TEXT_MUTED };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(format!(" {} ", self.title)).fg(title_color))
            .style(Style::default().bg(BG_SECONDARY))
            .padding(Padding::new(1, 1, 0, 0));

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        // Adjust scroll to keep selection visible
        state.adjust_scroll(visible_height);

        // Render items
        let items: Vec<ListItem> = self
            .models
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(visible_height)
            .map(|(i, model)| {
                let is_selected = i == state.selected && self.focused;

                let icon = if model.available {
                    ("●", SUCCESS)
                } else {
                    ("○", TEXT_MUTED)
                };

                let prefix = if is_selected { "▸ " } else { "  " };

                let line = Line::from(vec![
                    Span::styled(prefix, Style::default().fg(ACCENT)),
                    Span::styled(format!("{} ", icon.0), Style::default().fg(icon.1)),
                    Span::styled(
                        &model.name,
                        Style::default()
                            .fg(if is_selected { ACCENT } else { TEXT_PRIMARY })
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::styled(
                        format!(" - {}", model.description),
                        Style::default().fg(TEXT_MUTED),
                    ),
                ]);

                ListItem::new(line)
            })
            .collect();

        // Render block first, then list inside
        Widget::render(block, area, buf);

        let list = List::new(items);
        Widget::render(list, inner, buf);
    }
}

/// Compact model selector that renders inline (single line)
pub struct ModelSelectorCompact<'a> {
    /// Currently selected model
    selected: &'a ModelOption,
    /// Whether this is focused
    focused: bool,
}

impl<'a> ModelSelectorCompact<'a> {
    /// Create a new compact selector showing the current selection
    pub fn new(selected: &'a ModelOption) -> Self {
        Self {
            selected,
            focused: false,
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for ModelSelectorCompact<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }

        let style = if self.focused {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(TEXT_PRIMARY)
        };

        let icon = if self.selected.available {
            "●"
        } else {
            "○"
        };

        let text = format!(" {} {} ◂", icon, self.selected.name);

        // Render the text
        let span = Span::styled(text, style);
        buf.set_span(area.x, area.y, &span, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_option_creation() {
        let model = ModelOption::new("test", "Test Model", "A test model");
        assert_eq!(model.id, "test");
        assert_eq!(model.name, "Test Model");
        assert!(model.available);
    }

    #[test]
    fn test_model_option_availability() {
        let model = ModelOption::new("test", "Test", "Test").with_available(false);
        assert!(!model.available);
    }

    #[test]
    fn test_selector_state_navigation() {
        let mut state = ModelSelectorState::new(0);

        state.next(3);
        assert_eq!(state.selected, 1);

        state.next(3);
        assert_eq!(state.selected, 2);

        state.next(3); // Wraps around
        assert_eq!(state.selected, 0);

        state.previous(3); // Wraps around
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn test_default_models() {
        let models = default_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "claude-code"));
    }
}
