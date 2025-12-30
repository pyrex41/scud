//! UI rendering for TUI monitor
//!
//! Split-view design:
//! - Left panel: Agent list with status indicators
//! - Right panel: Live terminal output from selected agent
//! - Fullscreen mode: Just the terminal output
//!
//! Minimalist Zen aesthetic with calm colors and clean typography.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::commands::spawn::monitor::AgentStatus;

use super::app::{App, ViewMode};

// ─────────────────────────────────────────────────────────────
// Color Palette: Zen minimalist
// ─────────────────────────────────────────────────────────────

const BG_PRIMARY: Color = Color::Rgb(15, 23, 42);      // Deep slate
const BG_SECONDARY: Color = Color::Rgb(30, 41, 59);    // Elevated surface
const BG_TERMINAL: Color = Color::Rgb(22, 22, 22);     // Terminal black
const TEXT_PRIMARY: Color = Color::Rgb(226, 232, 240); // Soft white
const TEXT_MUTED: Color = Color::Rgb(100, 116, 139);   // Subdued
const TEXT_TERMINAL: Color = Color::Rgb(200, 200, 200); // Terminal text
const BORDER_DEFAULT: Color = Color::Rgb(51, 65, 85);  // Subtle border
const BORDER_ACTIVE: Color = Color::Rgb(96, 165, 250); // Active border
const ACCENT: Color = Color::Rgb(96, 165, 250);        // Calm blue
const STATUS_STARTING: Color = Color::Rgb(148, 163, 184); // Gray
const STATUS_RUNNING: Color = Color::Rgb(34, 197, 94);    // Green
const STATUS_COMPLETED: Color = Color::Rgb(96, 165, 250); // Blue
const STATUS_FAILED: Color = Color::Rgb(248, 113, 113);   // Soft red

/// Main render function
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Fill background
    frame.render_widget(
        Block::default().style(Style::default().bg(BG_PRIMARY)),
        area,
    );

    match app.view_mode {
        ViewMode::Split => render_split_view(frame, area, app),
        ViewMode::Fullscreen => render_fullscreen_view(frame, area, app),
        ViewMode::Input => render_input_view(frame, area, app),
    }

    // Help overlay (on top of everything)
    if app.show_help {
        render_help_overlay(frame, area, app);
    }
}

fn render_split_view(frame: &mut Frame, area: Rect, app: &App) {
    // Main layout: header, content, footer
    let [header_area, content_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(area);

    render_header(frame, header_area, app);
    render_split_content(frame, content_area, app);
    render_footer(frame, footer_area, app);
}

fn render_fullscreen_view(frame: &mut Frame, area: Rect, app: &App) {
    // Fullscreen: small header + terminal + small footer
    let [header_area, terminal_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(area);

    render_fullscreen_header(frame, header_area, app);
    render_terminal_output(frame, terminal_area, app, true);
    render_fullscreen_footer(frame, footer_area);
}

fn render_input_view(frame: &mut Frame, area: Rect, app: &App) {
    // Input view: header + terminal + input bar + footer
    let [header_area, terminal_area, input_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(area);

    render_fullscreen_header(frame, header_area, app);
    render_terminal_output(frame, terminal_area, app, true);
    render_input_bar(frame, input_area, app);
    render_input_footer(frame, footer_area);
}

fn render_input_bar(frame: &mut Frame, area: Rect, app: &App) {
    let input_text = format!("▸ {}", app.input_buffer);

    let input = Paragraph::new(Line::from(vec![
        Span::styled(&input_text, Style::default().fg(TEXT_PRIMARY)),
        Span::styled("█", Style::default().fg(ACCENT)), // Cursor
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Line::from(" Send to Agent ").fg(ACCENT))
            .style(Style::default().bg(BG_SECONDARY))
            .padding(Padding::horizontal(1)),
    );

    frame.render_widget(input, area);
}

fn render_input_footer(frame: &mut Frame, area: Rect) {
    let help_text = " Enter Send  ·  Esc Cancel  ·  Type your message... ";

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(help_text, Style::default().fg(TEXT_MUTED)),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(BG_PRIMARY));

    frame.render_widget(footer, area);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let (starting, running, completed, failed) = app.status_counts();

    // Status line with legend labels
    let status_line = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(&app.session_name, Style::default().fg(ACCENT).bold()),
        Span::styled("    ", Style::default()),
        // Gray = Starting/Waiting
        Span::styled("◉ ", Style::default().fg(STATUS_STARTING)),
        Span::styled("Starting ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(format!("{}  ", starting), Style::default().fg(STATUS_STARTING)),
        // Green = Running
        Span::styled("◉ ", Style::default().fg(STATUS_RUNNING)),
        Span::styled("Running ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(format!("{}  ", running), Style::default().fg(STATUS_RUNNING)),
        // Blue = Completed
        Span::styled("◉ ", Style::default().fg(STATUS_COMPLETED)),
        Span::styled("Done ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(format!("{}  ", completed), Style::default().fg(STATUS_COMPLETED)),
        // Red = Failed
        Span::styled("◉ ", Style::default().fg(STATUS_FAILED)),
        Span::styled("Failed ", Style::default().fg(TEXT_MUTED).dim()),
        Span::styled(format!("{}", failed), Style::default().fg(STATUS_FAILED)),
    ]);

    let header = Paragraph::new(status_line)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(BORDER_DEFAULT))
                .style(Style::default().bg(BG_SECONDARY))
                .padding(Padding::horizontal(1)),
        );

    frame.render_widget(header, area);
}

fn render_fullscreen_header(frame: &mut Frame, area: Rect, app: &App) {
    let agent_name = app
        .selected_agent()
        .map(|a| format!("{}: {}", a.task_id, a.task_title))
        .unwrap_or_else(|| "No agent".to_string());

    let title = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(&agent_name, Style::default().fg(ACCENT).bold()),
    ]);

    let header = Paragraph::new(title)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(BORDER_ACTIVE))
                .style(Style::default().bg(BG_SECONDARY)),
        );

    frame.render_widget(header, area);
}

fn render_split_content(frame: &mut Frame, area: Rect, app: &App) {
    let agents = app.agents();

    if agents.is_empty() {
        let empty_msg = Paragraph::new("No agents in session")
            .alignment(Alignment::Center)
            .style(Style::default().fg(TEXT_MUTED));
        frame.render_widget(empty_msg, area);
        return;
    }

    // Split: 30% agent list, 70% live output
    let [list_area, output_area] = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ])
    .areas(area);

    render_agent_list(frame, list_area, app);
    render_terminal_output(frame, output_area, app, false);
}

fn render_agent_list(frame: &mut Frame, area: Rect, app: &App) {
    let agents = app.agents();

    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, agent)| {
            let is_selected = i == app.selected;

            let status_icon = match agent.status {
                AgentStatus::Starting => ("◐", STATUS_STARTING),
                AgentStatus::Running => ("●", STATUS_RUNNING),
                AgentStatus::Completed => ("✓", STATUS_COMPLETED),
                AgentStatus::Failed => ("✗", STATUS_FAILED),
            };

            // Truncate title to fit
            let max_len = 25;
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
                        .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DEFAULT))
                .title(Line::from(" Agents ").fg(TEXT_MUTED))
                .style(Style::default().bg(BG_SECONDARY))
                .padding(Padding::new(1, 1, 0, 0)),
        );

    frame.render_widget(list, area);
}

fn render_terminal_output(frame: &mut Frame, area: Rect, app: &App, fullscreen: bool) {
    let title = if fullscreen {
        " Terminal (Esc to exit) "
    } else {
        " Live Output (Enter for fullscreen) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if fullscreen { BORDER_ACTIVE } else { BORDER_DEFAULT }))
        .title(Line::from(title).fg(if fullscreen { ACCENT } else { TEXT_MUTED }))
        .style(Style::default().bg(BG_TERMINAL))
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render output lines
    let visible_height = inner.height as usize;
    let output = &app.live_output;

    // Calculate visible window based on scroll offset
    // scroll_offset=0 means bottom (most recent), higher = scrolled up
    let total_lines = output.len();
    let end_idx = total_lines.saturating_sub(app.scroll_offset);
    let start_idx = end_idx.saturating_sub(visible_height);

    let visible_lines: Vec<Line> = output
        .iter()
        .skip(start_idx)
        .take(visible_height)
        .map(|line| Line::from(Span::styled(line.as_str(), Style::default().fg(TEXT_TERMINAL))))
        .collect();

    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, inner);

    // Scrollbar
    if total_lines > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        let mut scrollbar_state = ScrollbarState::new(total_lines)
            .position(start_idx);

        frame.render_stateful_widget(
            scrollbar,
            inner,
            &mut scrollbar_state,
        );
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = " j/k Navigate  ·  Enter Fullscreen  ·  i Input  ·  x Stop  ·  ? Help  ·  q Quit ";

    let mut line = Line::from(vec![
        Span::styled(help_text, Style::default().fg(TEXT_MUTED)),
    ]);

    // Show error if present
    if let Some(ref error) = app.error {
        line = Line::from(vec![
            Span::styled(" ⚠ ", Style::default().fg(STATUS_FAILED)),
            Span::styled(error.as_str(), Style::default().fg(STATUS_FAILED)),
        ]);
    }

    let footer = Paragraph::new(line)
        .alignment(Alignment::Center)
        .style(Style::default().bg(BG_PRIMARY));

    frame.render_widget(footer, area);
}

fn render_fullscreen_footer(frame: &mut Frame, area: Rect) {
    let help_text = " ↑↓ Scroll  ·  j/k Switch  ·  G Bottom  ·  i Input  ·  Esc Back  ·  q Quit ";

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(help_text, Style::default().fg(TEXT_MUTED)),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(BG_PRIMARY));

    frame.render_widget(footer, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let overlay_width = 50;
    let overlay_height = 20;
    let x = (area.width.saturating_sub(overlay_width)) / 2;
    let y = (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    frame.render_widget(Clear, overlay_area);

    let mode_hint = match app.view_mode {
        ViewMode::Split => "Split View",
        ViewMode::Fullscreen => "Fullscreen",
        ViewMode::Input => "Input Mode",
    };

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  j / k     ", Style::default().fg(ACCENT)),
            Span::styled("Switch agent", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  ↑ / ↓     ", Style::default().fg(ACCENT)),
            Span::styled("Navigate (split) / Scroll (fullscreen)", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/Down ", Style::default().fg(ACCENT)),
            Span::styled("Scroll 10 lines", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  g / G     ", Style::default().fg(ACCENT)),
            Span::styled("Scroll to top / bottom", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter     ", Style::default().fg(ACCENT)),
            Span::styled("Toggle fullscreen", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  i         ", Style::default().fg(ACCENT)),
            Span::styled("Send input to agent", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  x         ", Style::default().fg(ACCENT)),
            Span::styled("Stop/interrupt agent", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  r         ", Style::default().fg(ACCENT)),
            Span::styled("Force refresh", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  ?         ", Style::default().fg(ACCENT)),
            Span::styled("Toggle this help", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc   ", Style::default().fg(ACCENT)),
            Span::styled("Quit / Exit fullscreen", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  Mode: {}", mode_hint), Style::default().fg(TEXT_MUTED)),
        ]),
        Line::from(""),
    ];

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Line::from(" Keybindings ").fg(ACCENT).bold())
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(BG_SECONDARY));

    let help_para = Paragraph::new(help_text).block(help_block);
    frame.render_widget(help_para, overlay_area);
}
