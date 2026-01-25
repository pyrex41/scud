//! Streaming view component for TUI
//!
//! Provides a UI widget for displaying streaming agent output with ANSI support,
//! scrolling, and live updates. Designed for showing terminal output from agents.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, StatefulWidget, Widget, Wrap,
    },
};

use super::super::theme::*;

/// Output line with optional styling
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The text content of the line
    pub text: String,
    /// Line type for different styling
    pub line_type: OutputLineType,
    /// Timestamp when line was added (for live updates)
    pub timestamp: Option<u64>,
}

impl OutputLine {
    /// Create a new output line
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            line_type: OutputLineType::Normal,
            timestamp: None,
        }
    }

    /// Set line type
    pub fn with_type(mut self, line_type: OutputLineType) -> Self {
        self.line_type = line_type;
        self
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = Some(ts);
        self
    }
}

impl From<String> for OutputLine {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for OutputLine {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Type of output line for styling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputLineType {
    /// Regular output text
    #[default]
    Normal,
    /// Error or warning text
    Error,
    /// Success/completion text
    Success,
    /// System/info message
    System,
    /// User input echo
    Input,
    /// Prompt line (e.g., "$ " or ">" prefix)
    Prompt,
}

impl OutputLineType {
    /// Get the color for this line type
    pub fn color(&self) -> ratatui::style::Color {
        match self {
            Self::Normal => TEXT_TERMINAL,
            Self::Error => ERROR,
            Self::Success => SUCCESS,
            Self::System => TEXT_MUTED,
            Self::Input => ACCENT,
            Self::Prompt => TEXT_MUTED,
        }
    }
}

/// State for the streaming view widget
#[derive(Debug, Default)]
pub struct StreamingViewState {
    /// Scroll offset (0 = bottom/most recent, higher = scrolled up)
    pub scroll_offset: usize,
    /// Whether to auto-scroll on new content
    pub auto_scroll: bool,
    /// Total number of lines (for scrollbar calculation)
    total_lines: usize,
}

impl StreamingViewState {
    /// Create new state with auto-scroll enabled
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            auto_scroll: true,
            total_lines: 0,
        }
    }

    /// Scroll up (show older content)
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.total_lines.saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        self.auto_scroll = false;
    }

    /// Scroll down (show newer content)
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// Jump to bottom (most recent output)
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Jump to top (oldest output)
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.total_lines.saturating_sub(1);
        self.auto_scroll = false;
    }

    /// Update total lines count (call before rendering)
    pub fn set_total_lines(&mut self, total: usize) {
        self.total_lines = total;
        // Clamp scroll offset if content shrunk
        if self.scroll_offset > 0 && self.scroll_offset >= total {
            self.scroll_offset = total.saturating_sub(1);
        }
    }

    /// Check if scrolled to bottom
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }
}

/// Streaming view widget for TUI
///
/// Displays streaming output with scrolling support and ANSI-aware rendering.
/// Designed for showing live terminal output from agents.
pub struct StreamingView<'a> {
    /// Output lines to display
    lines: &'a [OutputLine],
    /// Whether this widget is focused
    focused: bool,
    /// Title for the view
    title: Option<String>,
    /// Show scrollbar
    show_scrollbar: bool,
    /// Whether this is fullscreen mode
    fullscreen: bool,
}

impl<'a> StreamingView<'a> {
    /// Create a new streaming view with the given output lines
    pub fn new(lines: &'a [OutputLine]) -> Self {
        Self {
            lines,
            focused: false,
            title: None,
            show_scrollbar: true,
            fullscreen: false,
        }
    }

    /// Create from raw string lines
    pub fn from_strings(lines: &'a [String]) -> StreamingViewStrings<'a> {
        StreamingViewStrings {
            lines,
            focused: false,
            title: None,
            show_scrollbar: true,
            fullscreen: false,
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set custom title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set scrollbar visibility
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set fullscreen mode (changes styling)
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }
}

impl StatefulWidget for StreamingView<'_> {
    type State = StreamingViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Update state with total lines
        state.set_total_lines(self.lines.len());

        let border_color = if self.focused {
            BORDER_ACTIVE
        } else {
            BORDER_DEFAULT
        };
        let title_color = if self.focused { ACCENT } else { TEXT_MUTED };

        let title = if self.fullscreen {
            self.title
                .unwrap_or_else(|| " Output (Esc to exit) ".to_string())
        } else {
            self.title.unwrap_or_else(|| " Live Output ".to_string())
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(title).fg(title_color))
            .style(Style::default().bg(BG_TERMINAL))
            .padding(Padding::new(1, 0, 0, 0));

        let inner = block.inner(area);
        Widget::render(block, area, buf);

        let visible_height = inner.height as usize;
        let total_lines = self.lines.len();

        // Calculate visible window based on scroll offset
        let end_idx = total_lines.saturating_sub(state.scroll_offset);
        let start_idx = end_idx.saturating_sub(visible_height);

        // Reserve space for scrollbar
        let text_width = if self.show_scrollbar && total_lines > visible_height {
            inner.width.saturating_sub(2)
        } else {
            inner.width
        };
        let text_area = Rect::new(inner.x, inner.y, text_width, inner.height);

        // Render visible lines
        let visible_lines: Vec<Line> = self
            .lines
            .iter()
            .skip(start_idx)
            .take(visible_height)
            .map(|line| {
                Line::from(Span::styled(
                    line.text.as_str(),
                    Style::default().fg(line.line_type.color()),
                ))
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines);
        Widget::render(paragraph, text_area, buf);

        // Render scrollbar
        if self.show_scrollbar && total_lines > visible_height {
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
            StatefulWidget::render(scrollbar, scrollbar_area, buf, &mut scrollbar_state);
        }
    }
}

/// String-based streaming view (simpler API for raw string output)
pub struct StreamingViewStrings<'a> {
    lines: &'a [String],
    focused: bool,
    title: Option<String>,
    show_scrollbar: bool,
    fullscreen: bool,
}

impl<'a> StreamingViewStrings<'a> {
    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set custom title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set scrollbar visibility
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set fullscreen mode
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }
}

impl StatefulWidget for StreamingViewStrings<'_> {
    type State = StreamingViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Update state with total lines
        state.set_total_lines(self.lines.len());

        let border_color = if self.focused {
            BORDER_ACTIVE
        } else {
            BORDER_DEFAULT
        };
        let title_color = if self.focused { ACCENT } else { TEXT_MUTED };

        let title = if self.fullscreen {
            self.title
                .unwrap_or_else(|| " Output (Esc to exit) ".to_string())
        } else {
            self.title.unwrap_or_else(|| " Live Output ".to_string())
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(title).fg(title_color))
            .style(Style::default().bg(BG_TERMINAL))
            .padding(Padding::new(1, 0, 0, 0));

        let inner = block.inner(area);
        Widget::render(block, area, buf);

        let visible_height = inner.height as usize;
        let total_lines = self.lines.len();

        // Calculate visible window
        let end_idx = total_lines.saturating_sub(state.scroll_offset);
        let start_idx = end_idx.saturating_sub(visible_height);

        // Reserve space for scrollbar
        let text_width = if self.show_scrollbar && total_lines > visible_height {
            inner.width.saturating_sub(2)
        } else {
            inner.width
        };
        let text_area = Rect::new(inner.x, inner.y, text_width, inner.height);

        // Render visible lines
        let visible_lines: Vec<Line> = self
            .lines
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
        Widget::render(paragraph, text_area, buf);

        // Render scrollbar
        if self.show_scrollbar && total_lines > visible_height {
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
            StatefulWidget::render(scrollbar, scrollbar_area, buf, &mut scrollbar_state);
        }
    }
}

/// Simple output display (no scrolling state needed)
pub struct OutputDisplay<'a> {
    text: &'a str,
    focused: bool,
    title: Option<String>,
    wrap: bool,
}

impl<'a> OutputDisplay<'a> {
    /// Create a new output display with the given text
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            focused: false,
            title: None,
            wrap: true,
        }
    }

    /// Set focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set custom title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set word wrapping
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
}

impl Widget for OutputDisplay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.focused {
            BORDER_ACTIVE
        } else {
            BORDER_DEFAULT
        };
        let title_color = if self.focused { ACCENT } else { TEXT_MUTED };

        let title = self.title.unwrap_or_else(|| " Output ".to_string());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(title).fg(title_color))
            .style(Style::default().bg(BG_TERMINAL))
            .padding(Padding::horizontal(1));

        let mut paragraph = Paragraph::new(self.text)
            .style(Style::default().fg(TEXT_TERMINAL))
            .block(block);

        if self.wrap {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }

        Widget::render(paragraph, area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_line_creation() {
        let line = OutputLine::new("Hello world")
            .with_type(OutputLineType::Success)
            .with_timestamp(12345);

        assert_eq!(line.text, "Hello world");
        assert_eq!(line.line_type, OutputLineType::Success);
        assert_eq!(line.timestamp, Some(12345));
    }

    #[test]
    fn test_output_line_from_string() {
        let line: OutputLine = "Test line".into();
        assert_eq!(line.text, "Test line");
        assert_eq!(line.line_type, OutputLineType::Normal);
    }

    #[test]
    fn test_streaming_view_state_scrolling() {
        let mut state = StreamingViewState::new();
        state.set_total_lines(100);

        // Initially at bottom with auto-scroll
        assert!(state.is_at_bottom());
        assert!(state.auto_scroll);

        // Scroll up disables auto-scroll
        state.scroll_up(10);
        assert_eq!(state.scroll_offset, 10);
        assert!(!state.auto_scroll);

        // Scroll down towards bottom
        state.scroll_down(5);
        assert_eq!(state.scroll_offset, 5);
        assert!(!state.auto_scroll);

        // Scroll to bottom re-enables auto-scroll
        state.scroll_to_bottom();
        assert!(state.is_at_bottom());
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_scroll_clamping() {
        let mut state = StreamingViewState::new();
        state.set_total_lines(50);

        // Scroll past max should clamp
        state.scroll_up(100);
        assert_eq!(state.scroll_offset, 49); // max is total - 1

        // Content shrinking should clamp
        state.set_total_lines(30);
        assert_eq!(state.scroll_offset, 29);
    }

    #[test]
    fn test_line_type_colors() {
        // Just verify colors don't panic
        let _ = OutputLineType::Normal.color();
        let _ = OutputLineType::Error.color();
        let _ = OutputLineType::Success.color();
        let _ = OutputLineType::System.color();
        let _ = OutputLineType::Input.color();
        let _ = OutputLineType::Prompt.color();
    }
}
