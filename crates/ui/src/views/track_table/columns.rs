//! Column definitions for the track table: identity, header label, sizing
//! and each column's text for a given track.
//!
//! Moved from the egui frontend (#93) without the cell painters, which stay
//! there; sorting (`sort`) builds on [`ColumnId`].

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::library_api::{TrackInfo, format_minutes_ago};

/// Identifies one column. The `#` (row position) column is intentionally
/// excluded here: it is not backed by track data and is never sortable.
///
/// Serialized (lowercase) for the UI state saved on exit (#214).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnId {
    Title,
    Artist,
    Album,
    Year,
    Genre,
    Time,
    Format,
    Plays,
    LastPlayed,
    File,
}

impl ColumnId {
    /// This column's text for `track`, so both frontends format cells
    /// identically. Borrows the track's own strings where possible.
    pub fn cell<'a>(self, track: &'a TrackInfo) -> Cow<'a, str> {
        match self {
            Self::Title => Cow::Borrowed(title_text(track)),
            Self::Artist => Cow::Borrowed(artist_text(track)),
            Self::Album => Cow::Borrowed(&track.album),
            Self::Year => Cow::Owned(track.year.map(|year| year.to_string()).unwrap_or_default()),
            Self::Genre => Cow::Borrowed(&track.genre),
            Self::Time => Cow::Owned(format_duration(track.duration)),
            Self::Format => Cow::Borrowed(&track.format),
            Self::Plays => Cow::Owned(track.play_count.to_string()),
            Self::LastPlayed => Cow::Owned(
                track
                    .last_played_minutes_ago
                    .map(format_minutes_ago)
                    .unwrap_or_default(),
            ),
            Self::File => Cow::Borrowed(&track.path),
        }
    }
}

/// Static description of one column: label, default/min width and whether
/// header clicks should sort by it.
pub struct ColumnSpec {
    pub id: ColumnId,
    pub label: &'static str,
    pub initial_width: f32,
    pub min_width: f32,
}

/// All data columns, left to right (the `#` column is drawn separately by
/// the table widget itself).
// `Title` is deliberately excluded from this list: the table gives it the
// remaining space instead of a fixed initial width, since it's the column
// users most want to read in full and tables can't always scroll
// horizontally to reveal clipped columns.
pub const COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        id: ColumnId::Artist,
        label: "Artist",
        initial_width: 130.0,
        min_width: 70.0,
    },
    ColumnSpec {
        id: ColumnId::Album,
        label: "Album",
        initial_width: 130.0,
        min_width: 70.0,
    },
    ColumnSpec {
        id: ColumnId::Year,
        label: "Year",
        initial_width: 46.0,
        min_width: 40.0,
    },
    ColumnSpec {
        id: ColumnId::Genre,
        label: "Genre",
        initial_width: 84.0,
        min_width: 56.0,
    },
    ColumnSpec {
        id: ColumnId::Time,
        label: "Time",
        initial_width: 50.0,
        min_width: 44.0,
    },
    ColumnSpec {
        id: ColumnId::Format,
        label: "Format",
        initial_width: 52.0,
        min_width: 44.0,
    },
    ColumnSpec {
        id: ColumnId::Plays,
        label: "Plays",
        initial_width: 46.0,
        min_width: 40.0,
    },
    ColumnSpec {
        id: ColumnId::LastPlayed,
        label: "Last played",
        initial_width: 82.0,
        min_width: 64.0,
    },
    ColumnSpec {
        id: ColumnId::File,
        label: "File",
        initial_width: 140.0,
        min_width: 90.0,
    },
];

/// Title column sizing, handled separately from [`COLUMNS`] since it grows
/// to fill leftover space rather than taking a fixed initial width.
pub const TITLE_MIN_WIDTH: f32 = 120.0;

/// Placeholder shown for an empty/unknown tag value, matching the flat list
/// view's existing convention.
pub fn title_text(track: &TrackInfo) -> &str {
    if track.title.is_empty() {
        "(unknown title)"
    } else {
        &track.title
    }
}

/// Placeholder shown for an empty/unknown artist tag.
pub fn artist_text(track: &TrackInfo) -> &str {
    if track.artist.is_empty() {
        "(unknown artist)"
    } else {
        &track.artist
    }
}

/// A duration as `m:ss`, for the Time column.
pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn track() -> TrackInfo {
        TrackInfo {
            title: String::new(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            genre: "Rock".to_string(),
            year: Some(2020),
            duration: Duration::from_secs(65),
            format: "flac".to_string(),
            path: r"C:\music\a.flac".to_string(),
            play_count: 3,
            last_played_minutes_ago: Some(90),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn cell_formats_every_column() {
        let track = track();
        assert_eq!(ColumnId::Title.cell(&track), "(unknown title)");
        assert_eq!(ColumnId::Artist.cell(&track), "Artist");
        assert_eq!(ColumnId::Album.cell(&track), "Album");
        assert_eq!(ColumnId::Year.cell(&track), "2020");
        assert_eq!(ColumnId::Genre.cell(&track), "Rock");
        assert_eq!(ColumnId::Time.cell(&track), "1:05");
        assert_eq!(ColumnId::Format.cell(&track), "flac");
        assert_eq!(ColumnId::Plays.cell(&track), "3");
        assert_eq!(ColumnId::LastPlayed.cell(&track), "1 h ago");
        assert_eq!(ColumnId::File.cell(&track), r"C:\music\a.flac");
    }

    #[test]
    fn cell_renders_missing_values_as_empty() {
        let track = TrackInfo::default();
        assert_eq!(ColumnId::Year.cell(&track), "");
        assert_eq!(ColumnId::LastPlayed.cell(&track), "");
    }
}
