use std::path::{Path, PathBuf};

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use super::Store;

fn sample_track(path: &str) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: PathBuf::from(path),
        dir: PathBuf::from(path).parent().unwrap().to_path_buf(),
        filename: PathBuf::from(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        ext: "flac".to_string(),
        size: 1_000,
        mtime: 1_700_000_000,
        kind: TrackKind::Stream,
        duration_ms: 200_000,
        bitrate: Some(900),
        sample_rate: Some(44_100),
        channels: Some(2),
        title: Some("Title".to_string()),
        artist: Some("Artist".to_string()),
        album_artist: None,
        album: Some("Album".to_string()),
        genre: Some("Genre".to_string()),
        year: Some(2021),
        track_no: Some(3),
        disc_no: Some(1),
        composer: None,
        comment: None,
        art_source: ArtSource::ExternalFile(PathBuf::from(r"C:\music\Artist\Album\folder.jpg")),
        added_at: 1_700_000_000,
    }
}

#[test]
fn upsert_then_load_round_trips() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    assert!(!tracks[0].id.is_unassigned());

    let loaded = store.load_all_tracks().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0], tracks[0]);
}

#[test]
fn upsert_updates_existing_row_matched_by_path() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let first_id = tracks[0].id;

    tracks[0].title = Some("New Title".to_string());
    store.upsert_tracks(&mut tracks).unwrap();

    assert_eq!(tracks[0].id, first_id, "id should be stable across upserts");
    let loaded = store
        .get_track_by_path(Path::new(r"C:\music\a.flac"))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.title.as_deref(), Some("New Title"));
}

#[test]
fn delete_by_paths_removes_matching_rows() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![
        sample_track(r"C:\music\a.flac"),
        sample_track(r"C:\music\b.flac"),
    ];
    store.upsert_tracks(&mut tracks).unwrap();

    let deleted = store
        .delete_tracks_by_paths(&[PathBuf::from(r"C:\music\a.flac")])
        .unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
}

#[test]
fn get_track_by_path_returns_none_when_missing() {
    let store = Store::open_in_memory().unwrap();
    assert!(
        store
            .get_track_by_path(Path::new(r"C:\nope.flac"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn size_mtime_map_reflects_stored_tracks() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();

    let map = store.size_mtime_map().unwrap();
    assert_eq!(
        map.get(&PathBuf::from(r"C:\music\a.flac")),
        Some(&(1_000, 1_700_000_000))
    );
}

#[test]
fn move_track_preserves_id_and_added_at() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let original_id = tracks[0].id;

    let mut moved = sample_track(r"D:\elsewhere\b.flac");
    moved.title = Some("New Title".to_string());
    moved.added_at = 9_999_999_999; // must be overwritten by the row's original
    assert!(
        store
            .move_track(Path::new(r"C:\music\a.flac"), &mut moved)
            .unwrap()
    );

    assert_eq!(moved.id, original_id);
    assert_eq!(moved.added_at, 1_700_000_000);
    let loaded = store.load_all_tracks().unwrap();
    assert_eq!(loaded.len(), 1, "moving must not leave a second row");
    assert_eq!(loaded[0].path, PathBuf::from(r"D:\elsewhere\b.flac"));
    assert_eq!(loaded[0].id, original_id);
    assert_eq!(loaded[0].title.as_deref(), Some("New Title"));
    assert_eq!(loaded[0].added_at, 1_700_000_000);
}

#[test]
fn move_track_returns_false_for_unknown_path() {
    let mut store = Store::open_in_memory().unwrap();
    let mut track = sample_track(r"C:\music\a.flac");
    assert!(
        !store
            .move_track(Path::new(r"C:\nope.flac"), &mut track)
            .unwrap()
    );
    assert!(track.id.is_unassigned());
}

#[test]
fn play_history_survives_a_move() {
    use emusic_core::PlayEvent;

    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let id = tracks[0].id;
    store
        .record_play(&PlayEvent::new(id, 1_000, 30_000, true))
        .unwrap();

    let mut moved = sample_track(r"C:\music\renamed.flac");
    store
        .move_track(Path::new(r"C:\music\a.flac"), &mut moved)
        .unwrap();

    assert_eq!(moved.id, id);
    let stats = store.track_stats(id).unwrap().unwrap();
    assert_eq!(stats.play_count, 1);
}
