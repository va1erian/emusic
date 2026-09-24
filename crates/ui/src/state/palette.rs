//! Semantic colour [`Palette`] derived from [`Theme`] + [`Accent`] (#94).
//!
//! Plain RGB(A) data, no toolkit types: every frontend maps it to its own
//! colours (the egui frontend does so in its `theme.rs`). The derivations
//! mirror `ecolor` 0.36's gamma math exactly, so the egui mapping reproduces
//! today's pixels bit-for-bit (guarded by the snapshot tests plus the
//! golden mapping test in the frontend).

use super::appearance::{Accent, Rgb, Theme};

/// sRGB colour with alpha, for derived shades that blend (selection
/// backgrounds, hover strokes, dimmed accent text). Frontends that cannot
/// blend per-pixel use the RGB channels as the solid colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
    /// Alpha channel, 0–255.
    pub a: u8,
}

impl Rgba {
    /// Opaque colour.
    pub const fn opaque(rgb: Rgb) -> Self {
        Self {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
            a: 255,
        }
    }
}

/// Every colour a frontend needs to draw the shell, derived from the theme
/// and accent. Neutral fills are fixed per theme; accent shades are derived
/// from the user's accent so contrast stays readable in both themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Window backdrop.
    pub window_bg: Rgb,
    /// Backdrop behind labels and other non-interactive widgets.
    pub view_bg: Rgb,
    /// Panel fill (sidebars, top/bottom bars).
    pub panel_bg: Rgb,
    /// Recessed areas (e.g. text-edit backgrounds).
    pub extreme_bg: Rgb,
    /// Subtle raised areas.
    pub faint_bg: Rgb,
    /// Resting control fill (buttons, checkboxes).
    pub control_bg: Rgb,
    /// Hovered control fill.
    pub control_hover_bg: Rgb,
    /// Primary text.
    pub fg: Rgb,
    /// Secondary text.
    pub weak: Rgb,
    /// The accent itself: active fills, dark-theme links.
    pub accent: Rgb,
    /// Hovered-widget border.
    pub accent_hover: Rgba,
    /// Selection background.
    pub selection_bg: Rgba,
    /// Selected text and light-theme links.
    pub accent_text: Rgba,
}

impl Palette {
    /// Derives the full palette from a theme and accent.
    pub fn of(theme: Theme, accent: Accent) -> Self {
        let base = accent.rgb();
        match theme {
            Theme::Dark => Self {
                window_bg: Rgb::from_rgb(0x24, 0x25, 0x29),
                view_bg: Rgb::from_rgb(0x1B, 0x1B, 0x1B),
                panel_bg: Rgb::from_rgb(0x1E, 0x1F, 0x22),
                extreme_bg: Rgb::from_rgb(0x17, 0x18, 0x1A),
                faint_bg: Rgb::from_rgb(0x28, 0x29, 0x2D),
                control_bg: Rgb::from_rgb(0x2C, 0x2D, 0x32),
                control_hover_bg: Rgb::from_rgb(0x3A, 0x3B, 0x41),
                fg: Rgb::from_rgb(0x8C, 0x8C, 0x8C),
                weak: Rgb::from_rgb(0x54, 0x54, 0x54),
                accent: base,
                accent_hover: Rgba::opaque(toward_white(base, 0.18)),
                selection_bg: linear_multiply(base, 0.55),
                accent_text: Rgba::opaque(base),
            },
            Theme::Light => Self {
                window_bg: Rgb::from_rgb(0xFF, 0xFF, 0xFF),
                view_bg: Rgb::from_rgb(0xF8, 0xF8, 0xF8),
                panel_bg: Rgb::from_rgb(0xF3, 0xF3, 0xF4),
                extreme_bg: Rgb::from_rgb(0xFA, 0xFA, 0xFA),
                faint_bg: Rgb::from_rgb(0xEC, 0xEC, 0xEE),
                control_bg: Rgb::from_rgb(0xE6, 0xE6, 0xE9),
                control_hover_bg: Rgb::from_rgb(0xDD, 0xDD, 0xE2),
                fg: Rgb::from_rgb(0x50, 0x50, 0x50),
                weak: Rgb::from_rgb(0x30, 0x30, 0x30),
                accent: base,
                accent_hover: linear_multiply(base, 0.85),
                selection_bg: Rgba::opaque(toward_white(base, 0.75)),
                accent_text: linear_multiply(base, 0.72),
            },
        }
    }
}

/// Blends `color` toward white by `t` (0..1) in sRGB space.
fn toward_white(color: Rgb, t: f32) -> Rgb {
    let mix = |c: u8| (c as f32 + (255.0 - c as f32) * t).round() as u8;
    Rgb::from_rgb(mix(color.r), mix(color.g), mix(color.b))
}

/// Multiplies an opaque colour by `factor` in linear space, mirroring
/// `ecolor` 0.36's `Color32::linear_multiply` for opaque inputs exactly
/// (including the resulting alpha, which frontends blend).
fn linear_multiply(color: Rgb, factor: f32) -> Rgba {
    debug_assert!(0.0 <= factor && factor.is_finite());
    // Opaque input: `Rgba::from` gives alpha 1.0, `multiply` scales every
    // channel, and the way back packs `fast_round(gamma(r/a)) * a` per
    // channel with `fast_round(a * 255)` alpha — reproduced here directly,
    // including the `(lin * f) / f` round-trip, which is not a no-op in f32.
    let channel = |c: u8| {
        let divided = linear_from_gamma(c) * factor / factor;
        fast_round(gamma_u8_from_linear(divided) as f32 * factor)
    };
    Rgba {
        r: channel(color.r),
        g: channel(color.g),
        b: channel(color.b),
        a: fast_round(factor * 255.0),
    }
}

/// sRGB [0, 255] byte to linear [0, 1], mirroring `ecolor`'s float-threshold
/// variant (used on the opaque path, not the `u8`-lookup one).
fn linear_from_gamma(byte: u8) -> f32 {
    let gamma = byte as f32 / 255.0;
    if gamma <= 0.04045 {
        gamma / 12.92
    } else {
        ((gamma + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear [0, 1] to sRGB [0, 255] byte (clamped), mirroring `ecolor`.
fn gamma_u8_from_linear(linear: f32) -> u8 {
    if linear <= 0.0 {
        0
    } else if linear <= 0.0031308 {
        fast_round(3294.6 * linear)
    } else if linear <= 1.0 {
        fast_round(269.025 * linear.powf(1.0 / 2.4) - 14.025)
    } else {
        255
    }
}

fn fast_round(value: f32) -> u8 {
    (value + 0.5) as u8 // `as` saturates, matching `ecolor`
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toward_white_endpoints() {
        let accent = Rgb::from_rgb(0xE8, 0x7A, 0x1E);
        assert_eq!(toward_white(accent, 0.0), accent);
        assert_eq!(toward_white(accent, 1.0), Rgb::from_rgb(0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn dark_selection_is_dimmed_with_accent_text() {
        let palette = Palette::of(Theme::Dark, Accent::Blue);
        assert_eq!(palette.accent, Rgb::from_rgb(0x35, 0x84, 0xE4));
        assert_eq!(palette.accent_text, Rgba::opaque(palette.accent));
        assert_ne!(
            (
                palette.selection_bg.r,
                palette.selection_bg.g,
                palette.selection_bg.b
            ),
            (palette.accent.r, palette.accent.g, palette.accent.b)
        );
    }

    #[test]
    fn light_selection_is_a_pale_tint_with_dark_text() {
        let palette = Palette::of(Theme::Light, Accent::Blue);
        let luminance =
            |c: (u8, u8, u8)| 0.299 * c.0 as f32 + 0.587 * c.1 as f32 + 0.114 * c.2 as f32;
        let accent_luminance = luminance((0x35, 0x84, 0xE4));
        let selected = (
            palette.selection_bg.r,
            palette.selection_bg.g,
            palette.selection_bg.b,
        );
        let text = (
            palette.accent_text.r,
            palette.accent_text.g,
            palette.accent_text.b,
        );
        assert!(luminance(selected) > accent_luminance);
        assert!(luminance(text) < accent_luminance);
    }

    #[test]
    fn text_colours_match_todays_egui_defaults() {
        // `egui::Visuals` default text: dark `gray(140)` weakened ×0.6,
        // light `gray(80)` weakened ×0.6 (`(c*f+0.5) as u8`).
        let dark = Palette::of(Theme::Dark, Accent::Orange);
        assert_eq!(dark.fg, Rgb::from_rgb(140, 140, 140));
        assert_eq!(dark.weak, Rgb::from_rgb(84, 84, 84));
        let light = Palette::of(Theme::Light, Accent::Orange);
        assert_eq!(light.fg, Rgb::from_rgb(80, 80, 80));
        assert_eq!(light.weak, Rgb::from_rgb(48, 48, 48));
    }
}
