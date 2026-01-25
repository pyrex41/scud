//! Model selector component for GUI
//!
//! Provides a widget for selecting AI models/harnesses when spawning agents.
//! Displays available models with their descriptions and allows selection via UI.

use iced::widget::{column, container, scrollable, text, Column};
use iced::{Element, Length};

use crate::theme;

/// Available AI model/harness option
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
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
        ModelOption::new("codex", "Codex", "OpenAI Codex for code completion").with_available(false),
    ]
}

/// State for the model selector widget
#[derive(Debug, Clone, Default)]
pub struct ModelSelectorState {
    /// Currently selected model ID
    pub selected: Option<String>,
}

impl ModelSelectorState {
    /// Create new state with given selection
    pub fn new(selected: Option<String>) -> Self {
        Self { selected }
    }

    /// Create new state with the first available model selected
    pub fn with_default_selection(models: &[ModelOption]) -> Self {
        let selected = models
            .iter()
            .find(|m| m.available)
            .map(|m| m.id.clone());
        Self { selected }
    }

    /// Get the currently selected model from the list
    pub fn get_selected<'a>(&self, models: &'a [ModelOption]) -> Option<&'a ModelOption> {
        self.selected
            .as_ref()
            .and_then(|id| models.iter().find(|m| &m.id == id))
    }
}

/// Message type for model selector events
#[derive(Debug, Clone)]
pub enum ModelSelectorMessage {
    /// A model was selected
    Selected(String),
}

/// Render a model selector widget
///
/// Returns an Element that displays available models as clickable buttons
/// with their names and descriptions.
pub fn view<'a, Message>(
    models: &'a [ModelOption],
    state: &ModelSelectorState,
    title: &'a str,
    on_select: impl Fn(String) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let title_text = text(title).size(18);

    let mut options_column = Column::new().spacing(8);

    for model in models {
        let is_selected = state.selected.as_ref() == Some(&model.id);

        // Create status indicator
        let status_text = if model.available { "Available" } else { "Unavailable" };
        let status = text(status_text).style(move |_| text::Style {
            color: Some(if model.available {
                theme::SUCCESS
            } else {
                theme::text::MUTED
            }),
        });

        // Create selection indicator
        let selection_icon = if is_selected { "◉" } else { "○" };

        let name_text = text(format!("{} {}", selection_icon, model.name)).style(move |_| {
            text::Style {
                color: Some(if model.available {
                    if is_selected {
                        theme::ACCENT
                    } else {
                        theme::text::PRIMARY
                    }
                } else {
                    theme::text::MUTED
                }),
            }
        });

        let description_text = text(&model.description)
            .size(14)
            .style(|_| text::Style {
                color: Some(theme::text::SECONDARY),
            });

        let row_content = column![name_text, description_text, status].spacing(2);

        let row_container = container(row_content)
            .padding(8)
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

        // Wrap in button for selection (only if available)
        let row_element: Element<'a, Message> = if model.available {
            let model_id = model.id.clone();
            let on_select_clone = on_select.clone();
            iced::widget::button(row_container)
                .on_press(on_select_clone(model_id))
                .padding(0)
                .style(|_, _| iced::widget::button::Style {
                    background: None,
                    border: iced::Border::default(),
                    ..Default::default()
                })
                .into()
        } else {
            row_container.into()
        };

        options_column = options_column.push(row_element);
    }

    let content = column![title_text, scrollable(options_column).height(Length::Fill)]
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

/// Render a compact model selector (single line display)
pub fn view_compact<'a, Message>(
    selected: Option<&ModelOption>,
    on_click: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let display_text = if let Some(model) = selected {
        let icon = if model.available { "●" } else { "○" };
        format!("{} {} ▾", icon, model.name)
    } else {
        "Select model ▾".to_string()
    };

    iced::widget::button(text(display_text))
        .on_press(on_click)
        .padding(8)
        .into()
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
    fn test_selector_state_default_selection() {
        let models = default_models();
        let state = ModelSelectorState::with_default_selection(&models);
        assert!(state.selected.is_some());
        // Should select the first available model (claude-code)
        assert_eq!(state.selected, Some("claude-code".to_string()));
    }

    #[test]
    fn test_default_models() {
        let models = default_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "claude-code"));
        // Codex should be unavailable
        assert!(models.iter().any(|m| m.id == "codex" && !m.available));
    }
}
