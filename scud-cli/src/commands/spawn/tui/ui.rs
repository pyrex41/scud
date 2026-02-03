//! UI rendering for TUI monitor
//!
//! Three-panel design:
//! - Top: Waves/Tasks panel showing tasks by execution wave
//! - Middle: Agents panel showing running agents with status filtering
//! - Bottom: Live terminal output from selected agent with scrolling
//!
//! Integrates ported components from components/ for enhanced functionality:
//! - StreamingView: Rich terminal output with scrolling and line types
//! - AgentSelector: Agent list with filtering and selection
//! - Theme system: JSON-based themes for consistent styling
//!
//! Minimalist Zen aesthetic with calm colors and clean typography.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
};

use super::app::{App, FocusedPanel, ViewMode};
use super::components::{
    AgentDisplayStatus, AgentInfo, AgentSelector, AgentSelectorState, StreamingView,
    StreamingViewState,
};
use super::header::{render_fullscreen_header, render_header};
use super::theme::*;
use super::waves::render_waves_panel;

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

fn render_fullscreen_view(frame: &mut Frame, area: Rect, app: &mut App) {
    // Fullscreen: small header + terminal + small footer
    let [header_area, terminal_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .areas(area);

    render_fullscreen_header(frame, header_area, app);
    // Use integrated StreamingView component for fullscreen terminal output
    render_output_panel(frame, terminal_area, app, true);
    render_fullscreen_footer(frame, footer_area);
}

fn render_input_view(frame: &mut Frame, area: Rect, app: &mut App) {
    // Input view: header + terminal + input bar + footer
    let [header_area, terminal_area, input_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(area);

    render_fullscreen_header(frame, header_area, app);
    // Use integrated StreamingView component for terminal output in input mode
    render_output_panel(frame, terminal_area, app, true);
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

// ─────────────────────────────────────────────────────────────────────────────
// Integrated Component Renderers
//
// These functions use the ported components from components/ for enhanced
// functionality including StatefulWidget patterns, filtering, and rich styling.
// ─────────────────────────────────────────────────────────────────────────────

/// Render the output panel using the StreamingViewStrings component
///
/// Uses the ported StreamingView component for enhanced terminal output rendering
/// with proper scrollbar support and line styling.
fn render_output_panel(frame: &mut Frame, area: Rect, app: &mut App, fullscreen: bool) {
    let is_focused = app.focused_panel == FocusedPanel::Output || fullscreen;

    // Create title based on context
    let title = if fullscreen {
        " Terminal (Esc to exit) ".to_string()
    } else if let Some(agent) = app.selected_agent() {
        format!(" Output: {} ", agent.task_id)
    } else {
        " Live Output ".to_string()
    };

    // Create streaming view state from app state
    let mut view_state = StreamingViewState::new();
    view_state.scroll_offset = app.scroll_offset;
    view_state.auto_scroll = app.auto_scroll;
    view_state.set_total_lines(app.live_output.len());

    // Create and render the streaming view widget
    let streaming_view = StreamingView::from_strings(&app.live_output)
        .focused(is_focused)
        .title(title)
        .show_scrollbar(true)
        .fullscreen(fullscreen);

    frame.render_stateful_widget(streaming_view, area, &mut view_state);

    // Sync state back to app (for scroll position updates)
    app.scroll_offset = view_state.scroll_offset;
    app.auto_scroll = view_state.auto_scroll;
}

/// Render the agents panel using the AgentSelector component
///
/// Uses the ported AgentSelector component for enhanced agent display
/// with filtering support and rich status indicators.
fn render_agents_panel_v2(frame: &mut Frame, area: Rect, app: &mut App) {
    use crate::commands::spawn::monitor::AgentStatus;

    let is_focused = app.focused_panel == FocusedPanel::Agents;

    // Convert app agents to AgentInfo format for the component
    let agents: Vec<AgentInfo> = app
        .agents()
        .iter()
        .map(|agent| {
            let status = match agent.status {
                AgentStatus::Starting => AgentDisplayStatus::Starting,
                AgentStatus::Running => AgentDisplayStatus::Running,
                AgentStatus::Completed => AgentDisplayStatus::Completed,
                AgentStatus::Failed => AgentDisplayStatus::Failed,
            };
            AgentInfo::new(&agent.task_id, &agent.task_id, status)
                .with_task_title(&agent.task_title)
        })
        .collect();

    // Create selector state from app state
    let mut selector_state = AgentSelectorState::new(app.selected);
    selector_state.offset = app.agents_scroll_offset;

    // Calculate visible height for scroll adjustment
    let inner_height = area.height.saturating_sub(3) as usize;
    selector_state.adjust_scroll(inner_height);

    // Render using the AgentSelector component
    let selector = AgentSelector::new(&agents)
        .focused(is_focused)
        .compact(false);

    frame.render_stateful_widget(selector, area, &mut selector_state);

    // Sync state back to app
    app.agents_scroll_offset = selector_state.offset;
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

    // Render each panel using the integrated components
    // Waves panel - uses existing render_waves_panel (no equivalent ported component)
    render_waves_panel(frame, waves_area, app);

    // Agents panel - uses integrated AgentSelector component for enhanced display
    render_agents_panel_v2(frame, agents_area, app);

    // Output panel - uses integrated StreamingView component for rich output
    render_output_panel(frame, output_area, app, false);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    // Context-sensitive help based on focused panel
    let ralph_hint = if app.ralph_mode {
        "R Ralph OFF"
    } else {
        "R Ralph"
    };
    let help_text = match app.focused_panel {
        FocusedPanel::Waves => format!(" Tab Panel  ·  j/k Navigate  ·  Space Select  ·  a All  ·  s Spawn  ·  W Swarm  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
        FocusedPanel::Agents => format!(" Tab Panel  ·  j/k Navigate  ·  d Done  ·  p Pending  ·  b Blocked  ·  W Swarm  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
        FocusedPanel::Output => format!(" Tab Panel  ·  ↑↓ Scroll  ·  G Bottom  ·  Enter Fullscreen  ·  W Swarm  ·  {}  ·  ? Help  ·  q Quit ", ralph_hint),
    };

    let mut line = Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(TEXT_MUTED),
    )]);

    // Show error if present
    if let Some(ref error) = app.error {
        line = Line::from(vec![
            Span::styled(" ⚠ ", Style::default().fg(ERROR)),
            Span::styled(error.as_str(), Style::default().fg(ERROR)),
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
        Line::from(vec![
            Span::styled(" d ", Style::default().fg(ACCENT)),
            Span::styled("Done ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" p ", Style::default().fg(ACCENT)),
            Span::styled("Pending ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" b ", Style::default().fg(ACCENT)),
            Span::styled("Blocked", Style::default().fg(TEXT_PRIMARY)),
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
            Span::styled(" W ", Style::default().fg(ACCENT)),
            Span::styled("Start Swarm ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" ? ", Style::default().fg(ACCENT)),
            Span::styled("Help ", Style::default().fg(TEXT_PRIMARY)),
            Span::styled(" q ", Style::default().fg(ACCENT)),
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
        .title(
            Line::from(" Keybindings ")
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(BG_SECONDARY));

    let help_para = Paragraph::new(help_text).block(help_block);
    frame.render_widget(help_para, overlay_area);
}
