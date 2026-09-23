//! [`LibraryDataSource`] backed by the generated mock data.

use super::data::{self, GeneratedLibrary};
use crate::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, FolderInfo, GenreInfo, HistoryEntry, LibraryDataSource,
    StatsWindow, TrackInfo,
};

pub struct MockLibrary {
    data: GeneratedLibrary,
    scanning: bool,
    status: Option<String>,
}

impl MockLibrary {
    pub fn new() -> Self {
        Self {
            data: data::generate(),
            scanning: false,
            status: None,
        }
    }

    /// An empty library, for screenshots of the first-run empty state (#19).
    pub fn empty() -> Self {
        Self {
            data: empty_data(),
            scanning: false,
            status: None,
        }
    }

    /// An empty library that is mid-scan, for screenshots of the first-run
    /// "building your music library" state (#80): no tracks are loaded yet,
    /// but a scan is running and reporting progress.
    pub fn scanning() -> Self {
        Self {
            data: empty_data(),
            scanning: true,
            status: Some("Scanning 750 / 6,096 - track-0750.flac".to_string()),
        }
    }
}

/// Empty generated data, shared by the first-run mock states.
fn empty_data() -> GeneratedLibrary {
    GeneratedLibrary {
        tracks: Vec::new(),
        albums: Vec::new(),
        artists: Vec::new(),
        genres: Vec::new(),
        folders: Vec::new(),
        dirs: Vec::new(),
        history: Vec::new(),
        most_played_all: Vec::new(),
        most_played_30d: Vec::new(),
        most_played_year: Vec::new(),
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

    fn genres(&self) -> &[GenreInfo] {
        &self.data.genres
    }

    fn folders(&self) -> &[FolderInfo] {
        &self.data.folders
    }

    fn dir_tree(&self) -> &[DirNodeInfo] {
        &self.data.dirs
    }

    fn history(&self) -> &[HistoryEntry] {
        &self.data.history
    }

    fn most_played(&self, window: StatsWindow) -> &[TrackInfo] {
        match window {
            StatsWindow::AllTime => &self.data.most_played_all,
            StatsWindow::Last30Days => &self.data.most_played_30d,
            StatsWindow::LastYear => &self.data.most_played_year,
        }
    }

    fn remove_history_entry(&mut self, id: i64) {
        self.data.history.retain(|entry| entry.id != id);
    }

    fn clear_history(&mut self) {
        self.data.history.clear();
        self.data.most_played_all.clear();
        self.data.most_played_30d.clear();
        self.data.most_played_year.clear();
    }

    fn set_starred(&mut self, id: u64, starred: bool) {
        for track in self.data.tracks.iter_mut().filter(|track| track.id == id) {
            track.starred = starred;
        }
        for list in [
            &mut self.data.most_played_all,
            &mut self.data.most_played_30d,
            &mut self.data.most_played_year,
        ] {
            for track in list.iter_mut().filter(|track| track.id == id) {
                track.starred = starred;
            }
        }
    }

    fn is_scanning(&self) -> bool {
        self.scanning
    }

    fn status_text(&self) -> Option<String> {
        self.status.clone()
    }
}
