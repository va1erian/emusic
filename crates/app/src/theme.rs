//! Dark/light visual styling, MusicBee-inspired: dense spacing, a strong
//! accent colour, subtle panel separation. The accent is user-selectable
//! (Settings → Appearance, #40); the semantic [`Palette`] (in `emusic-ui`)
//! derives every colour from the theme + accent, and this module maps it to
//! `egui::Visuals`.

use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, Style, TextStyle, Visuals};

use crate::state::{Accent, Appearance, DEFAULT_ACCENT, Metrics, Palette, Rgb, Rgba, Theme};

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

/// Maps a toolkit-agnostic colour to egui, for painters that need a
/// [`Color32`] (swatches, icons, markers).
pub fn to_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Applies the theme's visuals and spacing to `ctx` for the given theme,
/// accent and appearance settings, makes it the active theme, and records the
/// accent and metrics for [`current_accent`] and [`crate::appearance`].
pub fn apply(ctx: &egui::Context, theme: Theme, accent: Accent, appearance: Appearance) {
    let palette = Palette::of(theme, accent);
    CURRENT_ACCENT.store(pack(palette.accent), Ordering::Relaxed);
    let metrics = crate::appearance::install(appearance, ctx.pixels_per_point());
    let egui_theme = to_egui_theme(theme);
    ctx.set_theme(egui_theme);
    ctx.style_mut_of(egui_theme, |style| {
        customize(style, theme, &palette, &metrics)
    });
}

const fn pack(color: Rgb) -> u32 {
    ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32
}

fn to_rgba(rgba: Rgba) -> Color32 {
    // Packs the derived bytes verbatim: `Palette` already ran the exact
    // gamma/alpha math, and `ecolor` stores these shades premultiplied, so
    // this must not convert again (`from_rgba_unmultiplied` would).
    Color32::from_rgba_premultiplied(rgba.r, rgba.g, rgba.b, rgba.a)
}

fn to_egui_theme(theme: Theme) -> egui::Theme {
    match theme {
        Theme::Dark => egui::Theme::Dark,
        Theme::Light => egui::Theme::Light,
    }
}

fn customize(style: &mut Style, theme: Theme, palette: &Palette, metrics: &Metrics) {
    style.visuals = match theme {
        Theme::Dark => dark_visuals(palette),
        Theme::Light => light_visuals(palette),
    };

    // Text sizes come from the shared metrics (#309), so the font-size
    // setting scales every style. The monospace family keeps the body size
    // (matching egui's defaults) while the proportional family carries it.
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(metrics.small, egui::FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(metrics.body, egui::FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(metrics.body, egui::FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(metrics.title, egui::FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(metrics.body, egui::FontFamily::Monospace),
        ),
    ]
    .into();

    // Dense, table-friendly spacing (MusicBee packs a lot into the track
    // list without feeling cramped).
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    style.spacing.interact_size.y = metrics.row_height;
}

fn dark_visuals(p: &Palette) -> Visuals {
    let mut v = Visuals::dark();
    // Same value the default derives (`noninteractive.fg_stroke`); set
    // explicitly so the palette is the single source of truth. Weak text
    // keeps egui's default derivation from it, exactly as before.
    v.override_text_color = Some(to_color32(p.fg));
    v.panel_fill = to_color32(p.panel_bg);
    v.window_fill = to_color32(p.window_bg);
    v.extreme_bg_color = to_color32(p.extreme_bg);
    v.faint_bg_color = to_color32(p.faint_bg);
    v.widgets.noninteractive.bg_fill = to_color32(p.view_bg);
    v.widgets.inactive.bg_fill = to_color32(p.control_bg);
    v.widgets.hovered.bg_fill = to_color32(p.control_hover_bg);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, to_rgba(p.accent_hover));
    v.widgets.active.bg_fill = to_color32(p.accent);
    // `selection.stroke.color` doubles as the selected-text colour in egui.
    v.selection.bg_fill = to_rgba(p.selection_bg);
    v.selection.stroke = Stroke::new(1.0, to_rgba(p.accent_text));
    v.hyperlink_color = to_rgba(p.accent_text);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v
}

fn light_visuals(p: &Palette) -> Visuals {
    let mut v = Visuals::light();
    v.override_text_color = Some(to_color32(p.fg));
    v.panel_fill = to_color32(p.panel_bg);
    v.window_fill = to_color32(p.window_bg);
    v.extreme_bg_color = to_color32(p.extreme_bg);
    v.faint_bg_color = to_color32(p.faint_bg);
    v.widgets.noninteractive.bg_fill = to_color32(p.view_bg);
    v.widgets.inactive.bg_fill = to_color32(p.control_bg);
    v.widgets.hovered.bg_fill = to_color32(p.control_hover_bg);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, to_rgba(p.accent_hover));
    v.widgets.active.bg_fill = to_color32(p.accent);
    // A pale accent tint with dark accent text, so selected text stays
    // readable (egui paints selection text in `selection.stroke.color`).
    v.selection.bg_fill = to_rgba(p.selection_bg);
    v.selection.stroke = Stroke::new(1.0, to_rgba(p.accent_text));
    v.hyperlink_color = to_rgba(p.accent_text);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_records_current_accent() {
        let ctx = egui::Context::default();
        apply(&ctx, Theme::Dark, Accent::Orange, Appearance::default());
        assert_eq!(current_accent(), to_color32(DEFAULT_ACCENT));
        apply(
            &ctx,
            Theme::Light,
            Accent::Custom(Rgb::from_rgb(1, 2, 3)),
            Appearance::default(),
        );
        assert_eq!(current_accent(), Color32::from_rgb(1, 2, 3));
    }

    /// The `Palette` → `Visuals` mapping reproduces the pre-#94 colours
    /// bit-for-bit (including blend alphas). Values recorded from the old
    /// derivation; if the gamma math in `emusic-ui` drifts by even one ulp,
    /// this fails.
    #[test]
    fn palette_mapping_matches_recorded_colours() {
        let cases = [
            (
                Theme::Dark,
                Accent::Blue,
                "3584E4",
                [
                    ("active", (0x35, 0x84, 0xE4, 0xFF)),
                    ("hover", (0x59, 0x9A, 0xE9, 0xFF)),
                    ("sel_bg", (0x1D, 0x49, 0x7D, 0x8C)),
                    ("sel_text", (0x35, 0x84, 0xE4, 0xFF)),
                    ("link", (0x35, 0x84, 0xE4, 0xFF)),
                ],
            ),
            (
                Theme::Dark,
                Accent::Custom(Rgb::from_rgb(0x12, 0xAB, 0xCF)),
                "12ABCF",
                [
                    ("active", (0x12, 0xAB, 0xCF, 0xFF)),
                    ("hover", (0x3D, 0xBA, 0xD8, 0xFF)),
                    ("sel_bg", (0x0A, 0x5E, 0x72, 0x8C)),
                    ("sel_text", (0x12, 0xAB, 0xCF, 0xFF)),
                    ("link", (0x12, 0xAB, 0xCF, 0xFF)),
                ],
            ),
            (
                Theme::Dark,
                Accent::Orange,
                "E87A1E",
                [
                    ("active", (0xE8, 0x7A, 0x1E, 0xFF)),
                    ("hover", (0xEC, 0x92, 0x47, 0xFF)),
                    ("sel_bg", (0x80, 0x43, 0x11, 0x8C)),
                    ("sel_text", (0xE8, 0x7A, 0x1E, 0xFF)),
                    ("link", (0xE8, 0x7A, 0x1E, 0xFF)),
                ],
            ),
            (
                Theme::Light,
                Accent::Blue,
                "3584E4",
                [
                    ("active", (0x35, 0x84, 0xE4, 0xFF)),
                    ("hover", (0x2D, 0x70, 0xC2, 0xD9)),
                    ("sel_bg", (0xCD, 0xE0, 0xF8, 0xFF)),
                    ("sel_text", (0x26, 0x5F, 0xA4, 0xB8)),
                    ("link", (0x26, 0x5F, 0xA4, 0xB8)),
                ],
            ),
            (
                Theme::Light,
                Accent::Custom(Rgb::from_rgb(0x12, 0xAB, 0xCF)),
                "12ABCF",
                [
                    ("active", (0x12, 0xAB, 0xCF, 0xFF)),
                    ("hover", (0x0F, 0x91, 0xB0, 0xD9)),
                    ("sel_bg", (0xC4, 0xEA, 0xF3, 0xFF)),
                    ("sel_text", (0x0D, 0x7B, 0x95, 0xB8)),
                    ("link", (0x0D, 0x7B, 0x95, 0xB8)),
                ],
            ),
            (
                Theme::Light,
                Accent::Orange,
                "E87A1E",
                [
                    ("active", (0xE8, 0x7A, 0x1E, 0xFF)),
                    ("hover", (0xC5, 0x68, 0x1A, 0xD9)),
                    ("sel_bg", (0xF9, 0xDE, 0xC7, 0xFF)),
                    ("sel_text", (0xA7, 0x58, 0x16, 0xB8)),
                    ("link", (0xA7, 0x58, 0x16, 0xB8)),
                ],
            ),
        ];
        for (theme, accent, name, expected) in cases {
            let mut style = egui::Style::default();
            customize(
                &mut style,
                theme,
                &Palette::of(theme, accent),
                &Metrics::DEFAULT,
            );
            let v = style.visuals;
            let actual = [
                ("active", v.widgets.active.bg_fill),
                ("hover", v.widgets.hovered.bg_stroke.color),
                ("sel_bg", v.selection.bg_fill),
                ("sel_text", v.selection.stroke.color),
                ("link", v.hyperlink_color),
            ];
            for ((key, color), (_, rgba)) in actual.iter().zip(expected.iter()) {
                let (r, g, b, a) = (color.r(), color.g(), color.b(), color.a());
                assert_eq!(
                    (r, g, b, a),
                    *rgba,
                    "{theme:?} {name} {key}: got ({r:02X},{g:02X},{b:02X},{a:02X})"
                );
            }
            // Fills never depended on the accent; one case pins them.
            if name == "E87A1E" {
                let (panel, window) = (v.panel_fill, v.window_fill);
                let (er, eg, eb) = (panel.r(), panel.g(), panel.b());
                let (wr, wg, wb) = (window.r(), window.g(), window.b());
                match theme {
                    Theme::Dark => {
                        assert_eq!((er, eg, eb), (0x1E, 0x1F, 0x22));
                        assert_eq!((wr, wg, wb), (0x24, 0x25, 0x29));
                    }
                    Theme::Light => {
                        assert_eq!((er, eg, eb), (0xF3, 0xF3, 0xF4));
                        assert_eq!((wr, wg, wb), (0xFF, 0xFF, 0xFF));
                    }
                }
            }
        }
    }
}
