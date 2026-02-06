//! Task waves view
//!
//! Displays tasks organized in parallel execution waves,
//! with launch controls at the bottom.

use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Alignment, Background, Border, Element, Length};
use std::collections::HashMap;

use crate::state::{AgentStatus, ExecutionMode, LaunchConfig, TaskInfo};
use crate::theme;
use crate::views::ViewMode;
use crate::Message;

/// Render the waves view showing tasks organized by execution wave,
/// with launch controls below.
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    waves: &'a [Vec<TaskInfo>],
    active_tag: &'a Option<String>,
    agent_status: AgentStatus,
    launch_config: &LaunchConfig,
    available_harnesses: &'a [String],
    available_agents: &'a [String],
    available_models: &'a HashMap<String, Vec<String>>,
    is_initialized: bool,
) -> Element<'a, Message> {
    // Show init banner if project is not initialized
    if !is_initialized {
        return build_init_banner();
    }

    let mut waves_column = Column::new().spacing(theme::SPACING_LG);

    if waves.is_empty() {
        waves_column = waves_column.push(
            text("No tasks loaded. Click Refresh to load tasks.")
                .style(theme::secondary_text())
                .size(theme::font_size::BODY),
        );
    } else {
        for (wave_idx, wave) in waves.iter().enumerate() {
            let wave_header = text(format!("Wave {}", wave_idx + 1))
                .size(theme::font_size::HEADING)
                .style(theme::heading_text());

            let mut task_column = Column::new().spacing(theme::SPACING_SM);
            for task in wave {
                let mut task_actions = row![].spacing(theme::SPACING_SM);

                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions.push(
                        button("Spawn")
                            .on_press(Message::SpawnTask {
                                task_id: task.id.clone(),
                            })
                            .style(theme::primary_button()),
                    );
                }

                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions.push(
                        button("Done")
                            .on_press(Message::MarkTaskComplete {
                                task_id: task.id.clone(),
                            })
                            .style(theme::ghost_button()),
                    );
                }
                if task.status != "Blocked" && task.status != "blocked" {
                    task_actions = task_actions.push(
                        button("Block")
                            .on_press(Message::MarkTaskBlocked {
                                task_id: task.id.clone(),
                            })
                            .style(theme::ghost_button()),
                    );
                }

                // Status badge
                let status_color = match task.status.to_lowercase().as_str() {
                    "done" => theme::SUCCESS,
                    "in-progress" | "inprogress" => theme::ACCENT,
                    "blocked" | "failed" => theme::ERROR,
                    _ => theme::text::MUTED,
                };

                let status_badge =
                    container(text(&task.status).size(theme::font_size::CAPTION).style(
                        move |_| iced::widget::text::Style {
                            color: Some(status_color),
                        },
                    ))
                    .padding([2, 8])
                    .style(theme::status_badge(status_color));

                let task_id_text = text(&task.id)
                    .size(theme::font_size::CAPTION)
                    .style(theme::muted_text())
                    .width(Length::Fixed(60.0));

                // Agent type badge (if assigned)
                let agent_badge = if let Some(ref agent) = task.agent {
                    container(
                        text(format!("@{}", agent))
                            .size(theme::font_size::CAPTION)
                            .style(|_| iced::widget::text::Style {
                                color: Some(theme::ACCENT),
                            }),
                    )
                    .padding([2, 6])
                    .style(|_| container::Style {
                        background: Some(Background::Color(iced::Color {
                            a: 0.1,
                            ..theme::ACCENT
                        })),
                        border: Border {
                            color: iced::Color {
                                a: 0.2,
                                ..theme::ACCENT
                            },
                            width: 1.0,
                            radius: theme::RADIUS_SMALL.into(),
                        },
                        ..Default::default()
                    })
                } else {
                    container(text("").size(theme::font_size::CAPTION))
                };

                let task_content = row![
                    task_id_text,
                    text(&task.title)
                        .size(theme::font_size::BODY)
                        .width(Length::Fill),
                    agent_badge,
                    status_badge,
                    task_actions,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center);

                let task_card = container(task_content)
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(Background::Color(theme::surface::RAISED)),
                        border: Border {
                            color: theme::border::SUBTLE,
                            width: 1.0,
                            radius: theme::RADIUS.into(),
                        },
                        ..Default::default()
                    });

                task_column = task_column.push(task_card);
            }

            waves_column = waves_column.push(wave_header).push(task_column);
        }
    }

    // Header row with controls
    let refresh_button = button("Refresh")
        .on_press(Message::RefreshTasks)
        .style(theme::ghost_button());

    let tag_display: Element<'_, Message> = if let Some(ref tag) = active_tag {
        row![
            text(tag)
                .size(theme::font_size::BODY)
                .style(theme::heading_text()),
            button("Change")
                .on_press(Message::SwitchView(ViewMode::Tags))
                .style(theme::ghost_button()),
        ]
        .spacing(theme::SPACING_SM)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            text("No tag selected").style(theme::muted_text()),
            button("Select Tag")
                .on_press(Message::SwitchView(ViewMode::Tags))
                .style(theme::primary_button()),
        ]
        .spacing(theme::SPACING_SM)
        .align_y(Alignment::Center)
        .into()
    };

    let header_row = row![refresh_button, tag_display]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center);

    // Launch controls at the bottom
    let launch_controls = build_launch_controls(
        agent_status,
        launch_config,
        available_harnesses,
        available_agents,
        available_models,
    );

    column![
        header_row,
        scrollable(waves_column).height(Length::Fill),
        launch_controls
    ]
    .spacing(theme::SPACING_MD)
    .into()
}

/// Build compact launch controls panel
fn build_launch_controls<'a>(
    agent_status: AgentStatus,
    launch_config: &LaunchConfig,
    available_harnesses: &'a [String],
    available_agents: &'a [String],
    available_models: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let label_width = Length::Fixed(120.0);

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
    .width(Length::Fixed(120.0));

    // Start / stop buttons
    let action_button: Element<'_, Message> = match agent_status {
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
            button(label)
                .on_press(start_msg)
                .style(theme::primary_button())
                .into()
        }
        AgentStatus::Running => {
            let stop_msg = match launch_config.execution_mode {
                ExecutionMode::Ralph => Message::StopRalph,
                ExecutionMode::Swarm => Message::StopSwarm,
            };
            row![
                button("Pause")
                    .on_press(Message::PauseAgent)
                    .style(theme::ghost_button()),
                button("Stop")
                    .on_press(stop_msg)
                    .style(theme::danger_button()),
            ]
            .spacing(theme::SPACING_SM)
            .into()
        }
        AgentStatus::Paused => row![
            button("Resume")
                .on_press(Message::ResumeAgent)
                .style(theme::primary_button()),
            button("Cancel")
                .on_press(Message::CancelAgent)
                .style(theme::ghost_button()),
        ]
        .spacing(theme::SPACING_SM)
        .into(),
    };

    // First row: mode + action buttons
    let mut controls_section = Column::new().spacing(theme::SPACING_SM);

    controls_section = controls_section.push(
        row![
            text("Mode")
                .width(label_width)
                .style(theme::secondary_text()),
            mode_picker,
            action_button,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
    );

    // Mode-specific config (inline)
    match launch_config.execution_mode {
        ExecutionMode::Swarm => {
            let round_input = text_input("1-30", &launch_config.round_size_input)
                .on_input(Message::SetRoundSizeInput)
                .width(Length::Fixed(60.0));

            controls_section = controls_section.push(
                row![
                    text("Round size")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    round_input,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );
        }
        ExecutionMode::Ralph => {
            let ralph = &launch_config.ralph_config;

            let iter_input = text_input("1-1000", &launch_config.ralph_max_iterations_input)
                .on_input(Message::SetRalphMaxIterations)
                .width(Length::Fixed(60.0));

            let validate_btn =
                toggle_button(ralph.validate, Message::SetRalphValidate(!ralph.validate));

            controls_section = controls_section.push(
                row![
                    text("Max iterations")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    iter_input,
                    text("Validate")
                        .width(Length::Fixed(60.0))
                        .style(theme::secondary_text()),
                    validate_btn,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );

            if ralph.validate {
                let repair_btn =
                    toggle_button(ralph.repair, Message::SetRalphRepair(!ralph.repair));

                let mut repair_row = row![
                    text("Repair")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    repair_btn,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center);

                if ralph.repair {
                    let repair_input =
                        text_input("1-10", &launch_config.ralph_max_repair_attempts_input)
                            .on_input(Message::SetRalphMaxRepairAttempts)
                            .width(Length::Fixed(60.0));
                    repair_row = repair_row
                        .push(
                            text("Max attempts")
                                .width(Length::Fixed(90.0))
                                .style(theme::secondary_text()),
                        )
                        .push(repair_input);
                }

                controls_section = controls_section.push(repair_row);
            }

            let batch_btn = toggle_button(
                ralph.batch_subtasks,
                Message::SetRalphBatchSubtasks(!ralph.batch_subtasks),
            );
            let push_btn = toggle_button(ralph.git_push, Message::SetRalphGitPush(!ralph.git_push));

            controls_section = controls_section.push(
                row![
                    text("Batch subtasks")
                        .width(label_width)
                        .style(theme::secondary_text()),
                    batch_btn,
                    text("Git push")
                        .width(Length::Fixed(70.0))
                        .style(theme::secondary_text()),
                    push_btn,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center),
            );
        }
    }

    // Override agents toggle + conditional overrides
    let override_btn = toggle_button(
        launch_config.override_agents,
        Message::SetOverrideAgents(!launch_config.override_agents),
    );
    controls_section = controls_section.push(
        row![
            text("Override agents")
                .width(label_width)
                .style(theme::secondary_text()),
            override_btn,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
    );

    if launch_config.override_agents {
        let harness_options = with_selected_option(available_harnesses, &launch_config.harness);
        let harness_picker = pick_list(
            harness_options,
            Some(launch_config.harness.clone()),
            Message::SetHarness,
        )
        .width(Length::Fixed(140.0));

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
        .width(Length::Fixed(140.0));

        let (agent_options, agent_selected) =
            agent_picker_options(available_agents, launch_config.agent_type.as_ref());
        let agent_picker = pick_list(agent_options, Some(agent_selected), |selected| {
            if selected == "Default" {
                Message::SetAgentType(None)
            } else {
                Message::SetAgentType(Some(selected))
            }
        })
        .width(Length::Fixed(140.0));

        controls_section = controls_section.push(
            row![
                text("Harness")
                    .width(label_width)
                    .style(theme::secondary_text()),
                harness_picker,
                text("Model")
                    .width(Length::Fixed(50.0))
                    .style(theme::secondary_text()),
                model_picker,
                text("Agent")
                    .width(Length::Fixed(50.0))
                    .style(theme::secondary_text()),
                agent_picker,
            ]
            .spacing(theme::SPACING_SM)
            .align_y(Alignment::Center),
        );
    }

    container(controls_section)
        .padding(theme::SPACING_MD)
        .width(Length::Fill)
        .style(theme::panel_container())
        .into()
}

/// Toggle button helper: styled ON/OFF button
fn toggle_button(value: bool, on_press: Message) -> Element<'static, Message> {
    let label = if value { "ON" } else { "OFF" };
    if value {
        button(text(label).size(theme::font_size::SMALL))
            .on_press(on_press)
            .style(theme::primary_button())
            .into()
    } else {
        button(text(label).size(theme::font_size::SMALL))
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

/// Build initialization banner for uninitialized projects
fn build_init_banner() -> Element<'static, Message> {
    let heading = text("Project not initialized")
        .size(theme::font_size::HEADING)
        .style(theme::heading_text());

    let description = text(
        "This directory doesn't have a .scud/ configuration. Initialize to get started.",
    )
    .size(theme::font_size::BODY)
    .style(theme::secondary_text());

    let init_button = button(
        text("Initialize Project").size(theme::font_size::BODY),
    )
    .on_press(Message::InitProject)
    .style(theme::primary_button());

    container(
        column![heading, description, init_button]
            .spacing(theme::SPACING_LG)
            .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
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
