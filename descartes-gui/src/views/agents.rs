//! Agent status view
//!
//! Displays current agent status and controls.

use iced::widget::{button, column, row, text};
use iced::Element;

use crate::state::{AgentStatus, SwarmDefaults};
use crate::Message;

/// Render the agents view with status and controls
pub fn view<'a>(
    agent_status: AgentStatus,
    current_task: &Option<String>,
    active_tag: &Option<String>,
    swarm_defaults: &SwarmDefaults,
) -> Element<'a, Message> {
    let status_text = match agent_status {
        AgentStatus::Idle => "No agent running",
        AgentStatus::Running => "Agent is running...",
        AgentStatus::Paused => "Agent is paused",
    };

    let mut controls = row![].spacing(10);

    match agent_status {
        AgentStatus::Idle => {
            // Swarm start controls when idle - use active tag or config default
            let tag = active_tag
                .clone()
                .unwrap_or_else(|| swarm_defaults.default_tag.clone());
            controls = controls.push(button("Start Swarm").on_press(Message::StartSwarm {
                tag,
                harness: swarm_defaults.harness.clone(),
                round_size: swarm_defaults.round_size,
            }));
        }
        AgentStatus::Running => {
            controls = controls
                .push(button("Pause").on_press(Message::PauseAgent))
                .push(button("Stop Swarm").on_press(Message::StopSwarm))
                .push(button("Cancel").on_press(Message::CancelAgent));
        }
        AgentStatus::Paused => {
            controls = controls
                .push(button("Resume").on_press(Message::ResumeAgent))
                .push(button("Cancel").on_press(Message::CancelAgent));
        }
    }

    let current_task_display = if let Some(ref task_id) = current_task {
        text(format!("Current task: {}", task_id))
    } else {
        text("No task selected")
    };

    // Show active tag and config info
    let tag_info = if let Some(ref tag) = active_tag {
        text(format!("Active tag: {}", tag))
    } else {
        text(format!(
            "No tag selected (using default: {})",
            swarm_defaults.default_tag
        ))
    };

    // Show swarm config summary
    let config_info = text(format!(
        "Harness: {} | Round size: {}",
        swarm_defaults.harness, swarm_defaults.round_size
    ))
    .size(12);

    column![
        text(status_text).size(18),
        current_task_display,
        tag_info,
        config_info,
        controls
    ]
    .spacing(15)
    .into()
}
