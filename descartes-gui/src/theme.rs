//! Theme constants for the GUI
//!
//! Dark theme with accent colors for Descartes.

use iced::Color;

/// Primary accent color (blue)
pub const ACCENT: Color = Color::from_rgb(0.3, 0.5, 0.9);

/// Success color (green)
pub const SUCCESS: Color = Color::from_rgb(0.3, 0.8, 0.4);

/// Warning color (yellow)
pub const WARNING: Color = Color::from_rgb(0.9, 0.7, 0.2);

/// Error color (red)
pub const ERROR: Color = Color::from_rgb(0.9, 0.3, 0.3);

/// Background colors
pub mod background {
    use iced::Color;

    pub const PRIMARY: Color = Color::from_rgb(0.12, 0.12, 0.14);
    pub const SECONDARY: Color = Color::from_rgb(0.16, 0.16, 0.18);
    pub const TERTIARY: Color = Color::from_rgb(0.2, 0.2, 0.22);
}

/// Text colors
pub mod text {
    use iced::Color;

    pub const PRIMARY: Color = Color::from_rgb(0.9, 0.9, 0.9);
    pub const SECONDARY: Color = Color::from_rgb(0.6, 0.6, 0.6);
    pub const MUTED: Color = Color::from_rgb(0.4, 0.4, 0.4);
}
