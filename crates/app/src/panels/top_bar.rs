//! Top transport bar: prev/play-pause/stop/next, repeat, shuffle, seek
//! slider with elapsed/total time, volume slider, and a search box.

use eframe::egui::{self, RichText};

use crate::player_api::{PlaybackStatus, PlayerApi, RepeatMode};
use crate::state::{AppState, Command};
use crate::theme;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    egui::Panel::top("top_bar").exact_size(56.0).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);
            transport_buttons(ui, state, player);

            ui.separator();
            toggle_button(
                ui,
                state,
                "🔁",
                "Repeat",
                repeat_label(player.repeat_mode()),
            );
            toggle_button_bool(ui, state, "🔀", "Shuffle", player.shuffle());

            ui.separator();
            seek_area(ui, state, player);

            ui.separator();
            volume_area(ui, state, player);

            ui.separator();
            search_box(ui, state);
        });
    });
}

fn transport_buttons(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    if ui.button(RichText::new("⏮").size(16.0)).clicked() {
        state.push(Command::PlayerPrevious);
    }
    let play_icon = if player.status() == PlaybackStatus::Playing {
        "⏸"
    } else {
        "▶"
    };
    if ui
        .add(egui::Button::new(RichText::new(play_icon).size(18.0)).fill(theme::current_accent()))
        .clicked()
    {
        state.push(Command::PlayerPlayPause);
    }
    if ui.button(RichText::new("⏹").size(16.0)).clicked() {
        state.push(Command::PlayerStop);
    }
    if ui.button(RichText::new("⏭").size(16.0)).clicked() {
        state.push(Command::PlayerNext);
    }
}

fn repeat_label(mode: RepeatMode) -> bool {
    mode != RepeatMode::Off
}

fn toggle_button(ui: &mut egui::Ui, state: &mut AppState, icon: &str, tooltip: &str, active: bool) {
    let mut button = egui::Button::new(icon);
    if active {
        button = button.fill(theme::current_accent());
    }
    if ui.add(button).on_hover_text(tooltip).clicked() {
        state.push(Command::PlayerToggleRepeat);
    }
}

fn toggle_button_bool(
    ui: &mut egui::Ui,
    state: &mut AppState,
    icon: &str,
    tooltip: &str,
    active: bool,
) {
    let mut button = egui::Button::new(icon);
    if active {
        button = button.fill(theme::current_accent());
    }
    if ui.add(button).on_hover_text(tooltip).clicked() {
        state.push(Command::PlayerToggleShuffle);
    }
}

fn seek_area(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    let position = player.position().as_secs_f64();
    let total = player.duration().map(|d| d.as_secs_f64()).unwrap_or(0.0);

    ui.label(format_time(position));
    let mut value = position;
    ui.spacing_mut().slider_width = 320.0;
    let slider = ui.add(egui::Slider::new(&mut value, 0.0..=total.max(0.001)).show_value(false));
    if slider.changed() {
        state.push(Command::PlayerSeek(std::time::Duration::from_secs_f64(
            value,
        )));
    }
    ui.label(format_time(total));
}

fn volume_area(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    ui.label("🔊");
    let mut volume = player.volume();
    ui.spacing_mut().slider_width = 90.0;
    if ui
        .add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false))
        .changed()
    {
        state.push(Command::PlayerSetVolume(volume));
    }
}

fn search_box(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("🔍");
    let mut query = state.search_query.clone();
    let response = ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("Search library...")
            .desired_width(200.0),
    );
    if response.changed() {
        state.push(Command::SetSearchQuery(query));
    }
}

fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
