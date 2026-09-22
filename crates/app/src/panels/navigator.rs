//! Left navigator: library sections (Music, Albums, Artists, Genres,
//! Folders) and a second group (Most played, History, Now playing).
//! Resizable and collapsible via [`crate::state::PanelKind::Navigator`].

use eframe::egui;

use crate::state::{AppState, Command, View};

const LIBRARY_VIEWS: &[View] = &[
    View::Music,
    View::Albums,
    View::Artists,
    View::Genres,
    View::Folders,
];
const ACTIVITY_VIEWS: &[View] = &[View::MostPlayed, View::History, View::NowPlaying];

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    egui::Panel::left("navigator")
        .resizable(true)
        .default_size(170.0)
        .size_range(120.0..=320.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            section(ui, state, "LIBRARY", LIBRARY_VIEWS);
            ui.add_space(10.0);
            section(ui, state, "ACTIVITY", ACTIVITY_VIEWS);
        });
}

fn section(ui: &mut egui::Ui, state: &mut AppState, heading: &str, views: &[View]) {
    ui.label(egui::RichText::new(heading).small().weak());
    for &view in views {
        let selected = state.view == view;
        if ui.selectable_label(selected, view.label()).clicked() {
            state.push(Command::SetView(view));
        }
    }
}
