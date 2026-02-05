//! Task waves view
//!
//! Displays tasks organized in parallel execution waves.

use iced::widget::{button, column, container, pick_list, row, scrollable, text, Column};
use iced::{Alignment, Background, Border, Element, Length};

use crate::state::TaskInfo;
use crate::theme;
use crate::Message;

/// Render the waves view showing tasks organized by execution wave
pub fn view<'a>(
    waves: &'a [Vec<TaskInfo>],
    active_tag: &Option<String>,
    available_tags: &'a [String],
) -> Element<'a, Message> {
    let mut waves_column = Column::new().spacing(theme::SPACING_LG);

    if waves.is_empty() {
        waves_column = waves_column.push(
            text("No tasks loaded. Click Refresh to load tasks.")
                .style(theme::secondary_text())
                .size(theme::font_size::BODY),
        );
    } else {
        for (wave_idx, wave) in waves.iter().enumerate() {
            let wave_header = text(format!("Wave {}", wave_idx + 1))
                .size(theme::font_size::HEADING)
                .style(theme::heading_text());

            let mut task_column = Column::new().spacing(theme::SPACING_SM);
            for task in wave {
                let mut task_actions = row![].spacing(theme::SPACING_SM);

                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions.push(
                        button("Spawn")
                            .on_press(Message::SpawnTask {
                                task_id: task.id.clone(),
                            })
                            .style(theme::primary_button()),
                    );
                }

                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions.push(
                        button("Done")
                            .on_press(Message::MarkTaskComplete {
                                task_id: task.id.clone(),
                            })
                            .style(theme::ghost_button()),
                    );
                }
                if task.status != "Blocked" && task.status != "blocked" {
                    task_actions = task_actions.push(
                        button("Block")
                            .on_press(Message::MarkTaskBlocked {
                                task_id: task.id.clone(),
                            })
                            .style(theme::ghost_button()),
                    );
                }

                // Status badge
                let status_color = match task.status.to_lowercase().as_str() {
                    "done" => theme::SUCCESS,
                    "in-progress" | "inprogress" => theme::ACCENT,
                    "blocked" | "failed" => theme::ERROR,
                    _ => theme::text::MUTED,
                };

                let status_badge = container(
                    text(&task.status)
                        .size(theme::font_size::CAPTION)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(status_color),
                        }),
                )
                .padding([2, 8])
                .style(theme::status_badge(status_color));

                let task_id_text = text(&task.id)
                    .size(theme::font_size::CAPTION)
                    .style(theme::muted_text())
                    .width(Length::Fixed(60.0));

                // Agent type badge (if assigned)
                let agent_badge = if let Some(ref agent) = task.agent {
                    container(
                        text(format!("@{}", agent))
                            .size(theme::font_size::CAPTION)
                            .style(|_| iced::widget::text::Style {
                                color: Some(theme::ACCENT),
                            }),
                    )
                    .padding([2, 6])
                    .style(|_| container::Style {
                        background: Some(Background::Color(iced::Color {
                            a: 0.1,
                            ..theme::ACCENT
                        })),
                        border: Border {
                            color: iced::Color {
                                a: 0.2,
                                ..theme::ACCENT
                            },
                            width: 1.0,
                            radius: theme::RADIUS_SMALL.into(),
                        },
                        ..Default::default()
                    })
                } else {
                    container(text("").size(theme::font_size::CAPTION))
                };

                let task_content = row![
                    task_id_text,
                    text(&task.title)
                        .size(theme::font_size::BODY)
                        .width(Length::Fill),
                    agent_badge,
                    status_badge,
                    task_actions,
                ]
                .spacing(theme::SPACING_MD)
                .align_y(Alignment::Center);

                let task_card = container(task_content)
                    .padding([8, 12])
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

                task_column = task_column.push(task_card);
            }

            waves_column = waves_column.push(wave_header).push(task_column);
        }
    }

    // Header row with controls
    let refresh_button = button("Refresh")
        .on_press(Message::RefreshTasks)
        .style(theme::ghost_button());

    let tag_options: Vec<String> = available_tags.to_vec();
    let tag_picker = pick_list(tag_options, active_tag.clone(), Message::SetLaunchTag)
        .placeholder("Select tag...");

    let archive_button = if let Some(ref tag) = active_tag {
        button("Archive")
            .on_press(Message::ArchiveTag {
                tag: tag.clone(),
            })
            .style(theme::ghost_button())
    } else {
        button("Archive").style(theme::ghost_button())
    };

    let tag_label = if let Some(ref tag) = active_tag {
        text(format!("Tag: {}", tag))
            .size(theme::font_size::SMALL)
            .style(theme::secondary_text())
    } else {
        text("All tasks")
            .size(theme::font_size::SMALL)
            .style(theme::muted_text())
    };

    let header_row = row![refresh_button, tag_picker, archive_button, tag_label]
        .spacing(theme::SPACING_MD)
        .align_y(Alignment::Center);

    column![header_row, scrollable(waves_column).height(Length::Fill)]
        .spacing(theme::SPACING_MD)
        .into()
}
