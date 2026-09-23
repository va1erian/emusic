//! Rich metadata block for the now-playing panel.

use eframe::egui;

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::NowPlayingInfo;
use crate::state::AppState;

use super::links;

/// Show full metadata for a track that exists in the library. The artist and
/// album names are clickable (they jump to their views) and a "Properties"
/// link opens the track's full-metadata dialog.
pub fn show(
    ui: &mut egui::Ui,
    np: &NowPlayingInfo,
    track: &TrackInfo,
    library: &dyn LibraryDataSource,
    state: &mut AppState,
) {
    ui.heading(&np.title);
    links::artist(ui, state, &np.artist, 15.0);
    album_line(ui, state, track, library);

    ui.add_space(4.0);

    ui.label(egui::RichText::new(metadata_details(track)).small().weak());

    ui.add_space(2.0);
    footer(ui, state, track);
}

/// The clickable album name (with year) followed by the track/disc number
/// and genre, as one wrapped line.
fn album_line(
    ui: &mut egui::Ui,
    state: &mut AppState,
    track: &TrackInfo,
    library: &dyn LibraryDataSource,
) {
    ui.horizontal_wrapped(|ui| {
        let mut has_album = false;
        if !track.album.is_empty() {
            links::album(ui, state, &track.album, &track.artist);
            if let Some(year) = album_year(track, library) {
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

/// Minimal metadata display when a track is loaded but not in the library.
pub fn show_basic(ui: &mut egui::Ui, np: &NowPlayingInfo) {
    ui.heading(&np.title);
    ui.label(egui::RichText::new(&np.artist).size(15.0));
    ui.label(egui::RichText::new(&np.album).weak());

    ui.add_space(2.0);
    path_row(ui, &np.path);
}

/// Release year for `track`'s album, looked up from the album list so it can
/// be shown next to the album name.
fn album_year(track: &TrackInfo, library: &dyn LibraryDataSource) -> Option<u32> {
    library
        .albums()
        .iter()
        .find(|a| a.name == track.album && (a.artist == track.artist || a.artist.is_empty()))
        .and_then(|a| a.year)
}

/// Adds a weak `·` between two pieces of album-line text, but only after the
/// first one has been written.
fn separator(ui: &mut egui::Ui, has_previous: &mut bool) {
    if *has_previous {
        ui.label(egui::RichText::new("·").weak());
    }
    *has_previous = true;
}

/// Codec/format, bitrate, sample rate, bit depth, channels and duration.
fn metadata_details(track: &TrackInfo) -> String {
    let mut parts = Vec::new();
    if !track.codec.is_empty() {
        parts.push(track.codec.clone());
    } else if !track.format.is_empty() {
        parts.push(track.format.to_uppercase());
    }
    if let Some(kbps) = track.bitrate {
        parts.push(format!("{kbps} kbps"));
    }
    if let Some(rate) = track.sample_rate {
        parts.push(format!("{rate} Hz"));
    }
    if let Some(bits) = track.bit_depth {
        parts.push(format!("{bits}-bit"));
    }
    if let Some(ch) = track.channels {
        parts.push(format!("{ch}ch"));
    }
    parts.push(super::format_duration(track.duration));
    parts.join("  ·  ")
}

/// The path link plus the "Properties" link that opens the track dialog.
fn footer(ui: &mut egui::Ui, state: &mut AppState, track: &TrackInfo) {
    ui.horizontal_wrapped(|ui| {
        path_row(ui, &track.path);
        ui.label(egui::RichText::new("·").weak());
        if links::link(
            ui,
            egui::RichText::new("Properties…").small(),
            "Show all track properties",
        ) {
            state.now_playing.properties = Some(track.clone());
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

fn truncate_path(path: &str) -> String {
    const MAX: usize = 48;
    if path.len() <= MAX {
        return path.to_string();
    }
    format!("...{}", &path[path.len() - MAX + 3..])
}

#[cfg(target_os = "windows")]
fn open_containing_folder(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path))
        .spawn();
}

#[cfg(not(target_os = "windows"))]
fn open_containing_folder(path: &str) {
    let _ = path;
}
