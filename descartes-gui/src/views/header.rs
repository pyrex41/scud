//! Header view
//!
//! Navigation and status display header.

use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Border, Element, Length};

use crate::state::AgentStatus;
use crate::theme;
use crate::Message;

/// Current view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Waves,
    Launch,
    Agents,
    Output,
    Monitor,
    Settings,
}

/// Render the header with navigation and status
pub fn view<'a>(
    current_view: ViewMode,
    agent_status: AgentStatus,
    headless_session_count: usize,
) -> Element<'a, Message> {
    let monitor_label = if headless_session_count > 0 {
        format!("Monitor ({})", headless_session_count)
    } else {
        "Monitor".to_string()
    };

    let nav_buttons = row![
        button(text("Waves").size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Waves))
            .style(theme::nav_button(current_view == ViewMode::Waves)),
        button(text("Launch").size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Launch))
            .style(theme::nav_button(current_view == ViewMode::Launch)),
        button(text("Agents").size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Agents))
            .style(theme::nav_button(current_view == ViewMode::Agents)),
        button(text("Output").size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Output))
            .style(theme::nav_button(current_view == ViewMode::Output)),
        button(text(monitor_label).size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Monitor))
            .style(theme::nav_button(current_view == ViewMode::Monitor)),
        button(text("Settings").size(theme::font_size::BODY))
            .on_press(Message::SwitchView(ViewMode::Settings))
            .style(theme::nav_button(current_view == ViewMode::Settings)),
    ]
    .spacing(4);

    let status_color = match agent_status {
        AgentStatus::Running => theme::SUCCESS,
        AgentStatus::Paused => theme::WARNING,
        AgentStatus::Idle => theme::text::MUTED,
    };

    let status = text(format!("Status: {:?}", agent_status))
        .size(theme::font_size::SMALL)
        .style(move |_| iced::widget::text::Style {
            color: Some(status_color),
        });

    let header_content = row![nav_buttons, status]
        .spacing(20)
        .align_y(Alignment::Center);

    container(header_content)
        .width(Length::Fill)
        .padding([12, 20])
        .style(|_| container::Style {
            background: Some(Background::Color(theme::surface::RAISED)),
            border: Border {
                color: theme::border::SUBTLE,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}
