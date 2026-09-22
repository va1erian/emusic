//! [`LibraryDataSource`] backed by the generated mock data.

use super::data::{self, GeneratedLibrary};
use crate::library_api::{
    AlbumInfo, ArtistInfo, FolderInfo, HistoryEntry, LibraryDataSource, TrackInfo,
};

pub struct MockLibrary {
    data: GeneratedLibrary,
}

impl MockLibrary {
    pub fn new() -> Self {
        Self {
            data: data::generate(),
        }
    }

    /// An empty library, for screenshots of the first-run empty state (#19).
    pub fn empty() -> Self {
        Self {
            data: data::GeneratedLibrary {
                tracks: Vec::new(),
                albums: Vec::new(),
                artists: Vec::new(),
                genres: Vec::new(),
                folders: Vec::new(),
                history: Vec::new(),
                most_played: Vec::new(),
            },
        }
    }
}

impl Default for MockLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryDataSource for MockLibrary {
    fn tracks(&self) -> &[TrackInfo] {
        &self.data.tracks
    }

    fn albums(&self) -> &[AlbumInfo] {
        &self.data.albums
    }

    fn artists(&self) -> &[ArtistInfo] {
        &self.data.artists
    }

    fn genres(&self) -> &[String] {
        &self.data.genres
    }

    fn folders(&self) -> &[FolderInfo] {
        &self.data.folders
    }

    fn history(&self) -> &[HistoryEntry] {
        &self.data.history
    }

    fn most_played(&self) -> &[TrackInfo] {
        &self.data.most_played
    }
}
