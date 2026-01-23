//! Theme system for TUI monitor
//!
//! Loads JSON themes (dark/light) and applies styles to ratatui widgets.
//! Mirrors pi's theme handling for consistent UI.

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

// Embedded theme files
const DARK_THEME_JSON: &str = include_str!("../../../assets/themes/dark.json");
const LIGHT_THEME_JSON: &str = include_str!("../../../assets/themes/light.json");

/// Global theme instance
static THEME: OnceLock<Theme> = OnceLock::new();

/// JSON theme file structure (mirrors pi's format)
#[derive(Debug, Deserialize)]
struct ThemeJson {
    name: String,
    vars: HashMap<String, String>,
    colors: HashMap<String, String>,
}

/// Resolved theme with ratatui colors
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,

    // Background colors
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_terminal: Color,

    // Text colors
    pub text_primary: Color,
    pub text_muted: Color,
    pub text_terminal: Color,

    // Border colors
    pub border_default: Color,
    pub border_active: Color,

    // Accent and status colors
    pub accent: Color,
    pub success: Color,
    pub error: Color,

    // Status-specific colors
    pub status_starting: Color,
    pub status_running: Color,
    pub status_completed: Color,
    pub status_failed: Color,

    // Special colors for specific UI elements
    pub swarm_purple: Color,
    pub ralph_orange: Color,
    pub failed_validation_red: Color,
}

impl Theme {
    /// Parse a hex color string (#RRGGBB) to ratatui Color
    fn parse_hex(hex: &str) -> Option<Color> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(Color::Rgb(r, g, b))
    }

    /// Resolve a color value - either a variable reference or a hex color
    fn resolve_color(value: &str, vars: &HashMap<String, String>) -> Color {
        // If value starts with #, it's a hex color
        if value.starts_with('#') {
            return Self::parse_hex(value).unwrap_or(Color::Reset);
        }

        // Otherwise it's a variable reference - look it up
        if let Some(var_value) = vars.get(value) {
            Self::parse_hex(var_value).unwrap_or(Color::Reset)
        } else {
            Color::Reset
        }
    }

    /// Load theme from JSON string
    pub fn from_json(json: &str) -> Result<Self, String> {
        let theme_json: ThemeJson =
            serde_json::from_str(json).map_err(|e| format!("Failed to parse theme JSON: {}", e))?;

        let get_color = |key: &str| -> Color {
            theme_json
                .colors
                .get(key)
                .map(|v| Self::resolve_color(v, &theme_json.vars))
                .unwrap_or(Color::Reset)
        };

        Ok(Theme {
            name: theme_json.name,
            bg_primary: get_color("bgPrimary"),
            bg_secondary: get_color("bgSecondary"),
            bg_terminal: get_color("bgTerminal"),
            text_primary: get_color("textPrimary"),
            text_muted: get_color("textMuted"),
            text_terminal: get_color("textTerminal"),
            border_default: get_color("borderDefault"),
            border_active: get_color("borderActive"),
            accent: get_color("accent"),
            success: get_color("success"),
            error: get_color("error"),
            status_starting: get_color("statusStarting"),
            status_running: get_color("statusRunning"),
            status_completed: get_color("statusCompleted"),
            status_failed: get_color("statusFailed"),
            swarm_purple: get_color("swarmPurple"),
            ralph_orange: get_color("ralphOrange"),
            failed_validation_red: get_color("failedValidationRed"),
        })
    }

    /// Load the dark theme
    pub fn dark() -> Self {
        Self::from_json(DARK_THEME_JSON).expect("Embedded dark theme should be valid")
    }

    /// Load the light theme
    pub fn light() -> Self {
        Self::from_json(LIGHT_THEME_JSON).expect("Embedded light theme should be valid")
    }

    // ─────────────────────────────────────────────────────────────
    // Style helpers for common widget patterns
    // ─────────────────────────────────────────────────────────────

    /// Style for primary background areas
    pub fn bg_style(&self) -> Style {
        Style::default().bg(self.bg_primary)
    }

    /// Style for secondary/elevated background areas
    pub fn bg_secondary_style(&self) -> Style {
        Style::default().bg(self.bg_secondary)
    }

    /// Style for terminal output areas
    pub fn bg_terminal_style(&self) -> Style {
        Style::default().bg(self.bg_terminal)
    }

    /// Style for primary text
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text_primary)
    }

    /// Style for muted/secondary text
    pub fn text_muted_style(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    /// Style for terminal text
    pub fn text_terminal_style(&self) -> Style {
        Style::default().fg(self.text_terminal)
    }

    /// Style for default borders
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border_default)
    }

    /// Style for active/focused borders
    pub fn border_active_style(&self) -> Style {
        Style::default().fg(self.border_active)
    }

    /// Style for accent elements (titles, highlights)
    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Style for bold accent elements
    pub fn accent_bold_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for success indicators
    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for error indicators
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Get border style based on focus state
    pub fn border_for_focus(&self, focused: bool) -> Style {
        if focused {
            self.border_active_style()
        } else {
            self.border_style()
        }
    }

    /// Get title color based on focus state
    pub fn title_color_for_focus(&self, focused: bool) -> Color {
        if focused {
            self.accent
        } else {
            self.text_muted
        }
    }
}

/// Initialize the global theme (call once at startup)
pub fn init_theme(variant: ThemeVariant) {
    let theme = match variant {
        ThemeVariant::Dark => Theme::dark(),
        ThemeVariant::Light => Theme::light(),
    };
    let _ = THEME.set(theme);
}

/// Get the current theme (initializes to dark if not set)
pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::dark)
}

/// Available theme variants
#[derive(Debug, Clone, Copy, Default)]
pub enum ThemeVariant {
    #[default]
    Dark,
    Light,
}

// ─────────────────────────────────────────────────────────────
// Convenience re-exports for backward compatibility
// These allow existing code to continue using color constants
// ─────────────────────────────────────────────────────────────

// Background colors
pub fn bg_primary() -> Color {
    theme().bg_primary
}
pub fn bg_secondary() -> Color {
    theme().bg_secondary
}
pub fn bg_terminal() -> Color {
    theme().bg_terminal
}

// Text colors
pub fn text_primary() -> Color {
    theme().text_primary
}
pub fn text_muted() -> Color {
    theme().text_muted
}
pub fn text_terminal() -> Color {
    theme().text_terminal
}

// Border colors
pub fn border_default() -> Color {
    theme().border_default
}
pub fn border_active() -> Color {
    theme().border_active
}

// Accent and status colors
pub fn accent() -> Color {
    theme().accent
}
pub fn success() -> Color {
    theme().success
}
pub fn error() -> Color {
    theme().error
}

// Status-specific colors
pub fn status_starting() -> Color {
    theme().status_starting
}
pub fn status_running() -> Color {
    theme().status_running
}
pub fn status_completed() -> Color {
    theme().status_completed
}
pub fn status_failed() -> Color {
    theme().status_failed
}

// Special colors
pub fn swarm_purple() -> Color {
    theme().swarm_purple
}
pub fn ralph_orange() -> Color {
    theme().ralph_orange
}
pub fn failed_validation_red() -> Color {
    theme().failed_validation_red
}

// ─────────────────────────────────────────────────────────────
// Legacy constant aliases for backward compatibility
// These are the original constants that existing code uses
// ─────────────────────────────────────────────────────────────

/// Background colors
pub const BG_PRIMARY: Color = Color::Rgb(15, 23, 42);
pub const BG_SECONDARY: Color = Color::Rgb(30, 41, 59);
pub const BG_TERMINAL: Color = Color::Rgb(22, 22, 22);

/// Text colors
pub const TEXT_PRIMARY: Color = Color::Rgb(226, 232, 240);
pub const TEXT_MUTED: Color = Color::Rgb(100, 116, 139);
pub const TEXT_TERMINAL: Color = Color::Rgb(200, 200, 200);

/// Border colors
pub const BORDER_DEFAULT: Color = Color::Rgb(51, 65, 85);
pub const BORDER_ACTIVE: Color = Color::Rgb(96, 165, 250);

/// Accent and status colors
pub const ACCENT: Color = Color::Rgb(96, 165, 250);
pub const SUCCESS: Color = Color::Rgb(34, 197, 94);
pub const ERROR: Color = Color::Rgb(248, 113, 113);

/// Status-specific colors
pub const STATUS_STARTING: Color = Color::Rgb(148, 163, 184);
pub const STATUS_RUNNING: Color = Color::Rgb(34, 197, 94);
pub const STATUS_COMPLETED: Color = Color::Rgb(96, 165, 250);
pub const STATUS_FAILED: Color = Color::Rgb(248, 113, 113);

/// Special colors for specific UI elements
pub const SWARM_PURPLE: Color = Color::Rgb(168, 85, 247);
pub const RALPH_ORANGE: Color = Color::Rgb(255, 165, 0);
pub const FAILED_VALIDATION_RED: Color = Color::Rgb(239, 68, 68);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(Theme::parse_hex("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(Theme::parse_hex("#00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(Theme::parse_hex("#0000ff"), Some(Color::Rgb(0, 0, 255)));
        assert_eq!(Theme::parse_hex("ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(Theme::parse_hex("#fff"), None); // Too short
        assert_eq!(Theme::parse_hex("invalid"), None);
    }

    #[test]
    fn test_load_dark_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.name, "dark");
        assert_eq!(theme.bg_primary, Color::Rgb(15, 23, 42));
        assert_eq!(theme.accent, Color::Rgb(96, 165, 250));
    }

    #[test]
    fn test_load_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.name, "light");
        // Light theme should have different colors
        assert_ne!(theme.bg_primary, Color::Rgb(15, 23, 42));
    }

    #[test]
    fn test_variable_resolution() {
        let json = r##"{
            "name": "test",
            "vars": {
                "myColor": "#123456"
            },
            "colors": {
                "bgPrimary": "myColor",
                "bgSecondary": "#abcdef",
                "bgTerminal": "#000000",
                "textPrimary": "#ffffff",
                "textMuted": "#888888",
                "textTerminal": "#cccccc",
                "borderDefault": "#333333",
                "borderActive": "#0066ff",
                "accent": "#0066ff",
                "success": "#00ff00",
                "error": "#ff0000",
                "statusStarting": "#808080",
                "statusRunning": "#00ff00",
                "statusCompleted": "#0066ff",
                "statusFailed": "#ff0000",
                "swarmPurple": "#9900ff",
                "ralphOrange": "#ff9900",
                "failedValidationRed": "#cc0000"
            }
        }"##;

        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.name, "test");
        // bgPrimary should resolve from variable "myColor"
        assert_eq!(theme.bg_primary, Color::Rgb(0x12, 0x34, 0x56));
        // bgSecondary should be direct hex
        assert_eq!(theme.bg_secondary, Color::Rgb(0xab, 0xcd, 0xef));
    }

    #[test]
    fn test_theme_styles() {
        let theme = Theme::dark();

        // Test style helpers return expected values
        let accent_style = theme.accent_style();
        assert_eq!(accent_style.fg, Some(theme.accent));

        let border_style = theme.border_for_focus(true);
        assert_eq!(border_style.fg, Some(theme.border_active));

        let border_style = theme.border_for_focus(false);
        assert_eq!(border_style.fg, Some(theme.border_default));
    }
}
