//! Theme constants and reusable style functions for the GUI
//!
//! Dark theme with accent colors for Descartes.

use iced::widget::{button, container};
use iced::{Background, Border, Color, Theme};

/// Primary accent color (blue)
pub const ACCENT: Color = Color::from_rgb(0.3, 0.5, 0.9);

/// Success color (green)
pub const SUCCESS: Color = Color::from_rgb(0.3, 0.8, 0.4);

/// Warning color (yellow)
#[allow(dead_code)]
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

/// Border colors
pub mod border {
    use iced::Color;

    pub const SUBTLE: Color = Color::from_rgb(0.22, 0.22, 0.25);
    pub const DEFAULT: Color = Color::from_rgb(0.28, 0.28, 0.32);
    #[allow(dead_code)]
    pub const FOCUSED: Color = super::ACCENT;
}

/// Surface colors (layered depth)
pub mod surface {
    use iced::Color;

    pub const BASE: Color = Color::from_rgb(0.09, 0.09, 0.11);
    pub const RAISED: Color = Color::from_rgb(0.13, 0.13, 0.15);
    pub const OVERLAY: Color = Color::from_rgb(0.17, 0.17, 0.19);
    pub const HOVER: Color = Color::from_rgb(0.20, 0.20, 0.23);
}

/// Typography sizes
pub mod font_size {
    #[allow(dead_code)]
    pub const TITLE: f32 = 24.0;
    pub const HEADING: f32 = 18.0;
    pub const BODY: f32 = 14.0;
    pub const SMALL: f32 = 12.0;
    pub const CAPTION: f32 = 11.0;
}

/// Layout constants
pub const RADIUS: f32 = 8.0;
pub const RADIUS_SMALL: f32 = 4.0;
pub const SPACING_SM: f32 = 4.0;
pub const SPACING_MD: f32 = 10.0;
pub const SPACING_LG: f32 = 16.0;
#[allow(dead_code)]
pub const SPACING_XL: f32 = 24.0;

// =============================================================
// Reusable style functions
// =============================================================

/// Panel container style: raised surface with subtle border
pub fn panel_container() -> impl Fn(&Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(surface::RAISED)),
        border: Border {
            color: border::SUBTLE,
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..Default::default()
    }
}

/// Card container style: overlay surface with default border
#[allow(dead_code)]
pub fn card_container() -> impl Fn(&Theme) -> container::Style {
    |_| container::Style {
        background: Some(Background::Color(surface::OVERLAY)),
        border: Border {
            color: border::DEFAULT,
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..Default::default()
    }
}

/// Status badge: semi-transparent background with colored border
pub fn status_badge(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(Color { a: 0.15, ..color })),
        border: Border {
            color: Color { a: 0.3, ..color },
            width: 1.0,
            radius: RADIUS_SMALL.into(),
        },
        ..Default::default()
    }
}

/// Navigation button style
pub fn nav_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if active {
            ACCENT
        } else {
            match status {
                button::Status::Hovered => surface::HOVER,
                _ => Color::TRANSPARENT,
            }
        };
        let text_color = if active {
            Color::WHITE
        } else {
            text::SECONDARY
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS.into(),
            },
            ..Default::default()
        }
    }
}

/// Primary action button: accent filled
pub fn primary_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_, status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.35, 0.55, 0.95),
            button::Status::Pressed => Color::from_rgb(0.25, 0.45, 0.85),
            _ => ACCENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS.into(),
            },
            ..Default::default()
        }
    }
}

/// Danger button: red filled
pub fn danger_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_, status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.95, 0.35, 0.35),
            button::Status::Pressed => Color::from_rgb(0.85, 0.25, 0.25),
            _ => ERROR,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS.into(),
            },
            ..Default::default()
        }
    }
}

/// Ghost button: transparent with subtle hover
pub fn ghost_button() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_, status| {
        let bg = match status {
            button::Status::Hovered => surface::HOVER,
            button::Status::Pressed => surface::OVERLAY,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text::SECONDARY,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS.into(),
            },
            ..Default::default()
        }
    }
}

/// Heading text style
pub fn heading_text() -> impl Fn(&Theme) -> iced::widget::text::Style {
    |_| iced::widget::text::Style {
        color: Some(ACCENT),
    }
}

/// Muted text style
pub fn muted_text() -> impl Fn(&Theme) -> iced::widget::text::Style {
    |_| iced::widget::text::Style {
        color: Some(text::MUTED),
    }
}

/// Secondary text style
pub fn secondary_text() -> impl Fn(&Theme) -> iced::widget::text::Style {
    |_| iced::widget::text::Style {
        color: Some(text::SECONDARY),
    }
}
