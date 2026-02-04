//! Streaming view component for GUI
//!
//! Provides a widget for displaying streaming agent output with scrolling
//! and live updates. Designed for showing terminal output from agents.

use iced::widget::{column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};

use crate::theme;

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
    pub fn color(&self) -> iced::Color {
        match self {
            Self::Normal => theme::text::PRIMARY,
            Self::Error => theme::ERROR,
            Self::Success => theme::SUCCESS,
            Self::System => theme::text::SECONDARY,
            Self::Input => theme::ACCENT,
            Self::Prompt => theme::text::MUTED,
        }
    }
}

/// State for the streaming view widget
#[derive(Debug, Clone, Default)]
pub struct StreamingViewState {
    /// Whether to auto-scroll on new content
    pub auto_scroll: bool,
    /// Whether the view is in fullscreen mode
    pub fullscreen: bool,
}

impl StreamingViewState {
    /// Create new state with auto-scroll enabled
    pub fn new() -> Self {
        Self {
            auto_scroll: true,
            fullscreen: false,
        }
    }

    /// Toggle auto-scroll
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }
}

/// Message type for streaming view events
#[derive(Debug, Clone)]
pub enum StreamingViewMessage {
    /// Toggle auto-scroll
    ToggleAutoScroll,
    /// Toggle fullscreen
    ToggleFullscreen,
    /// Clear output
    Clear,
}

/// Render a streaming view widget with OutputLine items
///
/// Returns an Element that displays streaming output with scrolling support.
pub fn view<'a, Message>(
    lines: &'a [OutputLine],
    state: &StreamingViewState,
    title: Option<&'a str>,
    on_toggle_auto_scroll: Message,
    on_clear: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let title_str = if state.fullscreen {
        title.unwrap_or("Output (Esc to exit)")
    } else {
        title.unwrap_or("Live Output")
    };

    let header = text(title_str.to_string()).size(18);

    // Auto-scroll toggle button
    let auto_scroll_label = if state.auto_scroll {
        "Auto-scroll: ON"
    } else {
        "Auto-scroll: OFF"
    };
    let auto_scroll_button = iced::widget::button(text(auto_scroll_label).size(12))
        .on_press(on_toggle_auto_scroll)
        .padding(4);

    // Clear button
    let clear_button = iced::widget::button(text("Clear").size(12))
        .on_press(on_clear)
        .padding(4);

    let header_row = row![header, auto_scroll_button, clear_button]
        .spacing(10)
        .align_y(Alignment::Center);

    // Build output content
    let mut output_column = Column::new().spacing(0);

    if lines.is_empty() {
        output_column =
            output_column.push(text("No output yet...").size(14).style(|_| text::Style {
                color: Some(theme::text::MUTED),
            }));
    } else {
        for line in lines {
            let line_text = text(&line.text).size(14).style(move |_| text::Style {
                color: Some(line.line_type.color()),
            });
            output_column = output_column.push(line_text);
        }
    }

    let output_container = container(output_column)
        .padding(10)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::TERTIARY)),
            ..Default::default()
        });

    let scrollable_output = scrollable(output_container).height(Length::Fill);

    let content = column![header_row, scrollable_output]
        .spacing(10)
        .padding(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::PRIMARY)),
            border: iced::Border {
                color: theme::text::MUTED,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Render a streaming view widget from raw strings
///
/// Simpler API for when you just have a slice of strings.
pub fn view_strings<'a, Message>(
    lines: &'a [String],
    state: &StreamingViewState,
    title: Option<&'a str>,
    on_toggle_auto_scroll: Message,
    on_clear: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let title_str = if state.fullscreen {
        title.unwrap_or("Output (Esc to exit)")
    } else {
        title.unwrap_or("Live Output")
    };

    let header = text(title_str.to_string()).size(18);

    // Auto-scroll toggle button
    let auto_scroll_label = if state.auto_scroll {
        "Auto-scroll: ON"
    } else {
        "Auto-scroll: OFF"
    };
    let auto_scroll_button = iced::widget::button(text(auto_scroll_label).size(12))
        .on_press(on_toggle_auto_scroll)
        .padding(4);

    // Clear button
    let clear_button = iced::widget::button(text("Clear").size(12))
        .on_press(on_clear)
        .padding(4);

    let header_row = row![header, auto_scroll_button, clear_button]
        .spacing(10)
        .align_y(Alignment::Center);

    // Build output content
    let mut output_column = Column::new().spacing(0);

    if lines.is_empty() {
        output_column =
            output_column.push(text("No output yet...").size(14).style(|_| text::Style {
                color: Some(theme::text::MUTED),
            }));
    } else {
        for line in lines {
            let line_text = text(line).size(14).style(|_| text::Style {
                color: Some(theme::text::PRIMARY),
            });
            output_column = output_column.push(line_text);
        }
    }

    let output_container = container(output_column)
        .padding(10)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::TERTIARY)),
            ..Default::default()
        });

    let scrollable_output = scrollable(output_container).height(Length::Fill);

    let content = column![header_row, scrollable_output]
        .spacing(10)
        .padding(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::PRIMARY)),
            border: iced::Border {
                color: theme::text::MUTED,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Simple output display (read-only, no controls)
pub fn output_display<'a>(text_content: &'a str, title: Option<&'a str>) -> Element<'a, ()> {
    let title_str = title.unwrap_or("Output");
    let header = text(title_str.to_string()).size(18);

    let content_text = if text_content.is_empty() {
        text("No output").size(14).style(|_| text::Style {
            color: Some(theme::text::MUTED),
        })
    } else {
        text(text_content).size(14).style(|_| text::Style {
            color: Some(theme::text::PRIMARY),
        })
    };

    let output_container = container(content_text)
        .padding(10)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::TERTIARY)),
            ..Default::default()
        });

    let scrollable_output = scrollable(output_container).height(Length::Fill);

    let content = column![header, scrollable_output].spacing(10).padding(10);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(theme::background::PRIMARY)),
            border: iced::Border {
                color: theme::text::MUTED,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Buffer for collecting streaming output
#[derive(Debug, Clone, Default)]
pub struct OutputBuffer {
    /// Collected lines
    lines: Vec<OutputLine>,
    /// Maximum number of lines to keep (0 = unlimited)
    max_lines: usize,
}

impl OutputBuffer {
    /// Create a new output buffer
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            max_lines: 0,
        }
    }

    /// Create a buffer with a maximum line limit
    pub fn with_max_lines(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }

    /// Push a new line to the buffer
    pub fn push(&mut self, line: OutputLine) {
        self.lines.push(line);
        if self.max_lines > 0 && self.lines.len() > self.max_lines {
            self.lines.remove(0);
        }
    }

    /// Push a string line
    pub fn push_str(&mut self, s: impl Into<String>) {
        self.push(OutputLine::new(s));
    }

    /// Push an error line
    pub fn push_error(&mut self, s: impl Into<String>) {
        self.push(OutputLine::new(s).with_type(OutputLineType::Error));
    }

    /// Push a success line
    pub fn push_success(&mut self, s: impl Into<String>) {
        self.push(OutputLine::new(s).with_type(OutputLineType::Success));
    }

    /// Push a system message line
    pub fn push_system(&mut self, s: impl Into<String>) {
        self.push(OutputLine::new(s).with_type(OutputLineType::System));
    }

    /// Get all lines
    pub fn lines(&self) -> &[OutputLine] {
        &self.lines
    }

    /// Clear all lines
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get line count
    pub fn len(&self) -> usize {
        self.lines.len()
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
    fn test_streaming_view_state() {
        let mut state = StreamingViewState::new();

        // Initially auto-scroll is enabled
        assert!(state.auto_scroll);

        // Toggle auto-scroll
        state.toggle_auto_scroll();
        assert!(!state.auto_scroll);

        // Toggle fullscreen
        assert!(!state.fullscreen);
        state.toggle_fullscreen();
        assert!(state.fullscreen);
    }

    #[test]
    fn test_output_buffer() {
        let mut buffer = OutputBuffer::new();
        assert!(buffer.is_empty());

        buffer.push_str("Line 1");
        buffer.push_error("Error line");
        buffer.push_success("Success line");
        buffer.push_system("System message");

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.lines()[0].text, "Line 1");
        assert_eq!(buffer.lines()[1].line_type, OutputLineType::Error);
        assert_eq!(buffer.lines()[2].line_type, OutputLineType::Success);
        assert_eq!(buffer.lines()[3].line_type, OutputLineType::System);

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_output_buffer_max_lines() {
        let mut buffer = OutputBuffer::with_max_lines(3);

        buffer.push_str("Line 1");
        buffer.push_str("Line 2");
        buffer.push_str("Line 3");
        buffer.push_str("Line 4");

        // Should have dropped the oldest line
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.lines()[0].text, "Line 2");
        assert_eq!(buffer.lines()[2].text, "Line 4");
    }
}
