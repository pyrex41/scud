//! Settings view
//!
//! Application settings, project selection, and backpressure configuration.

use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Column};
use iced::{Alignment, Background, Border, Element, Length};
use std::path::Path;

use crate::state::{AppSettings, BackpressureState};
use crate::theme;
use crate::Message;

/// Available terminal applications on macOS
const TERMINAL_APPS: &[&str] = &["Terminal", "iTerm", "Warp", "Alacritty", "Kitty"];

/// Render the settings view
pub fn view<'a>(
    settings: &AppSettings,
    working_directory: &Path,
    backpressure: &'a BackpressureState,
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

    // Backpressure section
    let backpressure_section = build_backpressure_section(backpressure, label_width);

    // Main content
    let content = column![
        project_section,
        recent_section,
        container(row![]).height(Length::Fixed(20.0)), // Spacer
        terminal_section,
        container(row![]).height(Length::Fixed(20.0)), // Spacer
        backpressure_section,
    ]
    .spacing(theme::SPACING_LG);

    scrollable(
        container(content)
            .width(Length::Fill)
            .padding(theme::SPACING_LG),
    )
    .height(Length::Fill)
    .into()
}

/// Build the backpressure configuration section
fn build_backpressure_section(
    state: &BackpressureState,
    label_width: Length,
) -> Element<'_, Message> {
    let mut section = Column::new().spacing(theme::SPACING_MD);

    // Heading with source indicator
    let source_label = if state.is_auto_detected {
        "auto-detected"
    } else {
        "from config"
    };

    section = section.push(
        row![
            text("Backpressure")
                .size(theme::font_size::HEADING)
                .style(theme::heading_text()),
            text(format!("({})", source_label))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
        ]
        .spacing(theme::SPACING_SM)
        .align_y(Alignment::Center),
    );

    section = section.push(
        text("Validation commands run after each task (Ralph) or wave (Swarm)")
            .size(theme::font_size::CAPTION)
            .style(theme::muted_text()),
    );

    if !state.loaded {
        section = section.push(
            text("Loading...")
                .size(theme::font_size::BODY)
                .style(theme::muted_text()),
        );
        return section.into();
    }

    // Command list
    let mut cmd_list = Column::new().spacing(theme::SPACING_SM);

    if state.commands.is_empty() {
        cmd_list = cmd_list.push(
            text("No commands configured")
                .size(theme::font_size::SMALL)
                .style(theme::muted_text()),
        );
    } else {
        for (i, cmd) in state.commands.iter().enumerate() {
            let cmd_row = row![
                text(cmd)
                    .size(theme::font_size::SMALL)
                    .width(Length::Fill)
                    .style(theme::secondary_text()),
                button(text("X").size(theme::font_size::CAPTION))
                    .on_press(Message::RemoveBackpressureCommand(i))
                    .style(theme::ghost_button()),
            ]
            .spacing(theme::SPACING_SM)
            .align_y(Alignment::Center);

            cmd_list = cmd_list.push(
                container(cmd_row)
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(Background::Color(theme::surface::RAISED)),
                        border: Border {
                            color: theme::border::SUBTLE,
                            width: 1.0,
                            radius: theme::RADIUS_SMALL.into(),
                        },
                        ..Default::default()
                    }),
            );
        }
    }

    section = section.push(cmd_list);

    // Add command input
    let add_row = row![
        text_input("e.g. cargo test", &state.new_command_input)
            .on_input(Message::SetBackpressureNewCommand)
            .on_submit(Message::AddBackpressureCommand)
            .width(Length::Fill),
        button("Add")
            .on_press(Message::AddBackpressureCommand)
            .style(theme::ghost_button()),
    ]
    .spacing(theme::SPACING_SM)
    .align_y(Alignment::Center);

    section = section.push(add_row);

    // Options row: stop_on_failure + timeout
    let stop_btn = toggle_button(
        state.stop_on_failure,
        Message::SetBackpressureStopOnFailure(!state.stop_on_failure),
    );

    let options_row = row![
        text("Stop on failure")
            .width(label_width)
            .style(theme::secondary_text()),
        stop_btn,
        text("Timeout (s)")
            .width(Length::Fixed(80.0))
            .style(theme::secondary_text()),
        text_input("300", &state.timeout_input)
            .on_input(Message::SetBackpressureTimeout)
            .width(Length::Fixed(60.0)),
    ]
    .spacing(theme::SPACING_MD)
    .align_y(Alignment::Center);

    section = section.push(options_row);

    // Action buttons: Save + Detect
    let mut action_row = row![].spacing(theme::SPACING_SM).align_y(Alignment::Center);

    if state.dirty {
        action_row = action_row.push(
            button("Save to config.toml")
                .on_press(Message::SaveBackpressureConfig)
                .style(theme::primary_button()),
        );
    } else {
        action_row = action_row.push(
            button("Save to config.toml").style(theme::ghost_button()),
        );
    }

    action_row = action_row.push(
        button("Re-detect")
            .on_press(Message::DetectBackpressureCommands)
            .style(theme::ghost_button()),
    );

    if let Some(ref status) = state.status {
        action_row = action_row.push(
            text(status)
                .size(theme::font_size::SMALL)
                .style(theme::secondary_text()),
        );
    }

    section = section.push(action_row);

    section.into()
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
