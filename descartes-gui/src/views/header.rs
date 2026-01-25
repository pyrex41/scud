//! Header view
//!
//! Navigation and status display header.

use iced::widget::{button, row, text};
use iced::{Alignment, Element};

use crate::state::AgentStatus;
use crate::Message;

/// Current view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Waves,
    Agents,
    Output,
}

/// Render the header with navigation and status
pub fn view<'a>(current_view: ViewMode, agent_status: AgentStatus) -> Element<'a, Message> {
    let nav_buttons = row![
        button(text("Waves"))
            .on_press(Message::SwitchView(ViewMode::Waves))
            .style(if current_view == ViewMode::Waves {
                button::primary
            } else {
                button::secondary
            }),
        button(text("Agents"))
            .on_press(Message::SwitchView(ViewMode::Agents))
            .style(if current_view == ViewMode::Agents {
                button::primary
            } else {
                button::secondary
            }),
        button(text("Output"))
            .on_press(Message::SwitchView(ViewMode::Output))
            .style(if current_view == ViewMode::Output {
                button::primary
            } else {
                button::secondary
            }),
    ]
    .spacing(10);

    let status = text(format!("Status: {:?}", agent_status));

    row![nav_buttons, status]
        .spacing(20)
        .align_y(Alignment::Center)
        .into()
}
