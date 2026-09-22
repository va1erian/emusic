//! Dark/light visual styling, MusicBee-inspired: dense spacing, a strong
//! accent colour, subtle panel separation. The accent is user-selectable
//! (Settings → Appearance, #40); hover/selected/dim variants are derived
//! from it so contrast stays readable in both themes.

use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui::{self, Color32, CornerRadius, Stroke, Style, Visuals};

use crate::state::Theme;

/// MusicBee-ish orange, the default accent ([`crate::state::Accent::Orange`]).
pub const DEFAULT_ACCENT: Color32 = Color32::from_rgb(0xE8, 0x7A, 0x1E);

/// The accent colour currently applied, packed as `0xRRGGBB`. The UI is
/// single-threaded, so a relaxed atomic is all the syncing this needs.
static CURRENT_ACCENT: AtomicU32 = AtomicU32::new(pack(DEFAULT_ACCENT));

/// The accent colour currently in effect (updated by [`apply`]). Panels
/// and views read this instead of a compile-time constant so they follow
/// the Settings choice live.
pub fn current_accent() -> Color32 {
    let packed = CURRENT_ACCENT.load(Ordering::Relaxed);
    Color32::from_rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

/// Applies the theme's visuals and spacing to `ctx` with the given accent,
/// makes it the active theme, and records the accent for
/// [`current_accent`].
pub fn apply(ctx: &egui::Context, theme: Theme, accent: Color32) {
    CURRENT_ACCENT.store(pack(accent), Ordering::Relaxed);
    let egui_theme = to_egui_theme(theme);
    ctx.set_theme(egui_theme);
    ctx.style_mut_of(egui_theme, |style| customize(style, theme, accent));
}

const fn pack(color: Color32) -> u32 {
    let [r, g, b, _] = color.to_array();
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn to_egui_theme(theme: Theme) -> egui::Theme {
    match theme {
        Theme::Dark => egui::Theme::Dark,
        Theme::Light => egui::Theme::Light,
    }
}

/// Accent shades derived from the user's choice, themed so contrast stays
/// readable in both dark and light visuals.
struct AccentShades {
    /// Fills for active/accent widgets, strokes and dark-theme links.
    base: Color32,
    /// Border of hovered widgets.
    hover: Color32,
    /// Selection background.
    selected: Color32,
    /// De-emphasised accent, e.g. links and selected text on light theme.
    dim: Color32,
}

impl AccentShades {
    fn derive(accent: Color32, theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                base: accent,
                hover: toward_white(accent, 0.18),
                selected: accent.linear_multiply(0.55),
                dim: accent.linear_multiply(0.75),
            },
            Theme::Light => Self {
                base: accent,
                hover: accent.linear_multiply(0.85),
                selected: toward_white(accent, 0.75),
                dim: accent.linear_multiply(0.72),
            },
        }
    }
}

/// Blends `color` toward white by `t` (0..1) in sRGB space.
fn toward_white(color: Color32, t: f32) -> Color32 {
    let mix = |c: u8| (c as f32 + (255.0 - c as f32) * t).round() as u8;
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

fn customize(style: &mut Style, theme: Theme, accent: Color32) {
    let s = AccentShades::derive(accent, theme);
    style.visuals = match theme {
        Theme::Dark => dark_visuals(&s),
        Theme::Light => light_visuals(&s),
    };

    // Dense, table-friendly spacing (MusicBee packs a lot into the track
    // list without feeling cramped).
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    style.spacing.interact_size.y = 20.0;
}

fn dark_visuals(s: &AccentShades) -> Visuals {
    let mut v = Visuals::dark();
    v.override_text_color = None;
    v.panel_fill = Color32::from_rgb(0x1E, 0x1F, 0x22);
    v.window_fill = Color32::from_rgb(0x24, 0x25, 0x29);
    v.extreme_bg_color = Color32::from_rgb(0x17, 0x18, 0x1A);
    v.faint_bg_color = Color32::from_rgb(0x28, 0x29, 0x2D);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x24, 0x25, 0x29);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x2C, 0x2D, 0x32);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x3A, 0x3B, 0x41);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, s.hover);
    v.widgets.active.bg_fill = s.base;
    // `selection.stroke.color` doubles as the selected-text colour in egui.
    v.selection.bg_fill = s.selected;
    v.selection.stroke = Stroke::new(1.0, s.base);
    v.hyperlink_color = s.base;
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v
}

fn light_visuals(s: &AccentShades) -> Visuals {
    let mut v = Visuals::light();
    v.panel_fill = Color32::from_rgb(0xF3, 0xF3, 0xF4);
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    v.faint_bg_color = Color32::from_rgb(0xEC, 0xEC, 0xEE);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0xE6, 0xE6, 0xE9);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0xDD, 0xDD, 0xE2);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, s.hover);
    v.widgets.active.bg_fill = s.base;
    // A pale accent tint with dark accent text, so selected text stays
    // readable (egui paints selection text in `selection.stroke.color`).
    v.selection.bg_fill = s.selected;
    v.selection.stroke = Stroke::new(1.0, s.dim);
    v.hyperlink_color = s.dim;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: Color32 = Color32::from_rgb(0xE8, 0x7A, 0x1E);

    #[test]
    fn apply_records_current_accent() {
        let ctx = egui::Context::default();
        apply(&ctx, Theme::Dark, ACCENT);
        assert_eq!(current_accent(), ACCENT);
        apply(&ctx, Theme::Light, Color32::from_rgb(1, 2, 3));
        assert_eq!(current_accent(), Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn derived_shades_stay_readable() {
        let dark = AccentShades::derive(ACCENT, Theme::Dark);
        assert_eq!(dark.base, ACCENT);
        // Dark theme: selection is a dimmed fill with accent text on top.
        assert_ne!(dark.selected, ACCENT);

        let light = AccentShades::derive(ACCENT, Theme::Light);
        // Light theme: selection is a pale tint with dark accent text.
        let luminance =
            |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
        assert!(luminance(light.selected) > luminance(ACCENT));
        assert!(luminance(light.dim) < luminance(ACCENT));
    }

    #[test]
    fn toward_white_endpoints() {
        assert_eq!(toward_white(ACCENT, 0.0), ACCENT);
        assert_eq!(toward_white(ACCENT, 1.0), Color32::WHITE);
    }
}
