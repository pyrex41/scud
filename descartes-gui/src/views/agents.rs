//! Agent status view
//!
//! Displays current agent status, configuration, and controls.

use iced::widget::{button, column, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::state::{AgentStatus, LaunchConfig};
use crate::Message;

/// Render the agents view with config panel and controls
pub fn view<'a>(
    agent_status: AgentStatus,
    current_task: &Option<String>,
    launch_config: &LaunchConfig,
    available_harnesses: &'a [String],
    available_tags: &'a [String],
    available_agents: &'a [String],
) -> Element<'a, Message> {
    let status_text = text(format!("Status: {:?}", agent_status));

    let current_task_display = if let Some(ref task_id) = current_task {
        text(format!("Current task: {}", task_id))
    } else {
        text("No task selected")
    };

    let status_section = column![status_text, current_task_display].spacing(4);

    let harness_options = with_selected_option(available_harnesses, &launch_config.harness);
    let harness_picker = pick_list(
        harness_options,
        Some(launch_config.harness.clone()),
        Message::SetHarness,
    )
    .width(Length::Fill);

    let round_options = round_size_options(launch_config.round_size);
    let round_picker = pick_list(
        round_options,
        Some(launch_config.round_size),
        Message::SetRoundSize,
    )
    .width(Length::Fill);

    let tag_options = with_selected_option(available_tags, &launch_config.tag);
    let tag_picker = pick_list(
        tag_options,
        Some(launch_config.tag.clone()),
        Message::SetLaunchTag,
    )
    .width(Length::Fill);

    let (agent_options, agent_selected) =
        agent_picker_options(available_agents, launch_config.agent_type.as_ref());
    let agent_picker = pick_list(agent_options, Some(agent_selected), |selected| {
        if selected == "Default" {
            Message::SetAgentType(None)
        } else {
            Message::SetAgentType(Some(selected))
        }
    })
    .width(Length::Fill);

    let model_input = text_input("Model (optional)", &launch_config.model)
        .on_input(Message::SetModel)
        .width(Length::Fill);

    let label_width = Length::Fixed(140.0);
    let config_section = column![
        text("Launch Configuration").size(18),
        row![text("Harness").width(label_width), harness_picker]
            .spacing(10)
            .align_y(Alignment::Center),
        row![text("Model").width(label_width), model_input]
            .spacing(10)
            .align_y(Alignment::Center),
        row![text("Round size").width(label_width), round_picker]
            .spacing(10)
            .align_y(Alignment::Center),
        row![text("Tag").width(label_width), tag_picker]
            .spacing(10)
            .align_y(Alignment::Center),
        row![text("Agent type").width(label_width), agent_picker]
            .spacing(10)
            .align_y(Alignment::Center),
    ]
    .spacing(10);

    let mut controls = row![].spacing(10);

    match agent_status {
        AgentStatus::Idle => {
            controls = controls.push(button("Start Swarm").on_press(Message::StartSwarm {
                tag: launch_config.tag.clone(),
                harness: launch_config.harness.clone(),
                round_size: launch_config.round_size,
            }));
            controls = controls.push(button("Start Headless").on_press(
                Message::StartSwarmHeadless {
                    tag: launch_config.tag.clone(),
                    harness: launch_config.harness.clone(),
                    round_size: launch_config.round_size,
                },
            ));
        }
        AgentStatus::Running => {
            controls = controls
                .push(button("Pause").on_press(Message::PauseAgent))
                .push(button("Stop Swarm").on_press(Message::StopSwarm))
                .push(button("Cancel").on_press(Message::CancelAgent));
        }
        AgentStatus::Paused => {
            controls = controls
                .push(button("Resume").on_press(Message::ResumeAgent))
                .push(button("Cancel").on_press(Message::CancelAgent));
        }
    }

    column![status_section, config_section, controls]
        .spacing(15)
        .into()
}

fn with_selected_option(options: &[String], selected: &str) -> Vec<String> {
    let mut values: Vec<String> = options.to_vec();
    if values.is_empty() {
        values.push(selected.to_string());
    } else if !values.iter().any(|value| value == selected) {
        values.insert(0, selected.to_string());
    }
    values
}

fn round_size_options(selected: usize) -> Vec<usize> {
    let mut values: Vec<usize> = (1..=10).collect();
    if !values.contains(&selected) {
        values.insert(0, selected);
    }
    values
}

fn agent_picker_options(
    available_agents: &[String],
    selected: Option<&String>,
) -> (Vec<String>, String) {
    let mut values = Vec::with_capacity(available_agents.len() + 1);
    values.push("Default".to_string());
    values.extend(available_agents.iter().cloned());

    let selected_value = selected
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Default".to_string());

    if !values.iter().any(|value| value == &selected_value) {
        values.insert(1, selected_value.clone());
    }

    (values, selected_value)
}
