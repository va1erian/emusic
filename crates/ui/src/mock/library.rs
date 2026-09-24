//! [`LibraryDataSource`] backed by the generated mock data.

use super::data::{self, GeneratedLibrary};
use crate::library_api::{
    AlbumInfo, ArtistInfo, AutoTagOutcome, AutoTagRequest, AutoTagStatus, Candidate, DatabaseInfo,
    DirNodeInfo, EditOutcome, EditRequest, EditableTags, FolderInfo, GenreInfo, HistoryEntry,
    LibraryDataSource, StatsWindow, TrackInfo,
};

pub struct MockLibrary {
    data: GeneratedLibrary,
    scanning: bool,
    status: Option<String>,
    /// Tag-edit outcomes not yet drained by the UI (#172). The mock applies
    /// edits to its in-memory tracks synchronously, so a submitted edit is
    /// reflected on the next frame; this just carries the result back.
    tag_edit_results: Vec<EditOutcome>,
    /// Auto-tag outcomes not yet drained by the UI (#208). The mock answers
    /// synchronously with a canned candidate, so screenshots and `--mock`
    /// runs stay offline.
    auto_tag_results: Vec<AutoTagOutcome>,
    /// The auto-tag progress line, when the mock is configured to show one.
    auto_tag_status: Option<AutoTagStatus>,
}

impl MockLibrary {
    pub fn new() -> Self {
        Self {
            data: data::generate(),
            scanning: false,
            status: None,
            tag_edit_results: Vec::new(),
            auto_tag_results: Vec::new(),
            auto_tag_status: None,
        }
    }

    /// An empty library, for screenshots of the first-run empty state (#19).
    pub fn empty() -> Self {
        Self {
            data: empty_data(),
            scanning: false,
            status: None,
            tag_edit_results: Vec::new(),
            auto_tag_results: Vec::new(),
            auto_tag_status: None,
        }
    }

    /// A populated library with an auto-tag lookup in flight, for screenshots
    /// of the status bar's lookup line and Cancel button (#210).
    pub fn auto_tagging() -> Self {
        Self {
            auto_tag_status: Some(AutoTagStatus {
                path: std::path::PathBuf::from("D:/Music/Tracker/Über_Horizon/Disc1/000.it"),
                text: "Looking up tags on MusicBrainz…".to_string(),
                done: 0,
                total: 1,
            }),
            ..Self::new()
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
            auto_tag_results: Vec::new(),
            auto_tag_status: None,
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

    fn database_info(&self) -> DatabaseInfo {
        DatabaseInfo {
            path: Some(std::path::PathBuf::from(
                "C:/Users/you/AppData/Local/emusic/library.db",
            )),
            size_bytes: Some(18_350_080),
            last_scan: Some(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_767_225_600),
            ),
        }
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

    fn request_auto_tag(&mut self, request: AutoTagRequest) {
        let candidate = Candidate {
            title: request
                .query
                .title
                .clone()
                .or_else(|| Some("Mock Title".to_string())),
            artist: request.query.artist.clone(),
            album: request
                .query
                .album
                .clone()
                .or_else(|| Some("Mock Album".to_string())),
            album_artist: request.query.artist.clone(),
            year: Some(2001),
            track_no: Some(1),
            disc_no: Some(1),
            score: 0.95,
            ..Default::default()
        };
        self.auto_tag_results.push(AutoTagOutcome {
            path: request.path,
            result: Ok(vec![candidate]),
        });
    }

    fn take_auto_tag_results(&mut self) -> Vec<AutoTagOutcome> {
        std::mem::take(&mut self.auto_tag_results)
    }

    fn auto_tag_status(&self) -> Option<AutoTagStatus> {
        self.auto_tag_status.clone()
    }

    fn cancel_auto_tag(&mut self) {
        self.auto_tag_status = None;
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
