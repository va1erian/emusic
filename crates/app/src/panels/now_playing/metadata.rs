//! Rich metadata block for the now-playing panel.

use eframe::egui;

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::NowPlayingInfo;

/// Show full metadata for a track that exists in the library.
pub fn show(
    ui: &mut egui::Ui,
    np: &NowPlayingInfo,
    track: &TrackInfo,
    library: &dyn LibraryDataSource,
) {
    ui.heading(&np.title);
    ui.label(egui::RichText::new(&np.artist).size(15.0));

    let album_line = album_line(track, library);
    ui.label(egui::RichText::new(album_line).weak());

    ui.add_space(4.0);

    let details = metadata_details(track);
    ui.label(egui::RichText::new(details).small().weak());

    ui.add_space(2.0);
    path_row(ui, &track.path);
}

/// Minimal metadata display when a track is loaded but not in the library.
pub fn show_basic(ui: &mut egui::Ui, np: &NowPlayingInfo) {
    ui.heading(&np.title);
    ui.label(egui::RichText::new(&np.artist).size(15.0));
    ui.label(egui::RichText::new(&np.album).weak());

    ui.add_space(2.0);
    path_row(ui, &np.path);
}

fn album_line(track: &TrackInfo, library: &dyn LibraryDataSource) -> String {
    let mut parts = Vec::new();
    if !track.album.is_empty() {
        let mut album = track.album.clone();
        if let Some(year) = library
            .albums()
            .iter()
            .find(|a| a.name == track.album && (a.artist == track.artist || a.artist.is_empty()))
            .and_then(|a| a.year)
        {
            album.push_str(&format!(" ({year})"));
        }
        parts.push(album);
    }
    if let Some(no) = track.track_no {
        let mut track_str = format!("Track {no}");
        if let Some(disc) = track.disc_no {
            track_str.push_str(&format!(", Disc {disc}"));
        }
        parts.push(track_str);
    }
    if !track.genre.is_empty() {
        parts.push(track.genre.clone());
    }
    parts.join("  ·  ")
}

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
