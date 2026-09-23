//! Track "Properties" dialog (#136): the full metadata of one track, shown
//! as selectable text so any value can be copied. It replaces the accidental
//! per-cell text selection that used to be the only way to copy a tag - and
//! which stole clicks from the row underneath it.

use eframe::egui;

use crate::library_api::TrackInfo;

use super::columns;

/// Shows the modal while `track` is `Some`, and clears it when the user
/// closes it (the Close button, Escape, or a click on the backdrop).
pub fn show(ctx: &egui::Context, track: &mut Option<TrackInfo>) {
    let Some(info) = track.clone() else {
        return;
    };

    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("track_properties")).show(ctx, |ui| {
        ui.set_width(480.0);
        ui.heading("Properties");
        ui.add_space(6.0);

        egui::Grid::new("track_properties_fields")
            .num_columns(2)
            .spacing([16.0, 5.0])
            .show(ui, |ui| {
                field(ui, "Title", columns::title_text(&info));
                field(ui, "Artist", columns::artist_text(&info));
                field(ui, "Album", &info.album);
                field(ui, "Album artist", &info.album_artist);
                field(ui, "Genre", &info.genre);
                field(ui, "Year", &optional(info.year));
                field(ui, "Track", &optional(info.track_no));
                field(ui, "Disc", &optional(info.disc_no));
                field(ui, "Composer", &info.composer);
                field(ui, "Duration", &columns::format_duration(info.duration));
                field(ui, "Format", &format_text(&info));
                field(ui, "Bitrate", &bitrate_text(info.bitrate));
                field(ui, "Sample rate", &sample_rate_text(info.sample_rate));
                field(ui, "File", &info.path);
                field(ui, "Comment", &info.comment);
            });

        ui.add_space(10.0);
        if ui.button("Close").clicked() {
            close = true;
        }
    });

    if close || modal.should_close() {
        *track = None;
    }
}

/// One label/value pair in the grid. Empty values show an em dash so the
/// field still reads as "present but blank" rather than a missing label.
fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).strong());
    let text = if value.trim().is_empty() {
        "—"
    } else {
        value
    };
    ui.add(egui::Label::new(text).selectable(true).wrap());
    ui.end_row();
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// `format (codec)`, e.g. `flac (FLAC)`; just the format when the codec is
/// unknown.
fn format_text(track: &TrackInfo) -> String {
    if track.codec.is_empty() {
        track.format.clone()
    } else {
        format!("{} ({})", track.format, track.codec)
    }
}

fn bitrate_text(bitrate: Option<u32>) -> String {
    bitrate.map(|b| format!("{b} kbps")).unwrap_or_default()
}

fn sample_rate_text(sample_rate: Option<u32>) -> String {
    sample_rate
        .map(|rate| format!("{rate} Hz"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_text_appends_codec_when_known() {
        let mut track = TrackInfo {
            format: "flac".to_string(),
            codec: "FLAC".to_string(),
            ..TrackInfo::default()
        };
        assert_eq!(format_text(&track), "flac (FLAC)");
        track.codec.clear();
        assert_eq!(format_text(&track), "flac");
    }

    #[test]
    fn bitrate_and_sample_rate_are_human_readable() {
        assert_eq!(bitrate_text(Some(320)), "320 kbps");
        assert_eq!(bitrate_text(None), "");
        assert_eq!(sample_rate_text(Some(44_100)), "44100 Hz");
        assert_eq!(sample_rate_text(None), "");
    }

    #[test]
    fn optional_renders_none_as_empty() {
        assert_eq!(optional(Some(2020u32)), "2020");
        assert_eq!(optional(None::<u32>), "");
    }
}
