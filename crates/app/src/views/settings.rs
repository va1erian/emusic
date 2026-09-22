//! "Settings" view: the Appearance section (theme toggle, accent colour
//! presets and a custom picker, #40) and File associations (#11). Real
//! settings (library paths, tracker playback options, ...) are later
//! issues (#8, #19).

use eframe::egui::{self, Color32, Stroke};

use crate::state::{Accent, AppState, Command};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("Appearance");
    ui.separator();
    if ui.button("Toggle dark / light theme").clicked() {
        state.push(Command::ToggleTheme);
    }

    ui.add_space(8.0);
    ui.label("Accent colour");
    ui.horizontal_wrapped(|ui| preset_buttons(ui, state));
    custom_picker(ui, state);

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    crate::settings::associations::show(ui);
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
