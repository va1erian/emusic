//! [`LibraryDataSource`] backed by the generated mock data.

use super::data::{self, GeneratedLibrary};
use crate::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, EditOutcome, EditRequest, EditableTags, FolderInfo,
    GenreInfo, HistoryEntry, LibraryDataSource, StatsWindow, TrackInfo,
};

pub struct MockLibrary {
    data: GeneratedLibrary,
    scanning: bool,
    status: Option<String>,
    /// Tag-edit outcomes not yet drained by the UI (#172). The mock applies
    /// edits to its in-memory tracks synchronously, so a submitted edit is
    /// reflected on the next frame; this just carries the result back.
    tag_edit_results: Vec<EditOutcome>,
}

impl MockLibrary {
    pub fn new() -> Self {
        Self {
            data: data::generate(),
            scanning: false,
            status: None,
            tag_edit_results: Vec::new(),
        }
    }

    /// An empty library, for screenshots of the first-run empty state (#19).
    pub fn empty() -> Self {
        Self {
            data: empty_data(),
            scanning: false,
            status: None,
            tag_edit_results: Vec::new(),
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
            tag_edit_results: Vec::new(),
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

    fn request_tag_edits(&mut self, requests: Vec<EditRequest>) {
        for request in requests {
            for track in self
                .data
                .tracks
                .iter_mut()
                .filter(|track| is_path(track, &request.path))
            {
                apply_tags(track, &request.tags);
            }
            for list in [
                &mut self.data.most_played_all,
                &mut self.data.most_played_30d,
                &mut self.data.most_played_year,
            ] {
                for track in list
                    .iter_mut()
                    .filter(|track| is_path(track, &request.path))
                {
                    apply_tags(track, &request.tags);
                }
            }
            self.tag_edit_results.push(EditOutcome {
                path: request.path,
                result: Ok(()),
            });
        }
    }

    fn take_tag_edit_results(&mut self) -> Vec<EditOutcome> {
        std::mem::take(&mut self.tag_edit_results)
    }

    fn is_scanning(&self) -> bool {
        self.scanning
    }

    fn status_text(&self) -> Option<String> {
        self.status.clone()
    }
}

/// Applies a full set of editable tags to one track, clearing the fields the
/// form left blank (mirroring the scanner's `None`-not-empty convention).
fn apply_tags(track: &mut TrackInfo, tags: &EditableTags) {
    track.title = tags.title.clone().unwrap_or_default();
    track.artist = tags.artist.clone().unwrap_or_default();
    track.album = tags.album.clone().unwrap_or_default();
    track.album_artist = tags.album_artist.clone().unwrap_or_default();
    track.genre = tags.genre.clone().unwrap_or_default();
    track.year = tags.year.and_then(|year| u32::try_from(year).ok());
    track.track_no = tags.track_no;
    track.disc_no = tags.disc_no;
    track.composer = tags.composer.clone().unwrap_or_default();
    track.comment = tags.comment.clone().unwrap_or_default();
}

/// Whether `track` is the file `path` names.
fn is_path(track: &TrackInfo, path: &std::path::Path) -> bool {
    std::path::Path::new(&track.path) == path
}
