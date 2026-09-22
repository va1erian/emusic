//! Dark/light visual styling, MusicBee-inspired: dense spacing, a strong
//! accent colour, subtle panel separation.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Style, Visuals};

use crate::state::Theme;

/// MusicBee-ish orange accent.
pub const ACCENT: Color32 = Color32::from_rgb(0xE8, 0x7A, 0x1E);

/// Applies the given theme's visuals and spacing to `ctx`, and makes it the
/// active theme.
pub fn apply(ctx: &egui::Context, theme: Theme) {
    let egui_theme = to_egui_theme(theme);
    ctx.set_theme(egui_theme);
    ctx.style_mut_of(egui_theme, |style| customize(style, theme));
}

fn to_egui_theme(theme: Theme) -> egui::Theme {
    match theme {
        Theme::Dark => egui::Theme::Dark,
        Theme::Light => egui::Theme::Light,
    }
}

fn customize(style: &mut Style, theme: Theme) {
    style.visuals = match theme {
        Theme::Dark => dark_visuals(),
        Theme::Light => light_visuals(),
    };

    // Dense, table-friendly spacing (MusicBee packs a lot into the track
    // list without feeling cramped).
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(6.0, 3.0);
    style.spacing.interact_size.y = 20.0;
}

fn dark_visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.override_text_color = None;
    v.panel_fill = Color32::from_rgb(0x1E, 0x1F, 0x22);
    v.window_fill = Color32::from_rgb(0x24, 0x25, 0x29);
    v.extreme_bg_color = Color32::from_rgb(0x17, 0x18, 0x1A);
    v.faint_bg_color = Color32::from_rgb(0x28, 0x29, 0x2D);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x24, 0x25, 0x29);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x2C, 0x2D, 0x32);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x3A, 0x3B, 0x41);
    v.widgets.active.bg_fill = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v
}

fn light_visuals() -> Visuals {
    let mut v = Visuals::light();
    v.panel_fill = Color32::from_rgb(0xF3, 0xF3, 0xF4);
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    v.faint_bg_color = Color32::from_rgb(0xEC, 0xEC, 0xEE);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0xE6, 0xE6, 0xE9);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0xDD, 0xDD, 0xE2);
    v.widgets.active.bg_fill = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v
}
