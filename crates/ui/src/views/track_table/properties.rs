//! Toolkit-agnostic content of the track "Properties" dialog (#280): the
//! sectioned, labelled fields both frontends show, formatted for display.
//!
//! The dialog itself is frontend-owned (an `egui::Modal` in `crates/app`, a
//! native modal window in `crates/win32`); the field list, grouping and value
//! formatting live here so the two never drift apart.

use crate::library_api::{TrackInfo, format_minutes_ago};

use super::columns;

/// Shown for a value that is present but blank, so the field still reads as
/// "known to be empty" rather than a missing label.
pub const EMPTY: &str = "—";

/// One labelled value row in the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyField {
    pub label: &'static str,
    /// Display-ready value; [`EMPTY`] when the tag is absent.
    pub value: String,
}

/// A titled group of rows ("Tags", "Format", ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertySection {
    pub title: &'static str,
    pub fields: Vec<PropertyField>,
}

/// The dialog's sections, top to bottom, formatted for display.
pub fn sections(track: &TrackInfo) -> Vec<PropertySection> {
    vec![
        PropertySection {
            title: "Tags",
            fields: vec![
                field("Title", columns::title_text(track)),
                field("Artist", columns::artist_text(track)),
                field("Album", &track.album),
                field("Album artist", &track.album_artist),
                field("Genre", &track.genre),
                field("Year", optional(track.year)),
                field("Track", optional(track.track_no)),
                field("Disc", optional(track.disc_no)),
                field("Composer", &track.composer),
                field("Comment", &track.comment),
            ],
        },
        PropertySection {
            title: "Format",
            fields: vec![
                field("Format", format_text(track)),
                field("Duration", columns::format_duration(track.duration)),
                field("Bitrate", bitrate_text(track.bitrate)),
                field("Sample rate", sample_rate_text(track.sample_rate)),
                field("Bit depth", bit_depth_text(track.bit_depth)),
                field("Channels", channels_text(track.channels)),
            ],
        },
        PropertySection {
            title: "File",
            fields: vec![field("Location", &track.path)],
        },
        PropertySection {
            title: "Playback",
            fields: vec![
                field("Plays", track.play_count.to_string()),
                field(
                    "Last played",
                    last_played_text(track.last_played_minutes_ago),
                ),
            ],
        },
    ]
}

/// One row; an empty value becomes [`EMPTY`].
fn field(label: &'static str, value: impl Into<String>) -> PropertyField {
    let value = value.into();
    PropertyField {
        label,
        value: if value.trim().is_empty() {
            EMPTY.to_owned()
        } else {
            value
        },
    }
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

fn bit_depth_text(bit_depth: Option<u8>) -> String {
    bit_depth
        .map(|bits| format!("{bits}-bit"))
        .unwrap_or_default()
}

/// `1`/`2` channels read as "Mono"/"Stereo"; anything else as `N channels`.
fn channels_text(channels: Option<u8>) -> String {
    match channels {
        Some(1) => "Mono".to_owned(),
        Some(2) => "Stereo".to_owned(),
        Some(count) => format!("{count} channels"),
        None => String::new(),
    }
}

fn last_played_text(minutes_ago: Option<u32>) -> String {
    minutes_ago.map_or_else(|| "Never".to_owned(), format_minutes_ago)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn track() -> TrackInfo {
        TrackInfo {
            title: "Song".to_owned(),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            genre: "Rock".to_owned(),
            year: Some(2020),
            track_no: Some(3),
            duration: Duration::from_secs(65),
            path: r"C:\music\a.flac".to_owned(),
            format: "flac".to_owned(),
            codec: "FLAC".to_owned(),
            bitrate: Some(900),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            channels: Some(2),
            play_count: 7,
            last_played_minutes_ago: Some(90),
            ..TrackInfo::default()
        }
    }

    fn value(sections: &[PropertySection], label: &str) -> String {
        sections
            .iter()
            .flat_map(|section| &section.fields)
            .find(|field| field.label == label)
            .unwrap_or_else(|| panic!("no field labelled {label:?}"))
            .value
            .clone()
    }

    #[test]
    fn sections_cover_tags_format_file_and_playback() {
        let sections = sections(&track());
        let titles: Vec<&str> = sections.iter().map(|section| section.title).collect();
        assert_eq!(titles, ["Tags", "Format", "File", "Playback"]);
        assert_eq!(value(&sections, "Title"), "Song");
        assert_eq!(value(&sections, "Location"), r"C:\music\a.flac");
        assert_eq!(value(&sections, "Plays"), "7");
        assert_eq!(value(&sections, "Last played"), "1 h ago");
    }

    #[test]
    fn empty_values_render_as_an_em_dash() {
        let sections = sections(&TrackInfo::default());
        assert_eq!(value(&sections, "Album"), EMPTY);
        assert_eq!(value(&sections, "Year"), EMPTY);
        assert_eq!(value(&sections, "Bitrate"), EMPTY);
        assert_eq!(value(&sections, "Title"), "(unknown title)");
        assert_eq!(value(&sections, "Last played"), "Never");
    }

    #[test]
    fn format_values_are_human_readable() {
        let sections = sections(&track());
        assert_eq!(value(&sections, "Format"), "flac (FLAC)");
        assert_eq!(value(&sections, "Duration"), "1:05");
        assert_eq!(value(&sections, "Bitrate"), "900 kbps");
        assert_eq!(value(&sections, "Sample rate"), "44100 Hz");
        assert_eq!(value(&sections, "Bit depth"), "16-bit");
        assert_eq!(value(&sections, "Channels"), "Stereo");
    }

    #[test]
    fn channels_have_names_for_mono_and_stereo() {
        assert_eq!(channels_text(Some(1)), "Mono");
        assert_eq!(channels_text(Some(2)), "Stereo");
        assert_eq!(channels_text(Some(6)), "6 channels");
        assert_eq!(channels_text(None), "");
    }

    #[test]
    fn format_text_drops_the_codec_when_unknown() {
        let mut track = track();
        track.codec.clear();
        assert_eq!(format_text(&track), "flac");
    }
}
