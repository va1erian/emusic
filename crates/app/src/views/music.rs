//! "Music" view: a flat, filterable track list. A virtualized track table
//! widget (sortable columns, selection) is issue #15; this is a simple
//! placeholder that already reads real (mock) data and respects the top
//! bar's search box.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource, search_query: &str) {
    let query = search_query.to_lowercase();
    let tracks: Vec<_> = library
        .tracks()
        .iter()
        .filter(|t| {
            query.is_empty()
                || t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect();

    ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, tracks.len(), |ui, range| {
        for track in &tracks[range] {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                let title = if track.title.is_empty() {
                    "(unknown title)"
                } else {
                    track.title.as_str()
                };
                ui.add_sized([260.0, 16.0], egui::Label::new(title).truncate());
                let artist = if track.artist.is_empty() {
                    "(unknown artist)"
                } else {
                    track.artist.as_str()
                };
                ui.add_sized([180.0, 16.0], egui::Label::new(artist).truncate());
                ui.add_sized([180.0, 16.0], egui::Label::new(&track.album).truncate());
                ui.add_sized(
                    [50.0, 16.0],
                    egui::Label::new(egui::RichText::new(&track.format).weak()),
                );
                ui.label(format_duration(track.duration));
            });
        }
    });
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
