//! Resolving a shuffle scope to the track ids it covers, straight from the
//! in-memory index.
//!
//! A shuffle over the whole collection, an album, an artist, a genre or a
//! directory only needs the scope's [`TrackId`]s; the player then shuffles
//! them lazily (see `crates/player`'s `ShuffleSource`), so the whole list
//! never has to be loaded into a visible queue.

use std::path::PathBuf;

use emusic_core::TrackId;

use super::LibraryIndex;

/// The set of tracks a scoped shuffle plays over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShuffleScope {
    /// Every track in the library.
    All,
    /// Tracks in a directory. When `recursive`, subdirectories are included.
    Directory { path: PathBuf, recursive: bool },
    /// Tracks whose effective artist (album artist, falling back to track
    /// artist) matches `name`, case-insensitively.
    Artist { name: String },
    /// Tracks on the album with this title and album artist,
    /// case-insensitively.
    Album { title: String, artist: String },
    /// Tracks tagged with this genre (one of the names split out of the
    /// genre tag), case-insensitively.
    Genre { name: String },
}

impl LibraryIndex {
    /// The track ids covered by `scope`, in index order.
    pub fn scope_track_ids(&self, scope: &ShuffleScope) -> Vec<TrackId> {
        match scope {
            ShuffleScope::All => self.tracks().map(|track| track.id).collect(),
            ShuffleScope::Directory { path, recursive } => self
                .tracks()
                .filter(|track| {
                    if *recursive {
                        track.path.starts_with(path)
                    } else {
                        track.dir == *path
                    }
                })
                .map(|track| track.id)
                .collect(),
            ShuffleScope::Artist { name } => self
                .artists()
                .iter()
                .find(|artist| artist.name.eq_ignore_ascii_case(name))
                .map_or_else(Vec::new, |artist| self.ids_for_slots(&artist.track_slots)),
            ShuffleScope::Album { title, artist } => self
                .albums()
                .iter()
                .find(|album| {
                    album.title.eq_ignore_ascii_case(title)
                        && album.artist.eq_ignore_ascii_case(artist)
                })
                .map_or_else(Vec::new, |album| self.ids_for_slots(&album.track_slots)),
            ShuffleScope::Genre { name } => self
                .genres()
                .iter()
                .find(|genre| genre.name.eq_ignore_ascii_case(name))
                .map_or_else(Vec::new, |genre| self.ids_for_slots(&genre.track_slots)),
        }
    }

    /// Maps grouping slots to track ids, skipping any slot whose track was
    /// removed since the grouping was built.
    fn ids_for_slots(&self, slots: &[usize]) -> Vec<TrackId> {
        slots
            .iter()
            .filter_map(|&slot| self.tracks.get(slot).and_then(Option::as_ref))
            .map(|track| track.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use emusic_core::{Track, TrackId, TrackKind};

    use super::*;

    fn track(id: i64, path: &str, artist: &str, album: &str, genre: &str) -> Track {
        let path_str = if cfg!(not(windows)) {
            path.replace('\\', "/")
        } else {
            path.to_string()
        };
        let path = PathBuf::from(&path_str);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Track {
            id: TrackId(id),
            dir,
            filename: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            ext: "flac".to_string(),
            size: 1,
            mtime: 1,
            kind: TrackKind::Stream,
            duration_ms: 1000,
            bitrate: None,
            sample_rate: None,
            channels: None,
            title: None,
            artist: Some(artist.to_string()),
            album_artist: Some(artist.to_string()),
            album: Some(album.to_string()),
            genre: Some(genre.to_string()),
            year: None,
            track_no: None,
            disc_no: None,
            composer: None,
            comment: None,
            art_source: emusic_core::ArtSource::None,
            added_at: 1,
            starred: false,
        }
    }

    fn index() -> LibraryIndex {
        LibraryIndex::build(vec![
            track(1, r"C:\music\A\one.flac", "Alpha", "First", "Rock"),
            track(2, r"C:\music\A\two.flac", "Alpha", "First", "Rock"),
            track(3, r"C:\music\B\three.flac", "Beta", "Second", "Jazz"),
            track(4, r"C:\music\B\Sub\four.flac", "Beta", "Second", "Jazz"),
        ])
    }

    fn ids(ids: Vec<TrackId>) -> Vec<i64> {
        ids.into_iter().map(|id| id.0).collect()
    }

    #[test]
    fn all_scope_covers_every_track() {
        let index = index();
        assert_eq!(
            ids(index.scope_track_ids(&ShuffleScope::All)),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn artist_scope_matches_case_insensitively() {
        let index = index();
        let scope = ShuffleScope::Artist {
            name: "alpha".to_string(),
        };
        assert_eq!(ids(index.scope_track_ids(&scope)), vec![1, 2]);
    }

    #[test]
    fn album_scope_matches_title_and_artist() {
        let index = index();
        let scope = ShuffleScope::Album {
            title: "second".to_string(),
            artist: "Beta".to_string(),
        };
        assert_eq!(ids(index.scope_track_ids(&scope)), vec![3, 4]);
    }

    #[test]
    fn genre_scope_covers_tagged_tracks() {
        let index = index();
        let scope = ShuffleScope::Genre {
            name: "jazz".to_string(),
        };
        assert_eq!(ids(index.scope_track_ids(&scope)), vec![3, 4]);
    }

    #[test]
    fn directory_scope_is_recursive_when_asked() {
        let index = index();
        let dir_path_str = if cfg!(not(windows)) {
            r"C:\music\B".replace('\\', "/")
        } else {
            r"C:\music\B".to_string()
        };
        let shallow = ShuffleScope::Directory {
            path: PathBuf::from(&dir_path_str),
            recursive: false,
        };
        assert_eq!(ids(index.scope_track_ids(&shallow)), vec![3]);

        let recursive = ShuffleScope::Directory {
            path: PathBuf::from(&dir_path_str),
            recursive: true,
        };
        assert_eq!(ids(index.scope_track_ids(&recursive)), vec![3, 4]);
    }

    #[test]
    fn unknown_scope_is_empty() {
        let index = index();
        let scope = ShuffleScope::Artist {
            name: "Nobody".to_string(),
        };
        assert!(index.scope_track_ids(&scope).is_empty());
    }
}
