//! Live output view
//!
//! Displays streaming output from the running agent.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Element, Length};

use crate::state::AgentStatus;
use crate::theme;
use crate::Message;

/// Render the output view with task output and controls
pub fn view<'a>(
    current_task: &Option<String>,
    agent_status: AgentStatus,
    output_buffer: &'a str,
) -> Element<'a, Message> {
    // Header with current task and controls
    let status_color = match agent_status {
        AgentStatus::Running => theme::SUCCESS,
        AgentStatus::Paused => theme::WARNING,
        AgentStatus::Idle => theme::text::MUTED,
    };

    let task_status = if let Some(ref task_id) = current_task {
        text(format!("Task: {} | Status: {:?}", task_id, agent_status))
            .size(theme::font_size::BODY)
            .style(move |_| iced::widget::text::Style {
                color: Some(status_color),
            })
    } else {
        text(format!("Status: {:?}", agent_status))
            .size(theme::font_size::BODY)
            .style(theme::secondary_text())
    };

    let clear_button = button("Clear")
        .on_press(Message::ClearOutput)
        .style(theme::ghost_button());
    let header = row![task_status, clear_button]
        .spacing(theme::SPACING_LG)
        .align_y(Alignment::Center);

    // Output content
    let output_text = if output_buffer.is_empty() {
        text("No output yet. Start a task or swarm to see output here.")
            .style(theme::muted_text())
            .size(theme::font_size::BODY)
    } else {
        text(output_buffer).size(theme::font_size::BODY)
    };

    let output_container = scrollable(
        container(output_text)
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

    column![header, output_container]
        .spacing(theme::SPACING_MD)
        .into()
}
