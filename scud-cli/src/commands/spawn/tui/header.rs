//! Header view components for TUI monitor

use ratatui::{
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
    Frame,
};

use super::app::App;
use super::theme::*;

/// Render the main header bar with session name and status counts
pub fn render_header(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let (starting, running, completed, failed) = app.status_counts();

    // Swarm mode indicator
    let swarm_indicator = if app.swarm_mode {
        vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "SWARM ",
                Style::default()
                    .fg(SWARM_PURPLE)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![]
    };

    // Ralph mode indicator
    let ralph_indicator = if app.ralph_mode {
        vec![
            Span::styled("  🔄 ", Style::default()),
            Span::styled(
                "RALPH ",
                Style::default()
                    .fg(RALPH_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![]
    };

    // Status line with legend labels
    let mut spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            &app.session_name,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
    ];
    spans.extend(swarm_indicator);
    spans.extend(ralph_indicator);
    spans.extend(vec![
        Span::styled("    ", Style::default()),
        // Gray = Starting/Waiting
        Span::styled("◉ ", Style::default().fg(STATUS_STARTING)),
        Span::styled("Starting ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(
            format!("{}  ", starting),
            Style::default().fg(STATUS_STARTING),
        ),
        // Green = Running
        Span::styled("◉ ", Style::default().fg(STATUS_RUNNING)),
        Span::styled("Running ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(
            format!("{}  ", running),
            Style::default().fg(STATUS_RUNNING),
        ),
        // Blue = Completed
        Span::styled("◉ ", Style::default().fg(STATUS_COMPLETED)),
        Span::styled("Done ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(
            format!("{}  ", completed),
            Style::default().fg(STATUS_COMPLETED),
        ),
        // Red = Failed
        Span::styled("◉ ", Style::default().fg(STATUS_FAILED)),
        Span::styled("Failed ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(format!("{}", failed), Style::default().fg(STATUS_FAILED)),
    ]);
    let status_line = Line::from(spans);

    let header = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER_DEFAULT))
            .style(Style::default().bg(BG_SECONDARY))
            .padding(Padding::horizontal(1)),
    );

    frame.render_widget(header, area);
}

/// Render the fullscreen mode header (shows agent name)
pub fn render_fullscreen_header(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let agent_name = app
        .selected_agent()
        .map(|a| format!("{}: {}", a.task_id, a.task_title))
        .unwrap_or_else(|| "No agent".to_string());

    let title = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            &agent_name,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
    ]);

    let header = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .style(Style::default().bg(BG_SECONDARY)),
    );

    frame.render_widget(header, area);
}
