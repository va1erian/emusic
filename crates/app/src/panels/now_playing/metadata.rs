//! Rich metadata block for the now-playing panel.
//!
//! The display data (details line, path, links) comes from the
//! [`NowPlayingView`] model; this module only draws and records link clicks as
//! messages.

use eframe::egui;

use emusic_ui::views::now_playing::{NowPlayingMsg, NowPlayingView, truncate_path};

use crate::player_api::NowPlayingInfo;
use crate::views::track_table::columns;

use super::links;

/// Show full metadata for a track that exists in the library. The artist and
/// album names are clickable (they jump to their views) and a "Properties"
/// link opens the track's full-metadata dialog.
pub fn show(
    ui: &mut egui::Ui,
    np: &NowPlayingInfo,
    track: &crate::library_api::TrackInfo,
    view: &NowPlayingView,
    messages: &mut Vec<NowPlayingMsg>,
) {
    ui.horizontal(|ui| {
        if columns::star_cell(ui, track) {
            messages.push(NowPlayingMsg::ToggleStar(track.id));
        }
        ui.add(egui::Label::new(egui::RichText::new(&np.title).heading()).truncate());
    });
    links::artist(ui, &np.artist, 15.0, messages);
    album_line(ui, track, view, messages);

    ui.add_space(4.0);

    if let Some(details) = view.details_text() {
        ui.label(egui::RichText::new(details).small().weak());
    }

    ui.add_space(2.0);
    footer(ui, track, messages);
}

/// The clickable album name (with year) followed by the track/disc number
/// and genre, as one wrapped line.
fn album_line(
    ui: &mut egui::Ui,
    track: &crate::library_api::TrackInfo,
    view: &NowPlayingView,
    messages: &mut Vec<NowPlayingMsg>,
) {
    ui.horizontal_wrapped(|ui| {
        let mut has_album = false;
        if !track.album.is_empty() {
            links::album(ui, &track.album, &track.artist, messages);
            if let Some(year) = view.album_year() {
                ui.label(egui::RichText::new(format!("({year})")).weak());
            }
            has_album = true;
        }
        if let Some(no) = track.track_no {
            separator(ui, &mut has_album);
            let mut text = format!("Track {no}");
            if let Some(disc) = track.disc_no {
                text.push_str(&format!(", Disc {disc}"));
            }
            ui.label(egui::RichText::new(text).weak());
        }
        if !track.genre.is_empty() {
            separator(ui, &mut has_album);
            ui.label(egui::RichText::new(&track.genre).weak());
        }
    });
}

/// Adds a weak `·` between two pieces of album-line text, but only after the
/// first one has been written.
fn separator(ui: &mut egui::Ui, has_previous: &mut bool) {
    if *has_previous {
        ui.label(egui::RichText::new("·").weak());
    }
    *has_previous = true;
}

/// Minimal metadata display when a track is loaded but not in the library.
pub fn show_basic(ui: &mut egui::Ui, np: &NowPlayingInfo) {
    ui.heading(&np.title);
    ui.label(egui::RichText::new(&np.artist).size(15.0));
    ui.label(egui::RichText::new(&np.album).weak());

    ui.add_space(2.0);
    path_row(ui, &np.path);
}

/// The path link, then the "Properties" and "Edit tags" links.
fn footer(
    ui: &mut egui::Ui,
    track: &crate::library_api::TrackInfo,
    messages: &mut Vec<NowPlayingMsg>,
) {
    path_row(ui, &track.path);
    ui.horizontal(|ui| {
        if links::link(
            ui,
            egui::RichText::new("Properties…").small(),
            "Show all track properties",
        ) {
            messages.push(NowPlayingMsg::ShowProperties(Box::new(track.clone())));
        }
        ui.label(egui::RichText::new("·").weak());
        if links::link(
            ui,
            egui::RichText::new("Edit tags…").small(),
            "Edit this track's tags",
        ) {
            messages.push(NowPlayingMsg::EditTags(track.id));
        }
    });
}

/// Clickable file path that opens the containing folder in Explorer.
fn path_row(ui: &mut egui::Ui, path: &str) {
    let response = ui.add(
        egui::Label::new(
            egui::RichText::new(truncate_path(path))
                .small()
                .color(ui.visuals().hyperlink_color),
        )
        .truncate(),
    );

    if response.clicked() {
        open_containing_folder(path);
    }
    response.on_hover_text(format!("Open folder:\n{path}"));
}

#[cfg(target_os = "windows")]
fn open_containing_folder(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn open_containing_folder(path: &str) {
    let _ = path;
}
