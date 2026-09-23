//! "Settings" view: a tab strip over three independent sub-pages — Library
//! folders (#19), Appearance (theme toggle, accent colour presets and a
//! custom picker, #40) and File associations (#11).
//!
//! Tabs (#137) keep each concern from pushing the others off-screen: the
//! Library tab's folder list scrolls within its own bounded area, so a
//! library with many roots can never hide the Appearance or File association
//! controls.

use eframe::egui::{self, Color32, Stroke};

use crate::state::{Accent, AppState, Command, SettingsTab, VisualizerMode};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    tab_strip(ui, state);
    ui.separator();
    ui.add_space(8.0);

    // The tab body fills the page's remaining height, so the Library tab's
    // scroll area can bound itself to it (#137).
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap()),
        |ui| match state.settings_tab {
            SettingsTab::Library => crate::settings::library::show(ui, state),
            SettingsTab::Appearance => appearance(ui, state),
            SettingsTab::Associations => crate::settings::associations::show(ui),
            SettingsTab::Playback => crate::settings::tracker::show(ui, state),
            SettingsTab::About => crate::settings::about::show(ui),
        },
    );
}

/// One selectable label per sub-page; the active one is highlighted by
/// `selectable_value` itself.
fn tab_strip(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal_wrapped(|ui| {
        for tab in SettingsTab::ALL {
            ui.selectable_value(&mut state.settings_tab, tab, tab.label());
        }
    });
}

/// Theme toggle plus accent presets and a free-form picker (#40).
fn appearance(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("Toggle dark / light theme").clicked() {
        state.push(Command::ToggleTheme);
    }

    ui.add_space(8.0);
    ui.label("Accent colour");
    ui.horizontal_wrapped(|ui| preset_buttons(ui, state));
    custom_picker(ui, state);

    ui.add_space(12.0);
    visualizer(ui, state);
}

/// The optional status-bar visualizer strip (#25). Disabled by default: an
/// animated strip forces a continuous repaint while playing, so it stays off
/// unless the user explicitly turns it on.
fn visualizer(ui: &mut egui::Ui, state: &mut AppState) {
    ui.checkbox(&mut state.visualizer_enabled, "Visualizer")
        .on_hover_text(
            "Animated spectrum/oscilloscope strip in the status bar. \
             Kept off by default because it repaints continuously while playing.",
        );
    if state.visualizer_enabled {
        ui.horizontal_wrapped(|ui| {
            for mode in VisualizerMode::ALL {
                ui.selectable_value(&mut state.visualizer, mode, mode.label());
            }
        });
    }
}

/// One filled button per preset; the active preset gets a strong border.
fn preset_buttons(ui: &mut egui::Ui, state: &mut AppState) {
    for preset in Accent::PRESETS {
        let color = preset.color();
        let text = egui::RichText::new(preset.label()).color(text_on(color));
        let mut button = egui::Button::new(text).fill(color);
        if state.accent == preset {
            button = button.stroke(Stroke::new(2.0, ui.visuals().strong_text_color()));
        }
        if ui
            .add(button)
            .on_hover_text(if preset == Accent::default() {
                "Default"
            } else {
                preset.label()
            })
            .clicked()
        {
            state.push(Command::SetAccent(preset));
        }
    }
}

/// Free-form colour choice via egui's built-in picker; applied live while
/// the picker is open.
fn custom_picker(ui: &mut egui::Ui, state: &mut AppState) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Custom:");
        let [r, g, b, _] = state.accent.color().to_array();
        let mut rgb = [r, g, b];
        if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
            state.push(Command::SetAccent(Accent::Custom(Color32::from_rgb(
                rgb[0], rgb[1], rgb[2],
            ))));
        }
    });
}

/// White or black, whichever contrasts better with `bg`, so labels stay
/// readable on accent-filled buttons across presets.
fn text_on(bg: Color32) -> Color32 {
    let [r, g, b, _] = bg.to_array();
    let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    if luma > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
