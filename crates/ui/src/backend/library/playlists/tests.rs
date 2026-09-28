use std::path::PathBuf;

use emusic_library::{ArtSource, Store, Track, TrackId, TrackKind};

use super::super::LibraryBackend;
use super::super::test_support::unique_temp_dir;
use crate::library_api::LibraryDataSource;

fn track(name: &str) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: PathBuf::from(format!("/music/{name}.flac")),
        dir: PathBuf::from("/music"),
        filename: format!("{name}.flac"),
        ext: "flac".into(),
        size: 1,
        mtime: 1,
        kind: TrackKind::Stream,
        duration_ms: 1_000,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1,
        starred: false,
    }
}

/// A file-backed store holding tracks `a`, `b`, `c`; returns its path and ids.
fn seeded_db() -> (PathBuf, [u64; 3]) {
    let dir = unique_temp_dir("playlists");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.db");
    let mut store = Store::open(&path).unwrap();
    let mut tracks = vec![track("a"), track("b"), track("c")];
    store.upsert_tracks(&mut tracks).unwrap();
    let ids = [
        tracks[0].id.0 as u64,
        tracks[1].id.0 as u64,
        tracks[2].id.0 as u64,
    ];
    (path, ids)
}

fn track_ids(backend: &LibraryBackend, id: u64) -> Vec<u64> {
    backend
        .playlist_items(id)
        .iter()
        .map(|i| i.track_id)
        .collect()
}

#[test]
fn playlist_lifecycle_updates_the_cache_and_revision() {
    let (path, [a, b, c]) = seeded_db();
    let mut backend =
        LibraryBackend::with_store(Store::open(&path).unwrap(), None, Default::default());
    assert!(backend.playlists().is_empty());

    let before = backend.revision();
    let id = backend.create_playlist("Mix").unwrap();
    assert_ne!(backend.revision(), before);
    backend.add_to_playlist(id, &[a, b, c, a]);
    assert_eq!(track_ids(&backend, id), [a, b, c, a]);
    assert_eq!(backend.playlists()[0].track_count, 4);

    let entries: Vec<i64> = backend
        .playlist_items(id)
        .iter()
        .map(|i| i.entry_id)
        .collect();
    backend.move_in_playlist(id, &[entries[3]], 0);
    assert_eq!(track_ids(&backend, id), [a, a, b, c]);
    backend.remove_from_playlist(id, &[entries[1]]);
    assert_eq!(track_ids(&backend, id), [a, a, c]);

    backend.rename_playlist(id, "Renamed");
    assert_eq!(backend.playlists()[0].name, "Renamed");
    backend.delete_playlist(id);
    assert!(backend.playlists().is_empty());
    assert!(backend.playlist_items(id).is_empty());
}

#[test]
fn playlists_survive_reopening_the_store() {
    let (path, [a, b, _]) = seeded_db();
    {
        let mut backend =
            LibraryBackend::with_store(Store::open(&path).unwrap(), None, Default::default());
        let id = backend.create_playlist("Keep").unwrap();
        backend.add_to_playlist(id, &[b, a]);
    }

    let backend = LibraryBackend::with_store(Store::open(&path).unwrap(), None, Default::default());
    let list = backend.playlists();
    assert_eq!((list[0].name.as_str(), list[0].track_count), ("Keep", 2));
    assert_eq!(track_ids(&backend, list[0].id), [b, a]);
}
