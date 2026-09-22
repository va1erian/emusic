//! Turning a library scope into the [`Command::ShuffleScope`] sent to the
//! player (#57).
//!
//! The scopes are resolved from the UI's [`TrackInfo`] snapshot rather than
//! the library index, because that is the data the shell already holds; the
//! index offers the same mapping (`LibraryIndex::scope_track_ids`) for
//! non-UI callers.

use std::path::Path;

use crate::library_api::{AlbumInfo, LibraryDataSource, TrackInfo};
use crate::state::Command;

/// Shuffle the whole collection.
pub fn all(library: &dyn LibraryDataSource) -> Command {
    let ids = library.tracks().iter().map(|track| track.id).collect();
    Command::ShuffleScope {
        ids,
        label: "All tracks".to_string(),
    }
}

/// Shuffle one album.
pub fn album(library: &dyn LibraryDataSource, album: &AlbumInfo) -> Command {
    let ids = library
        .tracks()
        .iter()
        .filter(|track| belongs_to(track, album))
        .map(|track| track.id)
        .collect();
    Command::ShuffleScope {
        ids,
        label: format!("Album — {}", album.name),
    }
}

/// Shuffle one artist's tracks.
pub fn artist(library: &dyn LibraryDataSource, name: &str) -> Command {
    let ids = library
        .tracks()
        .iter()
        .filter(|track| track.artist.eq_ignore_ascii_case(name))
        .map(|track| track.id)
        .collect();
    Command::ShuffleScope {
        ids,
        label: format!("Artist — {name}"),
    }
}

/// Shuffle one genre's tracks.
pub fn genre(library: &dyn LibraryDataSource, name: &str) -> Command {
    let ids = library
        .tracks()
        .iter()
        .filter(|track| genre_matches(&track.genre, name))
        .map(|track| track.id)
        .collect();
    Command::ShuffleScope {
        ids,
        label: format!("Genre — {name}"),
    }
}

/// Shuffle a directory's tracks, optionally including subdirectories.
pub fn folder(library: &dyn LibraryDataSource, path: &str, recursive: bool) -> Command {
    let ids = library
        .tracks()
        .iter()
        .filter(|track| folder_matches(&track.path, path, recursive))
        .map(|track| track.id)
        .collect();
    Command::ShuffleScope {
        ids,
        label: format!("Folder — {}", folder_name(path)),
    }
}

/// Whether `track` belongs to `album`: same rule as the album grid.
fn belongs_to(track: &TrackInfo, album: &AlbumInfo) -> bool {
    track.album == album.name && (track.artist == album.artist || track.artist.is_empty())
}

/// Whether a raw genre tag contains `name` as one of its `;`/`/`/`,`-split
/// parts (the same split the library index uses).
fn genre_matches(tag: &str, name: &str) -> bool {
    tag.split(&[';', '/', ','][..])
        .any(|part| part.trim().eq_ignore_ascii_case(name))
}

/// Whether `track_path` is inside `folder` (directly, or in any descendant
/// when `recursive`).
fn folder_matches(track_path: &str, folder: &str, recursive: bool) -> bool {
    let track = Path::new(track_path);
    if recursive {
        track.starts_with(folder)
    } else {
        track
            .parent()
            .is_some_and(|parent| parent == Path::new(folder))
    }
}

/// Display name of a folder path (its last component).
fn folder_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u64, path: &str, artist: &str, album: &str, genre: &str) -> TrackInfo {
        TrackInfo {
            id,
            path: path.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            genre: genre.to_string(),
            ..TrackInfo::default()
        }
    }

    struct FakeLibrary(Vec<TrackInfo>);

    impl LibraryDataSource for FakeLibrary {
        fn tracks(&self) -> &[TrackInfo] {
            &self.0
        }
        fn albums(&self) -> &[AlbumInfo] {
            &[]
        }
        fn artists(&self) -> &[crate::library_api::ArtistInfo] {
            &[]
        }
        fn genres(&self) -> &[String] {
            &[]
        }
        fn folders(&self) -> &[crate::library_api::FolderInfo] {
            &[]
        }
        fn dir_tree(&self) -> &[crate::library_api::DirNodeInfo] {
            &[]
        }
        fn history(&self) -> &[crate::library_api::HistoryEntry] {
            &[]
        }
        fn most_played(&self, _window: crate::library_api::StatsWindow) -> &[TrackInfo] {
            &[]
        }
    }

    fn ids(command: Command) -> Vec<u64> {
        match command {
            Command::ShuffleScope { ids, .. } => ids,
            other => panic!("expected ShuffleScope, got {other:?}"),
        }
    }

    fn library() -> FakeLibrary {
        FakeLibrary(vec![
            track(1, r"C:\music\A\one.flac", "Alpha", "First", "Rock"),
            track(2, r"C:\music\A\two.flac", "Alpha", "First", "Rock"),
            track(3, r"C:\music\B\three.flac", "Beta", "Second", "Jazz; Live"),
            track(4, r"C:\music\B\Sub\four.flac", "Beta", "Second", "Jazz"),
        ])
    }

    #[test]
    fn all_covers_every_track() {
        assert_eq!(ids(all(&library())), vec![1, 2, 3, 4]);
    }

    #[test]
    fn album_filters_by_name_and_artist() {
        let second = AlbumInfo {
            name: "Second".to_string(),
            artist: "Beta".to_string(),
            ..AlbumInfo::default()
        };
        assert_eq!(ids(album(&library(), &second)), vec![3, 4]);
    }

    #[test]
    fn artist_matches_case_insensitively() {
        assert_eq!(ids(artist(&library(), "alpha")), vec![1, 2]);
    }

    #[test]
    fn genre_matches_any_split_part() {
        assert_eq!(ids(genre(&library(), "live")), vec![3]);
        assert_eq!(ids(genre(&library(), "jazz")), vec![3, 4]);
    }

    #[test]
    fn folder_is_recursive_only_when_asked() {
        let shallow = ids(folder(&library(), r"C:\music\B", false));
        assert_eq!(shallow, vec![3]);
        let deep = ids(folder(&library(), r"C:\music\B", true));
        assert_eq!(deep, vec![3, 4]);
    }
}
