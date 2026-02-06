//! Agent configuration view
//!
//! Configure agent types (builder, tester, etc.) with their harness and model settings.

use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input, Column,
};
use iced::{Alignment, Background, Border, Element, Length};
use std::collections::HashMap;

use crate::state::AgentConfig;
use crate::theme;
use crate::Message;

/// Render the agents configuration view
pub fn view<'a>(
    agent_configs: &'a HashMap<String, AgentConfig>,
    available_harnesses: &'a [String],
    available_models: &'a HashMap<String, Vec<String>>,
    selected_agent: &Option<String>,
) -> Element<'a, Message> {
    let mut agents_list = Column::new().spacing(theme::SPACING_SM);

    // Sort agents alphabetically
    let mut agent_names: Vec<&String> = agent_configs.keys().collect();
    agent_names.sort();

    if agent_names.is_empty() {
        agents_list = agents_list.push(
            text("No agents configured. Agents are loaded from .scud/agents/")
                .style(theme::muted_text())
                .size(theme::font_size::BODY),
        );
    } else {
        for name in &agent_names {
            let config = &agent_configs[*name];
            let is_selected = selected_agent.as_ref() == Some(*name);

            let (bg_color, border_width) = if is_selected {
                (theme::surface::HOVER, 2.0_f32)
            } else {
                (theme::surface::RAISED, 1.0_f32)
            };

            let dirty_indicator = if config.dirty { " *" } else { "" };

            let agent_row = button(
                container(
                    row![column![
                        text(format!("{}{}", config.name, dirty_indicator))
                            .size(theme::font_size::BODY)
                            .style(|_| iced::widget::text::Style {
                                color: Some(theme::text::PRIMARY),
                            }),
                        text(format!("{} / {}", config.harness, config.model))
                            .size(theme::font_size::CAPTION)
                            .style(theme::muted_text()),
                    ]
                    .spacing(2),]
                    .align_y(Alignment::Center),
                )
                .padding(theme::SPACING_MD)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: if is_selected {
                            theme::ACCENT
                        } else {
                            theme::border::SUBTLE
                        },
                        width: border_width,
                        radius: theme::RADIUS.into(),
                    },
                    ..Default::default()
                }),
            )
            .on_press(Message::SelectAgentConfig((*name).clone()))
            .width(Length::Fill)
            .style(|_, _| button::Style {
                background: None,
                ..Default::default()
            });

            agents_list = agents_list.push(agent_row);
        }
    }

    let left_panel = container(
        column![
            text("Agent Types")
                .size(theme::font_size::HEADING)
                .style(theme::heading_text()),
            scrollable(agents_list).height(Length::Fill),
        ]
        .spacing(theme::SPACING_MD),
    )
    .height(Length::Fill)
    .padding(theme::SPACING_MD);

    // Right panel: edit selected agent
    let right_panel = if let Some(ref agent_name) = selected_agent {
        if let Some(config) = agent_configs.get(agent_name) {
            build_agent_editor(config, available_harnesses, available_models)
        } else {
            centered_placeholder("Select an agent to configure")
        }
    } else {
        centered_placeholder("Select an agent to configure")
    };

    row![
        container(left_panel).width(Length::FillPortion(3)),
        container(right_panel).width(Length::FillPortion(7)),
    ]
    .spacing(theme::SPACING_MD)
    .height(Length::Fill)
    .into()
}

/// Build the agent editor panel
fn build_agent_editor<'a>(
    config: &'a AgentConfig,
    available_harnesses: &'a [String],
    available_models: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let name = config.name.clone();

    let harness_picker = pick_list(
        available_harnesses.to_vec(),
        Some(config.harness.clone()),
        move |h| Message::UpdateAgentHarness {
            agent: name.clone(),
            harness: h,
        },
    )
    .width(Length::Fill);

    // Get models for the currently selected harness
    let models_for_harness = available_models
        .get(&config.harness)
        .cloned()
        .unwrap_or_default();

    // Ensure current model is in the list (even if not from the harness's model list)
    let model_options = if models_for_harness.is_empty() {
        vec![config.model.clone()]
    } else if !models_for_harness.contains(&config.model) && !config.model.is_empty() {
        let mut opts = vec![config.model.clone()];
        opts.extend(models_for_harness);
        opts
    } else {
        models_for_harness
    };

    let name_for_model = config.name.clone();
    let model_picker = pick_list(model_options, Some(config.model.clone()), move |m| {
        Message::UpdateAgentModel {
            agent: name_for_model.clone(),
            model: m,
        }
    })
    .width(Length::Fill);

    let name_for_desc = config.name.clone();
    let desc_input = text_input("Description", &config.description)
        .on_input(move |d| Message::UpdateAgentDescription {
            agent: name_for_desc.clone(),
            description: d,
        })
        .width(Length::Fill);

    let label_width = Length::Fixed(100.0);

    let save_btn = if config.dirty {
        button(text("Save").size(theme::font_size::BODY))
            .on_press(Message::SaveAgentConfig(config.name.clone()))
            .style(theme::primary_button())
    } else {
        button(text("Saved").size(theme::font_size::BODY)).style(theme::ghost_button())
    };

    let editor = column![
        text(&config.name)
            .size(theme::font_size::HEADING)
            .style(theme::heading_text()),
        row![
            text("Description")
                .width(label_width)
                .style(theme::secondary_text()),
            desc_input,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
        row![
            text("Harness")
                .width(label_width)
                .style(theme::secondary_text()),
            harness_picker,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
        row![
            text("Model")
                .width(label_width)
                .style(theme::secondary_text()),
            model_picker,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
        row![save_btn].padding([theme::SPACING_MD, 0.0]),
    ]
    .spacing(theme::SPACING_MD);

    container(editor)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::SPACING_LG)
        .style(theme::panel_container())
        .into()
}

/// Create a centered placeholder message with panel styling
fn centered_placeholder(message: &str) -> Element<'_, Message> {
    container(
        text(message)
            .style(theme::muted_text())
            .size(theme::font_size::BODY),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(theme::SPACING_LG)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(theme::panel_container())
    .into()
}
