//! Left navigator: library sections (Music, Albums, Artists, Genres,
//! Folders) and a second group (Starred, Most played, History, Now playing).
//! Resizable and collapsible via [`crate::state::PanelKind::Navigator`].
//!
//! Render-only (#104): the sections and the click-to-switch intent live in
//! [`emusic_ui::panels::navigator`]; this module draws them.

use eframe::egui;

use emusic_ui::panels::navigator::{self, NavigatorMsg, SECTIONS, Section};
use emusic_ui::views::Commands;

use crate::library_api::LibraryDataSource;
use crate::state::{AppState, View};

pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    state.navigator.sync(state.view);
    egui::Panel::left("navigator")
        .resizable(true)
        .default_size(170.0)
        .size_range(120.0..=320.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            for (index, section) in SECTIONS.iter().enumerate() {
                if index > 0 {
                    ui.add_space(10.0);
                }
                section_ui(ui, state, library, section);
            }
        });
}

fn section_ui(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    section: &Section,
) {
    ui.label(egui::RichText::new(section.heading).small().weak());
    for &view in section.views {
        let response = ui.selectable_label(state.view == view, view.label());
        if response.clicked() {
            navigate(state, view);
        }
        // "Shuffle all" on the collection node (#57); the other nodes are
        // covered by their own views' group/row context menus.
        if view == View::Music {
            response.context_menu(|ui| {
                if ui.button("Shuffle all").clicked() {
                    state.push(crate::shuffle::all(library));
                    ui.close();
                }
            });
        }
    }
}

/// Switches the central view through the shared navigator model.
fn navigate(state: &mut AppState, view: View) {
    let mut out = Commands::new();
    navigator::update(NavigatorMsg::Select(view), &mut out);
    state.pending.extend(out.into_vec());
}
