//! Task waves view
//!
//! Displays tasks organized in parallel execution waves.

use iced::widget::{button, column, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

use crate::state::TaskInfo;
use crate::Message;

/// Render the waves view showing tasks organized by execution wave
pub fn view<'a>(waves: &'a [Vec<TaskInfo>], active_tag: &Option<String>) -> Element<'a, Message> {
    let mut waves_column = Column::new().spacing(15);

    if waves.is_empty() {
        waves_column = waves_column.push(text("No tasks loaded. Click Refresh to load tasks."));
    } else {
        for (wave_idx, wave) in waves.iter().enumerate() {
            let wave_header = text(format!("Wave {}", wave_idx + 1)).size(18);

            let mut task_column = Column::new().spacing(5);
            for task in wave {
                // Build task action buttons based on status
                let mut task_actions = row![].spacing(5);

                // Only show Start for non-completed tasks
                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions
                        .push(button("Start").on_press(Message::StartAgent(task.id.clone())));
                }

                // Show status management buttons
                if task.status != "Done" && task.status != "done" {
                    task_actions = task_actions.push(
                        button("Done")
                            .on_press(Message::MarkTaskComplete { task_id: task.id.clone() }),
                    );
                }
                if task.status != "Blocked" && task.status != "blocked" {
                    task_actions = task_actions.push(
                        button("Block")
                            .on_press(Message::MarkTaskBlocked { task_id: task.id.clone() }),
                    );
                }

                let task_row = row![
                    text(&task.id).width(Length::Fixed(80.0)),
                    text(&task.title).width(Length::Fill),
                    text(&task.status).width(Length::Fixed(100.0)),
                    task_actions,
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                task_column = task_column.push(task_row);
            }

            waves_column = waves_column.push(wave_header).push(task_column);
        }
    }

    // Use ScudBridge for refresh via RefreshTasks message
    let refresh_button = button("Refresh").on_press(Message::RefreshTasks);

    // Show active tag filter if set
    let tag_label = if let Some(ref tag) = active_tag {
        text(format!("Tag: {}", tag))
    } else {
        text("All tasks")
    };

    let header_row = row![refresh_button, tag_label]
        .spacing(15)
        .align_y(Alignment::Center);

    column![header_row, scrollable(waves_column).height(Length::Fill)]
        .spacing(10)
        .into()
}
