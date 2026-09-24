//! Column definitions for the track table: identity, header label, sizing
//! and each column's text for a given track.
//!
//! Moved from the egui frontend (#93) without the cell painters, which stay
//! there; sorting (`sort`) builds on [`ColumnId`].

use serde::{Deserialize, Serialize};

use crate::library_api::TrackInfo;

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
