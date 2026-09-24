//! egui renderer for the column browser (#16, #99).
//!
//! All state and logic live in [`ColumnBrowser`] (`emusic-ui`); this module
//! only draws the three cascading panes and maps clicks to
//! [`ColumnBrowserMsg`]s.

mod pane;
#[cfg(test)]
mod tests;

use eframe::egui;

use emusic_ui::views::column_browser::{
    ColumnBrowser, ColumnBrowserMsg, MAX_HEIGHT, MIN_HEIGHT, Pane,
};
use emusic_ui::views::{Commands, Ctx};

use super::EguiView;

/// Renders the three panes in a resizable top panel, applying any clicks to
/// the model. Must run before filtering the track table, so the table sees
/// the same selections this frame.
impl EguiView for ColumnBrowser {
    fn show(&mut self, ui: &mut egui::Ui, _id_salt: &str, cx: &Ctx, out: &mut Commands) {
        self.refresh(cx);
        let mut messages: Vec<ColumnBrowserMsg> = Vec::new();

        let response = egui::Panel::top("column_browser")
            .resizable(true)
            .default_size(self.height)
            .min_size(MIN_HEIGHT)
            .max_size(MAX_HEIGHT)
            .show(ui, |ui| {
                ui.columns(3, |columns| {
                    pane::show(
                        &mut columns[0],
                        "column_browser_genre",
                        "Genre",
                        Pane::Genre,
                        self,
                        &mut messages,
                    );
                    pane::show(
                        &mut columns[1],
                        "column_browser_artist",
                        "Artist",
                        Pane::Artist,
                        self,
                        &mut messages,
                    );
                    pane::show(
                        &mut columns[2],
                        "column_browser_album",
                        "Album",
                        Pane::Album,
                        self,
                        &mut messages,
                    );
                });
            });

        for msg in messages {
            self.update(msg);
        }

        // Mirror the panel's live size (which follows a user drag) back into
        // the state so the config can persist it.
        self.height = response.response.rect.height();
        let _ = out;
    }
}
