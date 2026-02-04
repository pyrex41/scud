//! Headless session monitor view
//!
//! Displays a split-panel layout for monitoring headless swarm sessions:
//! task list on the left, streaming output on the right.

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};
use std::collections::HashMap;

use crate::state::{HeadlessSessionInfo, HeadlessSessionStatus};
use crate::theme;
use crate::Message;

/// Render the monitor view with session list and streaming output
pub fn view<'a>(
    sessions: &'a HashMap<String, HeadlessSessionInfo>,
    selected_task: &Option<String>,
) -> Element<'a, Message> {
    if sessions.is_empty() {
        return container(
            text("No headless sessions. Start a swarm or run a task in headless mode.")
                .style(|_| text::Style {
                    color: Some(theme::text::SECONDARY),
                }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    // Sort sessions: active (Starting/Running) first, then by task_id
    let mut sorted_sessions: Vec<&HeadlessSessionInfo> = sessions.values().collect();
    sorted_sessions.sort_by(|a, b| {
        let a_active = matches!(a.status, HeadlessSessionStatus::Starting | HeadlessSessionStatus::Running);
        let b_active = matches!(b.status, HeadlessSessionStatus::Starting | HeadlessSessionStatus::Running);
        b_active.cmp(&a_active).then_with(|| a.task_id.cmp(&b.task_id))
    });

    // Left panel: scrollable task list
    let left_panel = build_task_list(&sorted_sessions, selected_task);

    // Right panel: streaming output
    let right_panel = build_output_panel(sessions, selected_task);

    row![left_panel, right_panel]
        .spacing(10)
        .height(Length::Fill)
        .into()
}

/// Build the left panel with the scrollable task list and clear button
fn build_task_list<'a>(
    sorted_sessions: &[&'a HeadlessSessionInfo],
    selected_task: &Option<String>,
) -> Element<'a, Message> {
    let mut task_column = Column::new().spacing(4);

    for session in sorted_sessions {
        let is_selected = selected_task
            .as_ref()
            .map(|s| s == &session.task_id)
            .unwrap_or(false);

        let status_indicator = status_indicator_text(&session.status);

        let title = text(&session.task_title)
            .size(14)
            .style(|_| text::Style {
                color: Some(theme::text::PRIMARY),
            });

        let stats = text(format!("{} events / {} lines", session.event_count, session.line_count))
            .size(11)
            .style(|_| text::Style {
                color: Some(theme::text::MUTED),
            });

        let task_content = column![
            row![status_indicator, title].spacing(6).align_y(Alignment::Center),
            stats,
        ]
        .spacing(2);

        let task_button = button(
            container(task_content)
                .padding(8)
                .width(Length::Fill)
                .style(move |_| {
                    if is_selected {
                        container::Style {
                            background: Some(iced::Background::Color(theme::background::TERTIARY)),
                            ..Default::default()
                        }
                    } else {
                        container::Style::default()
                    }
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
        text("Clear Completed").size(13).style(|_| text::Style {
            color: Some(theme::text::SECONDARY),
        }),
    )
    .on_press(Message::MonitorClearCompleted);

    let panel = column![task_list, clear_button]
        .spacing(8)
        .height(Length::Fill);

    container(panel)
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .padding(8)
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

                let header_title = text(&session.task_title).size(16).style(|_| text::Style {
                    color: Some(theme::text::PRIMARY),
                });

                let header_meta = text(format!("{} | {}", session.harness, status_label))
                    .size(12)
                    .style(|_| text::Style {
                        color: Some(theme::text::SECONDARY),
                    });

                let header = column![header_title, header_meta].spacing(4);

                let mut output_column = Column::new().spacing(1);
                for line in &session.output_lines {
                    output_column = output_column.push(
                        text(line).size(13).style(|_| text::Style {
                            color: Some(theme::text::PRIMARY),
                        }),
                    );
                }

                let output_scroll = scrollable(
                    container(output_column)
                        .padding(10)
                        .width(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(iced::Background::Color(
                                theme::background::TERTIARY,
                            )),
                            ..Default::default()
                        }),
                )
                .height(Length::Fill);

                column![header, output_scroll].spacing(8).into()
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

/// Create a status indicator text widget colored by session status
fn status_indicator_text(status: &HeadlessSessionStatus) -> Element<'_, Message> {
    let color = match status {
        HeadlessSessionStatus::Starting => theme::text::MUTED,
        HeadlessSessionStatus::Running => theme::ACCENT,
        HeadlessSessionStatus::Completed => theme::SUCCESS,
        HeadlessSessionStatus::Failed => theme::ERROR,
    };

    text("\u{25cf}")
        .size(14)
        .style(move |_| text::Style {
            color: Some(color),
        })
        .into()
}

/// Create a centered placeholder message
fn centered_placeholder(message: &str) -> Element<'_, Message> {
    container(text(message).style(|_| text::Style {
        color: Some(theme::text::MUTED),
    }))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
