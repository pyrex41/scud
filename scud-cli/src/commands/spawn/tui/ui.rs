//! UI rendering for TUI monitor
//!
//! Three-panel design:
//! - Top: Waves/Tasks panel showing tasks by execution wave
//! - Middle: Agents panel showing running agents
//! - Bottom: Live terminal output from selected agent
//!
//! Minimalist Zen aesthetic with calm colors and clean typography.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

use crate::commands::spawn::monitor::AgentStatus;

use super::app::{App, FocusedPanel, ViewMode, WaveTaskState};

// ─────────────────────────────────────────────────────────────
// Color Palette: Zen minimalist
// ─────────────────────────────────────────────────────────────

const BG_PRIMARY: Color = Color::Rgb(15, 23, 42); // Deep slate
const BG_SECONDARY: Color = Color::Rgb(30, 41, 59); // Elevated surface
const BG_TERMINAL: Color = Color::Rgb(22, 22, 22); // Terminal black
const TEXT_PRIMARY: Color = Color::Rgb(226, 232, 240); // Soft white
const TEXT_MUTED: Color = Color::Rgb(100, 116, 139); // Subdued
const TEXT_TERMINAL: Color = Color::Rgb(200, 200, 200); // Terminal text
const BORDER_DEFAULT: Color = Color::Rgb(51, 65, 85); // Subtle border
const BORDER_ACTIVE: Color = Color::Rgb(96, 165, 250); // Active border
const ACCENT: Color = Color::Rgb(96, 165, 250); // Calm blue
const STATUS_STARTING: Color = Color::Rgb(148, 163, 184); // Gray
const STATUS_RUNNING: Color = Color::Rgb(34, 197, 94); // Green
const STATUS_COMPLETED: Color = Color::Rgb(96, 165, 250); // Blue
const STATUS_FAILED: Color = Color::Rgb(248, 113, 113); // Soft red

/// Main render function
pub fn render(frame: &mut Frame, app: &mut App) {
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

fn render_split_view(frame: &mut Frame, area: Rect, app: &mut App) {
    // Main layout: header, three panels, footer
    let [header_area, content_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(area);

    render_header(frame, header_area, app);
    render_three_panel_content(frame, content_area, app);
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

    let footer = Paragraph::new(Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(TEXT_MUTED),
    )]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(BG_PRIMARY));

    frame.render_widget(footer, area);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let (starting, running, completed, failed) = app.status_counts();

    // Ralph mode indicator
    let ralph_indicator = if app.ralph_mode {
        vec![
            Span::styled("  🔄 ", Style::default()),
            Span::styled(
                "RALPH ",
                Style::default().fg(Color::Rgb(255, 165, 0)).bold(),
            ),
        ]
    } else {
        vec![]
    };

    // Status line with legend labels
    let mut spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(&app.session_name, Style::default().fg(ACCENT).bold()),
    ];
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

fn render_fullscreen_header(frame: &mut Frame, area: Rect, app: &App) {
    let agent_name = app
        .selected_agent()
        .map(|a| format!("{}: {}", a.task_id, a.task_title))
        .unwrap_or_else(|| "No agent".to_string());

    let title = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(&agent_name, Style::default().fg(ACCENT).bold()),
    ]);

    let header = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER_ACTIVE))
            .style(Style::default().bg(BG_SECONDARY)),
    );

    frame.render_widget(header, area);
}

fn render_three_panel_content(frame: &mut Frame, area: Rect, app: &mut App) {
    // Three vertical panels: waves (top), agents (middle), output (bottom)
    // Dynamic sizing based on content
    let has_agents = !app.agents().is_empty();
    let has_waves = !app.waves.is_empty();

    let constraints = if has_waves && has_agents {
        vec![
            Constraint::Percentage(35), // Waves
            Constraint::Percentage(25), // Agents
            Constraint::Percentage(40), // Output
        ]
    } else if has_waves {
        vec![
            Constraint::Percentage(50), // Waves
            Constraint::Length(3),      // Agents (minimal)
            Constraint::Percentage(50), // Output
        ]
    } else if has_agents {
        vec![
            Constraint::Length(3),      // Waves (minimal)
            Constraint::Percentage(40), // Agents
            Constraint::Percentage(60), // Output
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ]
    };

    let [waves_area, agents_area, output_area] = Layout::vertical(constraints).areas(area);

    render_waves_panel(frame, waves_area, app);
    render_agents_panel(frame, agents_area, app);
    render_terminal_output(frame, output_area, app, false);
}

fn render_waves_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focused_panel == FocusedPanel::Waves;
    let border_color = if is_focused {
        BORDER_ACTIVE
    } else {
        BORDER_DEFAULT
    };
    let title_color = if is_focused { ACCENT } else { TEXT_MUTED };

    let ready_count = app.ready_task_count();
    let selected_count = app.selected_task_count();
    let title = if selected_count > 0 {
        format!(
            " Waves & Tasks ({} selected / {} ready) ",
            selected_count, ready_count
        )
    } else {
        format!(" Waves & Tasks ({} ready) ", ready_count)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(title).fg(title_color))
        .style(Style::default().bg(BG_SECONDARY))
        .padding(Padding::new(1, 1, 0, 0));

    if app.waves.is_empty() {
        let empty_msg = Paragraph::new("No actionable tasks")
            .style(Style::default().fg(TEXT_MUTED))
            .block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    // Build list items from waves
    let mut all_items: Vec<ListItem> = Vec::new();
    let mut task_index = 0;

    for wave in &app.waves {
        // Wave header
        let ready_in_wave = wave
            .tasks
            .iter()
            .filter(|t| t.state == WaveTaskState::Ready)
            .count();
        let wave_header = Line::from(vec![
            Span::styled(
                format!("Wave {} ", wave.number),
                Style::default().fg(ACCENT).bold(),
            ),
            Span::styled(
                format!("({} tasks, {} ready)", wave.tasks.len(), ready_in_wave),
                Style::default().fg(TEXT_MUTED),
            ),
        ]);
        all_items.push(ListItem::new(wave_header));

        // Tasks in wave
        for task in &wave.tasks {
            let is_selected_in_list = task_index == app.wave_task_index && is_focused;
            let is_selected_for_spawn = app.selected_tasks.contains(&task.id);

            let state_icon = match task.state {
                WaveTaskState::Ready => ("○", STATUS_COMPLETED), // Blue circle = ready
                WaveTaskState::Running => ("●", STATUS_RUNNING), // Green filled = running
                WaveTaskState::Done => ("✓", STATUS_COMPLETED),  // Blue check = done
                WaveTaskState::Blocked => ("◌", TEXT_MUTED),     // Hollow = blocked
                WaveTaskState::InProgress => ("◐", STATUS_RUNNING), // Half = in progress
            };

            let checkbox = if is_selected_for_spawn {
                "[x]"
            } else if task.state == WaveTaskState::Ready {
                "[ ]"
            } else {
                "   "
            };

            // Truncate title
            let max_len = 40;
            let title_display = if task.title.len() > max_len {
                format!("{}…", &task.title[..max_len - 1])
            } else {
                task.title.clone()
            };

            let complexity = if task.complexity > 0 {
                format!(" [{}]", task.complexity)
            } else {
                String::new()
            };

            let line = Line::from(vec![
                Span::styled(
                    if is_selected_in_list { "▸ " } else { "  " },
                    Style::default().fg(ACCENT),
                ),
                Span::styled(
                    format!("{} ", checkbox),
                    Style::default().fg(if is_selected_for_spawn {
                        ACCENT
                    } else {
                        TEXT_MUTED
                    }),
                ),
                Span::styled(
                    format!("{} ", state_icon.0),
                    Style::default().fg(state_icon.1),
                ),
                Span::styled(format!("{} ", task.id), Style::default().fg(TEXT_MUTED)),
                Span::styled(
                    title_display,
                    Style::default()
                        .fg(if is_selected_in_list {
                            ACCENT
                        } else {
                            TEXT_PRIMARY
                        })
                        .add_modifier(if is_selected_in_list {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(complexity, Style::default().fg(TEXT_MUTED)),
            ]);

            all_items.push(ListItem::new(line));
            task_index += 1;
        }
    }

    // Apply scroll offset - skip first N items
    let visible_items: Vec<ListItem> = all_items.into_iter().skip(app.wave_scroll_offset).collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

fn render_agents_panel(frame: &mut Frame, area: Rect, app: &mut App) {
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

fn render_terminal_output(frame: &mut Frame, area: Rect, app: &App, fullscreen: bool) {
    let is_focused = app.focused_panel == FocusedPanel::Output || fullscreen;

    let title = if fullscreen {
        " Terminal (Esc to exit) ".to_string()
    } else if let Some(agent) = app.selected_agent() {
        format!(" Output: {} ", agent.task_id)
    } else {
        " Live Output ".to_string()
    };

    let border_color = if is_focused {
        BORDER_ACTIVE
    } else {
        BORDER_DEFAULT
    };
    let title_color = if is_focused { ACCENT } else { TEXT_MUTED };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(title).fg(title_color))
        .style(Style::default().bg(BG_TERMINAL))
        .padding(Padding::new(1, 0, 0, 0)); // Left padding only, scrollbar uses right side

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

    // Reserve 2 chars on right: 1 for scrollbar, 1 for spacing
    let text_width = inner.width.saturating_sub(2);
    let text_area = Rect::new(inner.x, inner.y, text_width, inner.height);

    let visible_lines: Vec<Line> = output
        .iter()
        .skip(start_idx)
        .take(visible_height)
        .map(|line| {
            Line::from(Span::styled(
                line.as_str(),
                Style::default().fg(TEXT_TERMINAL),
            ))
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, text_area);

    // Scrollbar on the right side of inner area (before the border)
    if total_lines > visible_height {
        let scrollbar_area = Rect::new(
            inner.x + inner.width.saturating_sub(1),
            inner.y,
            1,
            inner.height,
        );

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some(" "))
            .thumb_symbol("▐");

        let mut scrollbar_state = ScrollbarState::new(total_lines).position(start_idx);

        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    // Context-sensitive help based on focused panel
    let ralph_hint = if app.ralph_mode {
        "R Ralph OFF"
    } else {
        "R Ralph"
    };
    let help_text = match app.focused_panel {
        FocusedPanel::Waves => format!(" Tab Panel  ·  j/k Navigate  ·  Space Select  ·  a All  ·  s Spawn  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
        FocusedPanel::Agents => format!(" Tab Panel  ·  j/k Navigate  ·  Enter View  ·  i Input  ·  x Stop  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
        FocusedPanel::Output => format!(" Tab Panel  ·  ↑↓ Scroll  ·  G Bottom  ·  Enter Fullscreen  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
    };

    let mut line = Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(TEXT_MUTED),
    )]);

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

    let footer = Paragraph::new(Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(TEXT_MUTED),
    )]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(BG_PRIMARY));

    frame.render_widget(footer, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let overlay_width = 55.min(area.width.saturating_sub(4));
    let overlay_height = 22.min(area.height.saturating_sub(2)); // Leave margin
    let x = (area.width.saturating_sub(overlay_width)) / 2;
    let y = (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    frame.render_widget(Clear, overlay_area);

    let mode_hint = match app.view_mode {
        ViewMode::Split => "Three-Panel",
        ViewMode::Fullscreen => "Fullscreen",
        ViewMode::Input => "Input Mode",
    };

    let panel_hint = match app.focused_panel {
        FocusedPanel::Waves => "Waves",
        FocusedPanel::Agents => "Agents",
        FocusedPanel::Output => "Output",
    };

    let help_text = vec![
        Line::from(vec![
            Span::styled(" Tab ", Style::default().fg(ACCENT)),
            Span::styled("Panel ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" j/k ", Style::default().fg(ACCENT)),
            Span::styled("Navigate ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" r ", Style::default().fg(ACCENT)),
            Span::styled("Refresh", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Waves:", Style::default().fg(TEXT_MUTED))),
        Line::from(vec![
            Span::styled(" Space ", Style::default().fg(ACCENT)),
            Span::styled("Select ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" a ", Style::default().fg(ACCENT)),
            Span::styled("All ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" c ", Style::default().fg(ACCENT)),
            Span::styled("Clear ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" s ", Style::default().fg(ACCENT)),
            Span::styled("Spawn", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Agents:", Style::default().fg(TEXT_MUTED))),
        Line::from(vec![
            Span::styled(" Enter ", Style::default().fg(ACCENT)),
            Span::styled("View ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" i ", Style::default().fg(ACCENT)),
            Span::styled("Input ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" x ", Style::default().fg(ACCENT)),
            Span::styled("Stop", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" Output:", Style::default().fg(TEXT_MUTED))),
        Line::from(vec![
            Span::styled(" ↑/↓ ", Style::default().fg(ACCENT)),
            Span::styled("Scroll ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" G ", Style::default().fg(ACCENT)),
            Span::styled("Bottom ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" Enter ", Style::default().fg(ACCENT)),
            Span::styled("Fullscreen", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" ? ", Style::default().fg(ACCENT)),
            Span::styled("Help ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" q/Esc ", Style::default().fg(ACCENT)),
            Span::styled("Quit", Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!(" Mode: {} | Panel: {}", mode_hint, panel_hint),
            Style::default().fg(TEXT_MUTED),
        )]),
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
