//! Settings view
//!
//! Application settings and project selection.

use iced::widget::{button, column, container, pick_list, row, scrollable, text, Column};
use iced::{Alignment, Background, Border, Element, Length};
use std::path::Path;

use crate::state::AppSettings;
use crate::theme;
use crate::Message;

/// Available terminal applications on macOS
const TERMINAL_APPS: &[&str] = &["Terminal", "iTerm", "Warp", "Alacritty", "Kitty"];

/// Render the settings view
pub fn view<'a>(
    settings: &AppSettings,
    working_directory: &Path,
) -> Element<'a, Message> {
    let label_width = Length::Fixed(160.0);

    // Project section
    let project_path = working_directory.display().to_string();
    let project_section = column![
        text("Project")
            .size(theme::font_size::HEADING)
            .style(theme::heading_text()),
        row![
            text("Working Directory")
                .width(label_width)
                .style(theme::secondary_text()),
            text(project_path)
                .size(theme::font_size::BODY)
                .style(|_| iced::widget::text::Style {
                    color: Some(theme::text::PRIMARY),
                }),
            button(text("Change...").size(theme::font_size::SMALL))
                .on_press(Message::BrowseProject)
                .style(theme::ghost_button()),
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SPACING_MD);

    // Recent projects
    let mut recent_list = Column::new().spacing(theme::SPACING_SM);
    if settings.recent_projects.is_empty() {
        recent_list = recent_list.push(
            text("No recent projects")
                .size(theme::font_size::SMALL)
                .style(theme::muted_text()),
        );
    } else {
        for path in &settings.recent_projects {
            let path_str = path.display().to_string();
            let path_clone = path.clone();
            recent_list = recent_list.push(
                button(
                    text(path_str)
                        .size(theme::font_size::SMALL)
                        .style(theme::secondary_text()),
                )
                .on_press(Message::SwitchProject(path_clone))
                .style(theme::ghost_button()),
            );
        }
    }

    let recent_section = column![
        text("Recent Projects")
            .size(theme::font_size::BODY)
            .style(theme::secondary_text()),
        container(scrollable(recent_list).height(Length::Fixed(150.0)))
            .padding(theme::SPACING_SM)
            .style(|_| container::Style {
                background: Some(Background::Color(theme::surface::BASE)),
                border: Border {
                    color: theme::border::SUBTLE,
                    width: 1.0,
                    radius: theme::RADIUS_SMALL.into(),
                },
                ..Default::default()
            }),
    ]
    .spacing(theme::SPACING_SM);

    // Terminal section
    let terminal_options: Vec<String> = TERMINAL_APPS.iter().map(|s| s.to_string()).collect();
    let terminal_picker = pick_list(
        terminal_options,
        Some(settings.terminal_app.clone()),
        Message::SetTerminalApp,
    )
    .width(Length::Fixed(200.0));

    let terminal_section = column![
        text("Terminal")
            .size(theme::font_size::HEADING)
            .style(theme::heading_text()),
        row![
            text("Application")
                .width(label_width)
                .style(theme::secondary_text()),
            terminal_picker,
        ]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center),
        text("Used when attaching to agent sessions")
            .size(theme::font_size::CAPTION)
            .style(theme::muted_text()),
    ]
    .spacing(theme::SPACING_MD);

    // Main content
    let content = column![
        project_section,
        recent_section,
        container(row![]).height(Length::Fixed(20.0)), // Spacer
        terminal_section,
    ]
    .spacing(theme::SPACING_LG);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::SPACING_LG)
        .into()
}
