//! Launch configuration view
//!
//! Displays swarm/ralph launch configuration and controls.

use iced::widget::{button, column, container, pick_list, row, text, text_input, Column};
use iced::{Alignment, Element, Length};
use std::collections::HashMap;

use crate::state::{AgentStatus, ExecutionMode, LaunchConfig};
use crate::theme;
use crate::Message;

/// Render the launch view with config panel and controls
pub fn view<'a>(
    agent_status: AgentStatus,
    current_task: &Option<String>,
    launch_config: &LaunchConfig,
    available_harnesses: &'a [String],
    available_tags: &'a [String],
    available_agents: &'a [String],
    available_models: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let status_color = match agent_status {
        AgentStatus::Running => theme::SUCCESS,
        AgentStatus::Paused => theme::WARNING,
        AgentStatus::Idle => theme::text::MUTED,
    };

    let status_text = container(
        text(format!("Status: {:?}", agent_status))
            .size(theme::font_size::BODY)
            .style(move |_| iced::widget::text::Style {
                color: Some(status_color),
            }),
    )
    .padding([4, 10])
    .style(theme::status_badge(status_color));

    let current_task_display = if let Some(ref task_id) = current_task {
        text(format!("Current task: {}", task_id))
            .size(theme::font_size::BODY)
            .style(theme::secondary_text())
    } else {
        text("No task selected")
            .size(theme::font_size::BODY)
            .style(theme::muted_text())
    };

    let status_section = column![status_text, current_task_display].spacing(theme::SPACING_SM);

    // Shared config: harness, model, tag, agent override
    let harness_options = with_selected_option(available_harnesses, &launch_config.harness);
    let harness_picker = pick_list(
        harness_options,
        Some(launch_config.harness.clone()),
        Message::SetHarness,
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

    // Get models for the currently selected harness
    let models_for_harness = available_models
        .get(&launch_config.harness)
        .cloned()
        .unwrap_or_default();

    let model_options: Vec<String> = {
        let mut opts = vec!["(default)".to_string()];
        opts.extend(models_for_harness.clone());
        if !launch_config.model.is_empty()
            && launch_config.model != "(default)"
            && !models_for_harness.contains(&launch_config.model)
        {
            opts.push(launch_config.model.clone());
        }
        opts
    };

    let selected_model = if launch_config.model.is_empty() {
        "(default)".to_string()
    } else {
        launch_config.model.clone()
    };

    let model_picker = pick_list(model_options, Some(selected_model), |m| {
        if m == "(default)" {
            Message::SetModel(String::new())
        } else {
            Message::SetModel(m)
        }
    })
    .width(Length::Fill);

    // Mode picker
    let mode_options = vec!["Swarm".to_string(), "Ralph".to_string()];
    let mode_selected = launch_config.execution_mode.to_string();
    let mode_picker = pick_list(mode_options, Some(mode_selected), |m| {
        Message::SetExecutionMode(if m == "Ralph" {
            ExecutionMode::Ralph
        } else {
            ExecutionMode::Swarm
        })
    })
    .width(Length::Fill);

    let label_width = Length::Fixed(140.0);

    // Build the config section
    let mut config_section = Column::new().spacing(theme::SPACING_MD);

    config_section = config_section.push(
        text("Launch Configuration")
            .size(theme::font_size::HEADING)
            .style(theme::heading_text()),
    );

    // Mode row
    config_section = config_section.push(
        row![
            text("Mode")
                .width(label_width)
                .style(theme::secondary_text()),
            mode_picker
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
    );

    // Shared rows: harness, model, tag, agent override
    config_section = config_section
        .push(
            row![
                text("Harness")
                    .width(label_width)
                    .style(theme::secondary_text()),
                harness_picker
            ]
            .spacing(theme::SPACING_MD)
            .align_y(Alignment::Center),
        )
        .push(
            row![
                text("Model")
                    .width(label_width)
                    .style(theme::secondary_text()),
                model_picker
            ]
            .spacing(theme::SPACING_MD)
            .align_y(Alignment::Center),
        )
        .push(
            row![
                text("Tag")
                    .width(label_width)
                    .style(theme::secondary_text()),
                tag_picker
            ]
            .spacing(theme::SPACING_MD)
            .align_y(Alignment::Center),
        )
        .push(
            row![
                text("Agent override")
                    .width(label_width)
                    .style(theme::secondary_text()),
                agent_picker
            ]
            .spacing(theme::SPACING_MD)
            .align_y(Alignment::Center),
        )
        .push(
            text("When set, all tasks will use this agent profile instead of their assigned @agent")
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
        );

    // Mode-specific config
    match launch_config.execution_mode {
        ExecutionMode::Swarm => {
            let round_input = text_input("1-30", &launch_config.round_size_input)
                .on_input(Message::SetRoundSizeInput)
                .width(Length::Fixed(80.0));

            config_section = config_section.push(
                row![
                    text("Round size")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    round_input
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );
        }
        ExecutionMode::Ralph => {
            let ralph = &launch_config.ralph_config;

            // Max iterations
            let iter_input =
                text_input("1-1000", &launch_config.ralph_max_iterations_input)
                    .on_input(Message::SetRalphMaxIterations)
                    .width(Length::Fixed(80.0));

            config_section = config_section.push(
                row![
                    text("Max iterations")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    iter_input
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );

            // Validation toggle
            let validate_btn = toggle_button(ralph.validate, Message::SetRalphValidate(!ralph.validate));
            config_section = config_section.push(
                row![
                    text("Validation")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    validate_btn
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );

            // Repair toggle (only shown when validation ON)
            if ralph.validate {
                let repair_btn = toggle_button(ralph.repair, Message::SetRalphRepair(!ralph.repair));
                config_section = config_section.push(
                    row![
                        text("Repair on failure")
                            .width(label_width)
                            .style(theme::secondary_text()),
                        repair_btn
                    ]
                    .spacing(theme::SPACING_MD)
                    .align_y(Alignment::Center),
                );

                // Max repair attempts (only shown when repair ON)
                if ralph.repair {
                    let repair_input = text_input(
                        "1-10",
                        &launch_config.ralph_max_repair_attempts_input,
                    )
                    .on_input(Message::SetRalphMaxRepairAttempts)
                    .width(Length::Fixed(80.0));

                    config_section = config_section.push(
                        row![
                            text("Max repairs")
                                .width(label_width)
                                .style(theme::secondary_text()),
                            repair_input
                        ]
                        .spacing(theme::SPACING_MD)
                        .align_y(Alignment::Center),
                    );
                }
            }

            // Batch subtasks toggle
            let batch_btn =
                toggle_button(ralph.batch_subtasks, Message::SetRalphBatchSubtasks(!ralph.batch_subtasks));
            config_section = config_section.push(
                row![
                    text("Batch subtasks")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    batch_btn
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );

            // Git push toggle
            let push_btn = toggle_button(ralph.git_push, Message::SetRalphGitPush(!ralph.git_push));
            config_section = config_section.push(
                row![
                    text("Git push")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    push_btn
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );
        }
    }

    let config_panel = container(config_section)
        .padding(theme::SPACING_LG)
        .width(Length::Fill)
        .style(theme::panel_container());

    // Controls section
    let mut controls = row![].spacing(theme::SPACING_MD);

    match agent_status {
        AgentStatus::Idle => {
            let start_msg = match launch_config.execution_mode {
                ExecutionMode::Swarm => Message::StartSwarm {
                    tag: launch_config.tag.clone(),
                    harness: launch_config.harness.clone(),
                    round_size: launch_config.round_size,
                },
                ExecutionMode::Ralph => Message::StartRalph {
                    tag: launch_config.tag.clone(),
                    harness: launch_config.harness.clone(),
                },
            };
            let label = match launch_config.execution_mode {
                ExecutionMode::Swarm => "Start Swarm",
                ExecutionMode::Ralph => "Start Ralph",
            };
            controls = controls.push(
                button(label)
                    .on_press(start_msg)
                    .style(theme::primary_button()),
            );
        }
        AgentStatus::Running => {
            let stop_msg = match launch_config.execution_mode {
                ExecutionMode::Ralph => Message::StopRalph,
                ExecutionMode::Swarm => Message::StopSwarm,
            };
            controls = controls
                .push(
                    button("Pause")
                        .on_press(Message::PauseAgent)
                        .style(theme::ghost_button()),
                )
                .push(
                    button("Stop")
                        .on_press(stop_msg)
                        .style(theme::danger_button()),
                )
                .push(
                    button("Cancel")
                        .on_press(Message::CancelAgent)
                        .style(theme::ghost_button()),
                );
        }
        AgentStatus::Paused => {
            controls = controls
                .push(
                    button("Resume")
                        .on_press(Message::ResumeAgent)
                        .style(theme::primary_button()),
                )
                .push(
                    button("Cancel")
                        .on_press(Message::CancelAgent)
                        .style(theme::ghost_button()),
                );
        }
    }

    column![status_section, config_panel, controls]
        .spacing(theme::SPACING_LG)
        .into()
}

/// Toggle button helper: styled ON/OFF button
fn toggle_button(value: bool, on_press: Message) -> Element<'static, Message> {
    let label = if value { "ON" } else { "OFF" };
    if value {
        button(text(label).size(theme::font_size::BODY))
            .on_press(on_press)
            .style(theme::primary_button())
            .into()
    } else {
        button(text(label).size(theme::font_size::BODY))
            .on_press(on_press)
            .style(theme::ghost_button())
            .into()
    }
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
