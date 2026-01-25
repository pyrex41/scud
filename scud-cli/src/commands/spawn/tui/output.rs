//! Terminal output panel view for TUI monitor
//!
//! Displays live terminal output from the selected agent with scrolling support.

use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Frame,
};

use super::app::{App, FocusedPanel};
use super::theme::*;

/// Render the terminal output panel
pub fn render_terminal_output(frame: &mut Frame, area: Rect, app: &App, fullscreen: bool) {
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
