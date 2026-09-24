//! A controllable [`LibraryDataSource`] for view-model unit tests (#104).
//!
//! Tests set the lists and the [`LibraryDataSource::revision`] signal directly
//! and can read back how often the lists were queried, so they can prove that
//! an unchanged revision does no rebuilding work.

use std::cell::Cell;

use crate::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, FolderInfo, GenreInfo, HistoryEntry, LibraryDataSource,
    StatsWindow, TrackInfo,
};

/// A library whose artist/genre lists and revision are set by the test, and
/// which counts how often each list is read.
#[derive(Debug, Default)]
pub(crate) struct RiggedLibrary {
    /// Artists returned by [`LibraryDataSource::artists`].
    pub artists: Vec<ArtistInfo>,
    /// Genres returned by [`LibraryDataSource::genres`].
    pub genres: Vec<GenreInfo>,
    /// Tracks returned by [`LibraryDataSource::tracks`].
    pub tracks: Vec<TrackInfo>,
    /// The change signal returned by [`LibraryDataSource::revision`].
    pub revision: Option<u64>,
    /// How many times `artists` has been read.
    pub artist_reads: Cell<usize>,
    /// How many times `genres` has been read.
    pub genre_reads: Cell<usize>,
}

impl RiggedLibrary {
    /// A library reporting `revision`.
    pub(crate) fn new(revision: u64) -> Self {
        Self {
            revision: Some(revision),
            ..Self::default()
        }
    }

    /// An artist row with two albums and one track.
    pub(crate) fn artist(name: &str) -> ArtistInfo {
        ArtistInfo {
            name: name.to_string(),
            track_count: 1,
            album_count: 2,
        }
    }

    /// A genre row with one track.
    pub(crate) fn genre(name: &str) -> GenreInfo {
        GenreInfo {
            name: name.to_string(),
            track_count: 1,
        }
    }
}

impl LibraryDataSource for RiggedLibrary {
    fn tracks(&self) -> &[TrackInfo] {
        &self.tracks
    }

    fn albums(&self) -> &[AlbumInfo] {
        &[]
    }

    fn artists(&self) -> &[ArtistInfo] {
        self.artist_reads.set(self.artist_reads.get() + 1);
        &self.artists
    }

    fn genres(&self) -> &[GenreInfo] {
        self.genre_reads.set(self.genre_reads.get() + 1);
        &self.genres
    }

    fn folders(&self) -> &[FolderInfo] {
        &[]
    }

    fn dir_tree(&self) -> &[DirNodeInfo] {
        &[]
    }

    fn history(&self) -> &[HistoryEntry] {
        &[]
    }

    fn most_played(&self, _window: StatsWindow) -> &[TrackInfo] {
        &[]
    }

    fn revision(&self) -> Option<u64> {
        self.revision
    }
}
