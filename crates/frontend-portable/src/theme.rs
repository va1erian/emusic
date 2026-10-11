//! Maps the shell's colour scheme and accent (#276) onto the portable
//! [`xui_core::theme::Theme`].

use emusic_ui::state::{Accent, Theme};
use xui::xui_core::Color;
use xui::xui_core::theme::Theme as UiTheme;

/// How far the selection tint moves from the background toward the accent.
const DARK_SELECTION_MIX: f32 = 0.35;
const LIGHT_SELECTION_MIX: f32 = 0.25;

/// The portable palette for `theme`, with every accent-driven token
/// (highlights, focus border, selection, text on accent) taken from `accent`.
#[must_use]
pub fn app_theme(theme: Theme, accent: Accent) -> UiTheme {
    let base = match theme {
        Theme::Dark => UiTheme::dark(),
        Theme::Light => UiTheme::light(),
    };
    let [r, g, b] = accent.rgb().to_array();
    let accent = Color::rgb(r, g, b);
    let mix = match theme {
        Theme::Dark => DARK_SELECTION_MIX,
        Theme::Light => LIGHT_SELECTION_MIX,
    };
    UiTheme {
        accent,
        border_focused: accent,
        selection: base.background.lerp(accent, mix),
        text_on_accent: readable_on(accent),
        ..base
    }
}

/// The colours a window is painted with: emusic's own scheme and accent, or
/// the host's theme (the desktop's, on LazyOS) as it last reported it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Look {
    /// The scheme and accent chosen in Settings -> Appearance.
    Own(Theme, Accent),
    /// The host's theme, taken as is: it carries the desktop's accent.
    System(UiTheme),
}

impl Look {
    /// The portable theme to paint with.
    #[must_use]
    pub fn resolve(self) -> UiTheme {
        match self {
            Look::Own(theme, accent) => app_theme(theme, accent),
            Look::System(theme) => theme,
        }
    }
}

/// Black or white, whichever contrasts more with `background`.
fn readable_on(background: Color) -> Color {
    let white = Color::rgb(0xFF, 0xFF, 0xFF);
    let black = Color::rgb(0, 0, 0);
    if white.contrast_ratio(background) >= black.contrast_ratio(background) {
        white
    } else {
        black
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emusic_ui::state::Rgb;

    #[test]
    fn the_system_look_is_the_hosts_theme_untouched() {
        let host = UiTheme::light();
        assert_eq!(Look::System(host).resolve(), host);
        assert_eq!(
            Look::Own(Theme::Dark, Accent::default()).resolve(),
            app_theme(Theme::Dark, Accent::default())
        );
    }

    #[test]
    fn accent_replaces_the_windows_blue() {
        let theme = app_theme(Theme::Dark, Accent::Custom(Rgb::from_rgb(0xE8, 0x7A, 0x1E)));
        assert_eq!(theme.accent, Color::rgb(0xE8, 0x7A, 0x1E));
        assert_eq!(theme.border_focused, theme.accent);
        assert!(theme.is_dark);
        assert_eq!(theme.background, UiTheme::dark().background);
    }

    #[test]
    fn text_on_accent_stays_readable() {
        let yellow = app_theme(
            Theme::Light,
            Accent::Custom(Rgb::from_rgb(0xFF, 0xE0, 0x00)),
        );
        assert_eq!(yellow.text_on_accent, Color::rgb(0, 0, 0));
        let navy = app_theme(
            Theme::Light,
            Accent::Custom(Rgb::from_rgb(0x10, 0x20, 0x80)),
        );
        assert_eq!(navy.text_on_accent, Color::rgb(0xFF, 0xFF, 0xFF));
    }
}
