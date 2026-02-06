//! Headless session monitor view
//!
//! Displays a split-panel layout for monitoring headless swarm sessions:
//! task list on the left, streaming output on the right.

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Background, Border, Element, Length};
use std::collections::HashMap;

use crate::state::{
    AgentStatus, HeadlessSessionInfo, HeadlessSessionStatus, RalphPhase, RalphProgress,
    SwarmProgress,
};
use crate::theme;
use crate::Message;

/// Render the monitor view with session list and streaming output
pub fn view<'a>(
    sessions: &'a HashMap<String, HeadlessSessionInfo>,
    selected_task: &Option<String>,
    swarm_progress: &SwarmProgress,
    agent_status: AgentStatus,
    ralph_progress: &RalphProgress,
) -> Element<'a, Message> {
    if sessions.is_empty() {
        return container(
            text("No headless sessions. Start a swarm or spawn a task to monitor.")
                .style(theme::muted_text())
                .size(theme::font_size::BODY),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    // Sort sessions: by wave (if present), then by active status, then by task_id
    let mut sorted_sessions: Vec<&HeadlessSessionInfo> = sessions.values().collect();
    sorted_sessions.sort_by(|a, b| {
        // First sort by wave (current wave first, earlier waves last)
        let a_wave = a.wave.unwrap_or(usize::MAX);
        let b_wave = b.wave.unwrap_or(usize::MAX);
        // Then by active status within same wave
        let a_active = matches!(
            a.status,
            HeadlessSessionStatus::Starting | HeadlessSessionStatus::Running
        );
        let b_active = matches!(
            b.status,
            HeadlessSessionStatus::Starting | HeadlessSessionStatus::Running
        );
        a_wave
            .cmp(&b_wave)
            .then_with(|| b_active.cmp(&a_active))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });

    // Left panel: scrollable task list with wave grouping
    let left_panel = build_task_list(
        &sorted_sessions,
        selected_task,
        swarm_progress,
        agent_status,
        ralph_progress,
    );

    // Right panel: streaming output
    let right_panel = build_output_panel(sessions, selected_task);

    row![left_panel, right_panel]
        .spacing(theme::SPACING_MD)
        .height(Length::Fill)
        .into()
}

/// Build the left panel with the scrollable task list and clear button
fn build_task_list<'a>(
    sorted_sessions: &[&'a HeadlessSessionInfo],
    selected_task: &Option<String>,
    swarm_progress: &SwarmProgress,
    agent_status: AgentStatus,
    ralph_progress: &RalphProgress,
) -> Element<'a, Message> {
    let mut task_column = Column::new().spacing(theme::SPACING_SM);

    // Add Ralph progress header if active
    if ralph_progress.active {
        let phase_badge = match ralph_progress.phase {
            RalphPhase::Executing => "Executing",
            RalphPhase::Validating => "Validating",
            RalphPhase::Repairing => "Repairing",
            RalphPhase::Idle => "Idle",
        };

        let phase_color = match ralph_progress.phase {
            RalphPhase::Executing => theme::ACCENT,
            RalphPhase::Validating => theme::WARNING,
            RalphPhase::Repairing => theme::ERROR,
            RalphPhase::Idle => theme::text::MUTED,
        };

        let progress_text = format!(
            "Ralph: {} -- Iteration {}/{}",
            ralph_progress.tag, ralph_progress.current_iteration, ralph_progress.max_iterations
        );
        let progress_label = text(progress_text)
            .size(theme::font_size::SMALL)
            .style(|_| iced::widget::text::Style {
                color: Some(theme::ACCENT),
            });

        let mut detail_parts = vec![phase_badge.to_string()];
        if ralph_progress.phase == RalphPhase::Repairing && ralph_progress.repair_attempt > 0 {
            detail_parts.push(format!("#{}", ralph_progress.repair_attempt));
        }
        if let Some(ref title) = ralph_progress.current_task_title {
            detail_parts.push(title.clone());
        }

        let detail_label = text(detail_parts.join(" - "))
            .size(theme::font_size::CAPTION)
            .style(move |_| iced::widget::text::Style {
                color: Some(phase_color),
            });

        let counts_label = text(format!(
            "Completed: {} / Failed: {}",
            ralph_progress.completed_count, ralph_progress.failed_count
        ))
        .size(theme::font_size::CAPTION)
        .style(theme::secondary_text());

        let stop_btn = button(text("Stop Ralph").size(theme::font_size::CAPTION))
            .on_press(Message::StopRalph)
            .style(theme::danger_button());

        let ralph_header = container(
            column![progress_label, detail_label, counts_label, stop_btn]
                .spacing(theme::SPACING_SM),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(theme::surface::OVERLAY)),
            border: Border {
                color: theme::ACCENT,
                width: 1.0,
                radius: theme::RADIUS_SMALL.into(),
            },
            ..Default::default()
        });
        task_column = task_column.push(ralph_header);
    }

    // Add swarm progress header if active
    if swarm_progress.active && swarm_progress.total_waves > 0 {
        let progress_text = format!(
            "Swarm: {} - Wave {}/{}",
            swarm_progress.tag,
            swarm_progress.current_wave + 1,
            swarm_progress.total_waves
        );
        let progress_label = text(progress_text)
            .size(theme::font_size::SMALL)
            .style(|_| iced::widget::text::Style {
                color: Some(theme::ACCENT),
            });

        // Add pause/stop controls when swarm is running
        let controls = if agent_status == AgentStatus::Running {
            row![
                button(text("Pause").size(theme::font_size::CAPTION))
                    .on_press(Message::MonitorPauseSwarm)
                    .style(theme::ghost_button()),
                button(text("Stop").size(theme::font_size::CAPTION))
                    .on_press(Message::MonitorStopSwarm)
                    .style(theme::danger_button()),
            ]
            .spacing(theme::SPACING_SM)
        } else if agent_status == AgentStatus::Paused {
            row![
                button(text("Resume").size(theme::font_size::CAPTION))
                    .on_press(Message::ResumeAgent)
                    .style(theme::primary_button()),
                button(text("Stop").size(theme::font_size::CAPTION))
                    .on_press(Message::MonitorStopSwarm)
                    .style(theme::danger_button()),
            ]
            .spacing(theme::SPACING_SM)
        } else {
            row![]
        };

        let progress_header =
            container(column![progress_label, controls,].spacing(theme::SPACING_SM))
                .padding([4, 8])
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(theme::surface::OVERLAY)),
                    border: Border {
                        color: theme::ACCENT,
                        width: 1.0,
                        radius: theme::RADIUS_SMALL.into(),
                    },
                    ..Default::default()
                });
        task_column = task_column.push(progress_header);
    }

    // Track current wave for grouping
    let mut current_wave: Option<usize> = None;

    for session in sorted_sessions {
        // Add wave separator if wave changed
        if session.wave != current_wave {
            if let Some(wave) = session.wave {
                let wave_label = if swarm_progress.active && wave == swarm_progress.current_wave {
                    format!("Wave {} (running)", wave + 1)
                } else {
                    format!("Wave {}", wave + 1)
                };
                let wave_header = text(wave_label)
                    .size(theme::font_size::CAPTION)
                    .style(theme::secondary_text());
                task_column =
                    task_column.push(container(wave_header).padding([8, 4]).width(Length::Fill));
            }
            current_wave = session.wave;
        }
        let is_selected = selected_task
            .as_ref()
            .map(|s| s == &session.task_id)
            .unwrap_or(false);

        let status_color = match session.status {
            HeadlessSessionStatus::Starting => theme::text::MUTED,
            HeadlessSessionStatus::Running => theme::ACCENT,
            HeadlessSessionStatus::Completed => theme::SUCCESS,
            HeadlessSessionStatus::Failed => theme::ERROR,
        };

        // Status badge instead of dot
        let status_label = match session.status {
            HeadlessSessionStatus::Starting => "starting",
            HeadlessSessionStatus::Running => "running",
            HeadlessSessionStatus::Completed => "done",
            HeadlessSessionStatus::Failed => "failed",
        };

        let status_badge = container(text(status_label).size(theme::font_size::CAPTION).style(
            move |_| iced::widget::text::Style {
                color: Some(status_color),
            },
        ))
        .padding([1, 6])
        .style(theme::status_badge(status_color));

        let title = text(&session.task_title)
            .size(theme::font_size::BODY)
            .style(|_| iced::widget::text::Style {
                color: Some(theme::text::PRIMARY),
            });

        let stats = text(format!(
            "{} events / {} lines",
            session.event_count, session.line_count
        ))
        .size(theme::font_size::CAPTION)
        .style(theme::muted_text());

        let task_content = column![
            row![status_badge, title]
                .spacing(6)
                .align_y(Alignment::Center),
            stats,
        ]
        .spacing(2);

        let (bg_color, left_border_width) = if is_selected {
            (theme::surface::HOVER, 3.0_f32)
        } else {
            (iced::Color::TRANSPARENT, 0.0_f32)
        };
        let accent = theme::ACCENT;

        let task_button = button(
            container(task_content)
                .padding(8)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: if left_border_width > 0.0 {
                            accent
                        } else {
                            iced::Color::TRANSPARENT
                        },
                        width: left_border_width,
                        radius: theme::RADIUS_SMALL.into(),
                    },
                    ..Default::default()
                }),
        )
        .on_press(Message::MonitorSelectTask(session.task_id.clone()))
        .width(Length::Fill)
        .style(|_, _| button::Style {
            background: None,
            ..Default::default()
        });

        task_column = task_column.push(task_button);
    }

    let task_list = scrollable(task_column).height(Length::Fill);

    let clear_button = button(
        text("Clear Completed")
            .size(theme::font_size::SMALL)
            .style(theme::secondary_text()),
    )
    .on_press(Message::MonitorClearCompleted)
    .style(theme::ghost_button());

    let panel = column![task_list, clear_button]
        .spacing(8)
        .height(Length::Fill);

    container(panel)
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .padding(8)
        .style(|_| container::Style {
            border: Border {
                color: theme::border::SUBTLE,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Build the right panel showing streaming output for the selected task
fn build_output_panel<'a>(
    sessions: &'a HashMap<String, HeadlessSessionInfo>,
    selected_task: &Option<String>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = match selected_task {
        Some(task_id) => match sessions.get(task_id) {
            Some(session) => {
                let status_label = match &session.status {
                    HeadlessSessionStatus::Starting => "Starting",
                    HeadlessSessionStatus::Running => "Running",
                    HeadlessSessionStatus::Completed => "Completed",
                    HeadlessSessionStatus::Failed => "Failed",
                };

                let header_title = text(&session.task_title)
                    .size(theme::font_size::HEADING)
                    .style(|_| iced::widget::text::Style {
                        color: Some(theme::text::PRIMARY),
                    });

                let header_meta = text(format!("{} | {}", session.harness, status_label))
                    .size(theme::font_size::SMALL)
                    .style(theme::secondary_text());

                // Copy button (always available when there's output)
                let copy_btn = button(text("Copy").size(theme::font_size::SMALL))
                    .on_press(Message::MonitorCopyOutput {
                        task_id: session.task_id.clone(),
                    })
                    .style(theme::ghost_button());

                // Show attach button if session has a session_id (can be resumed)
                // Show stop button if session is still running
                let is_running = matches!(
                    session.status,
                    HeadlessSessionStatus::Starting | HeadlessSessionStatus::Running
                );

                let mut header_row = row![
                    column![header_title, header_meta].spacing(theme::SPACING_SM),
                    copy_btn,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(iced::Alignment::Start);

                if is_running {
                    let stop_btn = button(text("Stop").size(theme::font_size::SMALL))
                        .on_press(Message::MonitorStopSession {
                            task_id: session.task_id.clone(),
                        })
                        .style(theme::danger_button());
                    header_row = header_row.push(stop_btn);
                }

                if session.session_id.is_some() {
                    let attach_btn = button(text("Attach").size(theme::font_size::SMALL))
                        .on_press(Message::MonitorAttachSession {
                            task_id: session.task_id.clone(),
                        })
                        .style(theme::primary_button());
                    header_row = header_row.push(attach_btn);
                }

                let header: Element<'a, Message> = header_row.into();

                let mut output_column = Column::new().spacing(2);
                for line in &session.output_lines {
                    // Color tool events and validation/repair lines
                    let line_style = if line.starts_with(">>") {
                        move |_: &_| iced::widget::text::Style {
                            color: Some(theme::ACCENT),
                        }
                    } else if line.starts_with("<<") {
                        move |_: &_| iced::widget::text::Style {
                            color: Some(theme::SUCCESS),
                        }
                    } else if line.starts_with("--- VALIDATION") || line.starts_with("--- REPAIR") {
                        move |_: &_| iced::widget::text::Style {
                            color: Some(theme::WARNING),
                        }
                    } else {
                        move |_: &_| iced::widget::text::Style {
                            color: Some(theme::text::PRIMARY),
                        }
                    };

                    output_column = output_column
                        .push(text(line).size(theme::font_size::SMALL).style(line_style));
                }

                // Show the in-progress partial line so output streams in real-time
                if !session.partial_line.is_empty() {
                    output_column = output_column.push(
                        text(&session.partial_line)
                            .size(theme::font_size::SMALL)
                            .style(|_: &_| iced::widget::text::Style {
                                color: Some(theme::text::SECONDARY),
                            }),
                    );
                }

                let output_scroll = scrollable(
                    container(output_column)
                        .padding(theme::SPACING_MD)
                        .width(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(Background::Color(theme::surface::BASE)),
                            border: Border {
                                color: theme::border::SUBTLE,
                                width: 1.0,
                                radius: theme::RADIUS.into(),
                            },
                            ..Default::default()
                        }),
                )
                .height(Length::Fill);

                column![header, output_scroll]
                    .spacing(theme::SPACING_MD)
                    .into()
            }
            None => centered_placeholder("Select a task to view output"),
        },
        None => centered_placeholder("Select a task to view output"),
    };

    container(content)
        .width(Length::FillPortion(7))
        .height(Length::Fill)
        .padding(8)
        .into()
}

/// Create a centered placeholder message
fn centered_placeholder(message: &str) -> Element<'_, Message> {
    container(
        text(message)
            .style(theme::muted_text())
            .size(theme::font_size::BODY),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
