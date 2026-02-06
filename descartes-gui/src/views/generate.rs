//! Generate view
//!
//! Browse PRD markdown files, preview them, configure and run `scud generate`.

use iced::widget::{
    button, checkbox, column, container, row, scrollable, text, text_input, Column,
};
use iced::{Alignment, Background, Border, Element, Length};

use crate::state::GenerateState;
use crate::theme;
use crate::Message;

/// Render the generate view with PRD file list and preview/config panel.
pub fn view<'a>(
    state: &'a GenerateState,
    working_dir: &'a std::path::Path,
) -> Element<'a, Message> {
    // Left panel: PRD file list
    let left_panel = build_file_list(state, working_dir);

    // Right panel: preview + config
    let right_panel = build_preview_panel(state);

    row![
        container(left_panel)
            .width(Length::FillPortion(3))
            .height(Length::Fill),
        container(right_panel)
            .width(Length::FillPortion(7))
            .height(Length::Fill),
    ]
    .spacing(theme::SPACING_MD)
    .into()
}

/// Build the left panel: file list with scan button
fn build_file_list<'a>(
    state: &'a GenerateState,
    working_dir: &'a std::path::Path,
) -> Element<'a, Message> {
    let heading = text("PRD Files")
        .size(theme::font_size::HEADING)
        .style(theme::heading_text());

    let mut file_column = Column::new().spacing(theme::SPACING_SM);

    if state.prd_files.is_empty() {
        file_column = file_column.push(
            text("No PRD files found. Click Rescan to search.")
                .size(theme::font_size::SMALL)
                .style(theme::muted_text()),
        );
    } else {
        for file_path in &state.prd_files {
            let display_name = file_path
                .strip_prefix(working_dir)
                .unwrap_or(file_path)
                .display()
                .to_string();

            let is_selected = state.selected_prd.as_ref() == Some(file_path);

            let file_btn = if is_selected {
                button(text(display_name).size(theme::font_size::BODY))
                    .on_press(Message::SelectPrd(file_path.clone()))
                    .width(Length::Fill)
                    .style(theme::primary_button())
            } else {
                button(text(display_name).size(theme::font_size::BODY))
                    .on_press(Message::SelectPrd(file_path.clone()))
                    .width(Length::Fill)
                    .style(theme::ghost_button())
            };

            file_column = file_column.push(file_btn);
        }
    }

    let scan_button = button(text("Rescan").size(theme::font_size::SMALL))
        .on_press(Message::ScanPrdDirectory)
        .style(theme::ghost_button());

    column![
        heading,
        scrollable(file_column).height(Length::Fill),
        scan_button,
    ]
    .spacing(theme::SPACING_MD)
    .padding(theme::SPACING_MD)
    .into()
}

/// Build the right panel: preview + generate config
fn build_preview_panel(state: &GenerateState) -> Element<'_, Message> {
    if state.selected_prd.is_none() {
        return container(
            text("Select a PRD to preview")
                .size(theme::font_size::BODY)
                .style(theme::muted_text()),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let preview = if let Some(ref content) = state.prd_content {
        scrollable(
            container(
                text(content)
                    .size(theme::font_size::SMALL)
                    .style(theme::secondary_text()),
            )
            .padding(theme::SPACING_MD),
        )
        .height(Length::Fill)
    } else {
        scrollable(
            container(
                text("Loading...")
                    .size(theme::font_size::SMALL)
                    .style(theme::muted_text()),
            )
            .padding(theme::SPACING_MD),
        )
        .height(Length::Fill)
    };

    let config_panel = build_config_panel(state);

    // Streaming output panel (shown when generating or after completion)
    if !state.generate_output_lines.is_empty() {
        let output_panel = build_output_panel(state);
        column![preview, config_panel, output_panel]
            .spacing(theme::SPACING_MD)
            .padding(theme::SPACING_MD)
            .into()
    } else {
        column![preview, config_panel]
            .spacing(theme::SPACING_MD)
            .padding(theme::SPACING_MD)
            .into()
    }
}

/// Build the streaming output panel showing generate progress
fn build_output_panel(state: &GenerateState) -> Element<'_, Message> {
    let mut output_column = Column::new().spacing(2);

    for line in &state.generate_output_lines {
        output_column = output_column.push(
            text(line)
                .size(theme::font_size::SMALL)
                .style(theme::secondary_text()),
        );
    }

    let header_row = row![
        text("Output")
            .size(theme::font_size::BODY)
            .style(theme::heading_text()),
        text(format!("{} lines", state.generate_output_lines.len()))
            .size(theme::font_size::CAPTION)
            .style(theme::muted_text()),
    ]
    .spacing(theme::SPACING_SM)
    .align_y(Alignment::Center);

    container(
        column![
            header_row,
            container(
                scrollable(output_column)
                    .height(Length::Fixed(200.0))
            )
            .padding(theme::SPACING_SM)
            .width(Length::Fill)
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
        .spacing(theme::SPACING_SM),
    )
    .padding(theme::SPACING_MD)
    .width(Length::Fill)
    .style(theme::panel_container())
    .into()
}

/// Build the generate config panel at the bottom of the preview
fn build_config_panel(state: &GenerateState) -> Element<'_, Message> {
    let label_width = Length::Fixed(80.0);

    // Tag input row
    let tag_row = row![
        text("Tag").width(label_width).style(theme::secondary_text()),
        text_input("e.g. hello-world", &state.tag_input)
            .on_input(Message::SetGenerateTag)
            .width(Length::Fixed(200.0)),
    ]
    .spacing(theme::SPACING_MD)
    .align_y(Alignment::Center);

    // Task count row
    let num_row = row![
        text("Tasks").width(label_width).style(theme::secondary_text()),
        text_input("10", &state.num_tasks_input)
            .on_input(Message::SetGenerateNumTasks)
            .width(Length::Fixed(60.0)),
    ]
    .spacing(theme::SPACING_MD)
    .align_y(Alignment::Center);

    // Options as checkboxes — much clearer than ON/OFF toggles
    let options_row = row![
        checkbox(!state.no_expand)
            .label("Expand subtasks")
            .on_toggle(|v| Message::SetGenerateNoExpand(!v))
            .size(14)
            .text_size(theme::font_size::SMALL),
        checkbox(!state.no_check_deps)
            .label("Check deps")
            .on_toggle(|v| Message::SetGenerateNoCheckDeps(!v))
            .size(14)
            .text_size(theme::font_size::SMALL),
        checkbox(state.append)
            .label("Append to existing")
            .on_toggle(Message::SetGenerateAppend)
            .size(14)
            .text_size(theme::font_size::SMALL),
    ]
    .spacing(theme::SPACING_LG);

    // Generate button + status
    let can_generate =
        !state.generating && !state.tag_input.is_empty() && state.selected_prd.is_some();

    let generate_btn = if can_generate {
        button(text("Generate").size(theme::font_size::BODY))
            .on_press(Message::StartGenerate)
            .style(theme::primary_button())
    } else {
        button(text("Generate").size(theme::font_size::BODY)).style(theme::ghost_button())
    };

    let status_text = if state.generating {
        text("Generating...")
            .size(theme::font_size::SMALL)
            .style(theme::heading_text())
    } else if let Some(ref status) = state.generate_status {
        text(status)
            .size(theme::font_size::SMALL)
            .style(theme::secondary_text())
    } else if !can_generate && state.tag_input.is_empty() && state.selected_prd.is_some() {
        text("Enter a tag name to generate")
            .size(theme::font_size::SMALL)
            .style(theme::muted_text())
    } else {
        text("")
            .size(theme::font_size::SMALL)
            .style(theme::muted_text())
    };

    let action_row = row![generate_btn, status_text]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center);

    container(
        column![tag_row, num_row, options_row, action_row].spacing(theme::SPACING_MD),
    )
    .padding(theme::SPACING_MD)
    .width(Length::Fill)
    .style(theme::panel_container())
    .into()
}
