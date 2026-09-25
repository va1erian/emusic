//! Album identity and sort order for the album grid (#17).
//!
//! Shared identity for the grid: [`AlbumInfo`] carries no stable id, so an
//! album is its (name, artist) pair.

use serde::{Deserialize, Serialize};

use crate::library_api::{AlbumInfo, TrackInfo};

/// Order the album grid is sorted by.
///
/// Serialized (lowercase) as part of the UI state saved on exit (#214).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumSort {
    #[default]
    Artist,
    Album,
    Year,
    RecentlyAdded,
}

impl AlbumSort {
    /// Every sort option, in menu order.
    pub const ALL: [Self; 4] = [Self::Artist, Self::Album, Self::Year, Self::RecentlyAdded];

    /// Label shown in the sort menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Year => "Year",
            Self::RecentlyAdded => "Recently added",
        }
    }
}

/// Identity of an album within the view; `AlbumInfo` has no id yet.
///
/// Serialized as part of the UI state saved on exit (#214).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlbumKey {
    pub name: String,
    pub artist: String,
}

impl AlbumKey {
    /// Key for an album row.
    pub fn of(album: &AlbumInfo) -> Self {
        Self {
            name: album.name.clone(),
            artist: album.artist.clone(),
        }
    }

    /// Key for a track, or `None` when the artist tag is missing (the track
    /// cannot be attributed to a specific same-named album).
    pub(crate) fn of_track(track: &TrackInfo) -> Option<Self> {
        (!track.artist.is_empty()).then(|| Self {
            name: track.album.clone(),
            artist: track.artist.clone(),
        })
    }
}
