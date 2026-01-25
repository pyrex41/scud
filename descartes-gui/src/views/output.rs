//! Live output view
//!
//! Displays streaming output from the running agent.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

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
    let task_status = if let Some(ref task_id) = current_task {
        text(format!("Task: {} | Status: {:?}", task_id, agent_status))
    } else {
        text(format!("Status: {:?}", agent_status))
    };

    let clear_button = button("Clear").on_press(Message::ClearOutput);
    let header = row![task_status, clear_button]
        .spacing(15)
        .align_y(Alignment::Center);

    // Output content
    let output_text = if output_buffer.is_empty() {
        text("No output yet. Start a task or swarm to see output here.")
    } else {
        text(output_buffer)
    };

    let output_container = scrollable(
        container(output_text)
            .padding(10)
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(theme::background::PRIMARY)),
                ..Default::default()
            }),
    )
    .height(Length::Fill);

    column![header, output_container].spacing(10).into()
}
