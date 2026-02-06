//! Tag explorer view
//!
//! View all tags with status counts, switch active tag, archive tags,
//! and restore from archives.

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Background, Border, Element, Length};

use crate::state::TagExplorerState;
use crate::theme;
use crate::Message;

/// Render the tag explorer view.
pub fn view(state: &TagExplorerState) -> Element<'_, Message> {
    let tags_section = build_tags_section(state);
    let archives_section = build_archives_section(state);

    scrollable(
        column![tags_section, archives_section]
            .spacing(theme::SPACING_LG)
            .padding(theme::SPACING_MD),
    )
    .height(Length::Fill)
    .into()
}

/// Build the active tags section
fn build_tags_section(state: &TagExplorerState) -> Element<'_, Message> {
    let heading = text("Tags")
        .size(theme::font_size::HEADING)
        .style(theme::heading_text());

    let mut tag_column = Column::new().spacing(theme::SPACING_SM);

    if state.tags.is_empty() {
        tag_column = tag_column.push(
            text("No tags found")
                .size(theme::font_size::BODY)
                .style(theme::muted_text()),
        );
    } else {
        // Header row
        let header = row![
            text("").width(Length::Fixed(20.0)), // indicator
            text("Tag")
                .width(Length::FillPortion(3))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Total")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Done")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Pend")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("WIP")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Fail")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Actions")
                .width(Length::Fixed(160.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
        ]
        .spacing(theme::SPACING_SM)
        .align_y(Alignment::Center);

        tag_column = tag_column.push(header);

        for tag in &state.tags {
            let indicator_color = if tag.is_active {
                theme::SUCCESS
            } else {
                theme::text::MUTED
            };

            let indicator = container(text("").size(6.0))
                .width(Length::Fixed(20.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(indicator_color)),
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(12.0));

            let is_active = tag.is_active;
            let name_text = text(&tag.name)
                .width(Length::FillPortion(3))
                .size(theme::font_size::BODY)
                .style(move |_| iced::widget::text::Style {
                    color: Some(if is_active {
                        theme::ACCENT
                    } else {
                        theme::text::SECONDARY
                    }),
                });

            let total = text(tag.total_tasks.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(theme::secondary_text());

            let done = text(tag.done_count.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(|_| iced::widget::text::Style {
                    color: Some(theme::SUCCESS),
                });

            let pending = text(tag.pending_count.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(theme::muted_text());

            let wip = text(tag.in_progress_count.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(|_| iced::widget::text::Style {
                    color: Some(theme::ACCENT),
                });

            let failed = text(tag.failed_count.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(|_| iced::widget::text::Style {
                    color: Some(if tag.failed_count > 0 {
                        theme::ERROR
                    } else {
                        theme::text::MUTED
                    }),
                });

            let mut actions = row![].spacing(theme::SPACING_SM);

            if !tag.is_active {
                actions = actions.push(
                    button("Set Active")
                        .on_press(Message::SetActiveTag(tag.name.clone()))
                        .style(theme::ghost_button()),
                );
            }

            actions = actions.push(
                button("Archive")
                    .on_press(Message::TagExplorerArchiveTag(tag.name.clone()))
                    .style(theme::ghost_button()),
            );

            let tag_row = container(
                row![indicator, name_text, total, done, pending, wip, failed, actions]
                    .spacing(theme::SPACING_SM)
                    .align_y(Alignment::Center),
            )
            .padding([6, 12])
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

            tag_column = tag_column.push(tag_row);
        }
    }

    column![heading, tag_column]
        .spacing(theme::SPACING_SM)
        .into()
}

/// Build the archives section
fn build_archives_section(state: &TagExplorerState) -> Element<'_, Message> {
    let heading = text("Archives")
        .size(theme::font_size::HEADING)
        .style(theme::heading_text());

    let mut archive_column = Column::new().spacing(theme::SPACING_SM);

    if state.archives.is_empty() {
        archive_column = archive_column.push(
            text("No archived tags")
                .size(theme::font_size::BODY)
                .style(theme::muted_text()),
        );
    } else {
        // Header row
        let header = row![
            text("File")
                .width(Length::FillPortion(3))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Date")
                .width(Length::Fixed(100.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Tag")
                .width(Length::Fixed(120.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("Tasks")
                .width(Length::Fixed(50.0))
                .size(theme::font_size::CAPTION)
                .style(theme::muted_text()),
            text("")
                .width(Length::Fixed(80.0)),
        ]
        .spacing(theme::SPACING_SM)
        .align_y(Alignment::Center);

        archive_column = archive_column.push(header);

        for archive in &state.archives {
            let filename = text(&archive.filename)
                .width(Length::FillPortion(3))
                .size(theme::font_size::BODY)
                .style(theme::secondary_text());

            let date = text(&archive.date)
                .width(Length::Fixed(100.0))
                .size(theme::font_size::BODY)
                .style(theme::muted_text());

            let tag_name = text(archive.tag.as_deref().unwrap_or("-"))
                .width(Length::Fixed(120.0))
                .size(theme::font_size::BODY)
                .style(theme::secondary_text());

            let task_count = text(archive.task_count.to_string())
                .width(Length::Fixed(50.0))
                .size(theme::font_size::BODY)
                .style(theme::secondary_text());

            let restore_btn = button("Restore")
                .on_press(Message::RestoreArchive {
                    filename: archive.filename.clone(),
                })
                .style(theme::ghost_button());

            let archive_row = container(
                row![filename, date, tag_name, task_count, restore_btn]
                    .spacing(theme::SPACING_SM)
                    .align_y(Alignment::Center),
            )
            .padding([6, 12])
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

            archive_column = archive_column.push(archive_row);
        }
    }

    column![heading, archive_column]
        .spacing(theme::SPACING_SM)
        .into()
}
