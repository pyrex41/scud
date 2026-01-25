//! Agents panel view for TUI monitor
//!
//! Displays running agents with their status and task information.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

use crate::commands::spawn::monitor::AgentStatus;

use super::app::{App, FocusedPanel};
use super::theme::*;

/// Render the agents panel showing running agents
pub fn render_agents_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let is_focused = app.focused_panel == FocusedPanel::Agents;
    let border_color = if is_focused {
        BORDER_ACTIVE
    } else {
        BORDER_DEFAULT
    };
    let title_color = if is_focused { ACCENT } else { TEXT_MUTED };

    // Get counts first before borrowing agents slice
    let total = app.agents().len();
    let running = app
        .agents()
        .iter()
        .filter(|a| a.status == AgentStatus::Running)
        .count();
    let selected = app.selected;

    // Calculate visible height for scroll
    let inner_height = area.height.saturating_sub(3) as usize; // borders + padding

    // Auto-adjust scroll to keep selected visible
    if selected < app.agents_scroll_offset {
        app.agents_scroll_offset = selected;
    } else if total > 0 && inner_height > 0 && selected >= app.agents_scroll_offset + inner_height {
        app.agents_scroll_offset = selected.saturating_sub(inner_height - 1);
    }

    let scroll_offset = app.agents_scroll_offset;

    // Show scroll indicator if there are more agents than visible
    let title = if total > inner_height && inner_height > 0 {
        let visible_end = (scroll_offset + inner_height).min(total);
        format!(
            " Agents ({} running / {} total) [{}-{}] ",
            running,
            total,
            scroll_offset + 1,
            visible_end
        )
    } else {
        format!(" Agents ({} running / {} total) ", running, total)
    };

    // Now borrow agents for rendering
    let agents = app.agents();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(title).fg(title_color))
        .style(Style::default().bg(BG_SECONDARY))
        .padding(Padding::new(1, 1, 0, 0));

    if agents.is_empty() {
        let empty_msg = Paragraph::new("No agents spawned yet")
            .style(Style::default().fg(TEXT_MUTED))
            .block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(inner_height.max(1))
        .map(|(i, agent)| {
            let is_selected = i == selected && is_focused;

            let status_icon = match agent.status {
                AgentStatus::Starting => ("◐", STATUS_STARTING),
                AgentStatus::Running => ("●", STATUS_RUNNING),
                AgentStatus::Completed => ("✓", STATUS_COMPLETED),
                AgentStatus::Failed => ("✗", STATUS_FAILED),
            };

            // Truncate title
            let max_len = 35;
            let title = if agent.task_title.len() > max_len {
                format!("{}…", &agent.task_title[..max_len - 1])
            } else {
                agent.task_title.clone()
            };

            let line = Line::from(vec![
                Span::styled(
                    if is_selected { "▸ " } else { "  " },
                    Style::default().fg(ACCENT),
                ),
                Span::styled(
                    format!("{} ", status_icon.0),
                    Style::default().fg(status_icon.1),
                ),
                Span::styled(
                    format!("{}: ", agent.task_id),
                    Style::default().fg(TEXT_MUTED),
                ),
                Span::styled(
                    title,
                    Style::default()
                        .fg(if is_selected { ACCENT } else { TEXT_PRIMARY })
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
