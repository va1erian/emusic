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
