use std::path::PathBuf;
use std::time::Duration;

use emusic_library::stats::StatsWindow;
use emusic_library::{ArtSource, Store, Track, TrackId, TrackKind};

use crate::library_api::TrackInfo;

use super::Snapshot;

fn sample_track(path: &str) -> Track {
    Track {
        id: TrackId(1),
        path: PathBuf::from(path),
        dir: PathBuf::from(r"C:\music"),
        filename: "song.flac".to_string(),
        ext: "flac".to_string(),
        size: 1_000,
        mtime: 1_700_000_000,
        kind: TrackKind::Stream,
        duration_ms: 210_000,
        bitrate: Some(1_000),
        sample_rate: Some(44_100),
        channels: Some(2),
        title: Some("Song".to_string()),
        artist: Some("Artist".to_string()),
        album_artist: None,
        album: Some("Album".to_string()),
        genre: Some("Rock".to_string()),
        year: Some(2020),
        track_no: Some(1),
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1_700_000_000,
        starred: false,
    }
}

#[test]
fn from_store_builds_track_info() {
    let mut store = Store::open_in_memory().unwrap();
    let mut track = sample_track(r"C:\music\song.flac");
    store
        .upsert_tracks(std::slice::from_mut(&mut track))
        .unwrap();

    let folder = emusic_library::Folder {
        id: 1,
        path: PathBuf::from(r"C:\music"),
        enabled: true,
        watch: false,
    };

    let snapshot = Snapshot::from_store(&store, std::slice::from_ref(&folder)).unwrap();
    assert_eq!(snapshot.tracks.len(), 1);
    let info = &snapshot.tracks[0];
    assert_eq!(info.id, 1);
    assert_eq!(info.title, "Song");
    assert_eq!(info.artist, "Artist");
    assert_eq!(info.album, "Album");
    assert_eq!(info.duration, Duration::from_millis(210_000));
    assert_eq!(snapshot.folders[0].track_count, 1);
}

#[test]
fn from_store_builds_dir_tree_with_counts() {
    let mut store = Store::open_in_memory().unwrap();
    let mut in_album = sample_track(r"C:\music\Album\1.flac");
    in_album.id = TrackId(1);
    in_album.dir = PathBuf::from(r"C:\music\Album");
    let mut at_root = sample_track(r"C:\music\2.flac");
    at_root.id = TrackId(2);
    at_root.path = PathBuf::from(r"C:\music\2.flac");
    store.upsert_tracks(&mut [in_album, at_root]).unwrap();

    let snapshot = Snapshot::from_store(&store, &[]).unwrap();
    assert_eq!(snapshot.dirs.len(), 1);
    let root = &snapshot.dirs[0];
    let music = &root.children[0];
    assert_eq!(music.name, "music");
    assert_eq!(music.direct_track_count, 1);
    assert_eq!(music.total_track_count, 2);
    assert_eq!(music.children[0].name, "Album");
}

#[test]
fn from_store_maps_history_and_most_played_windows() {
    let mut store = Store::open_in_memory().unwrap();
    let mut track = sample_track(r"C:\music\song.flac");
    store
        .upsert_tracks(std::slice::from_mut(&mut track))
        .unwrap();
    let id = track.id;

    store
        .record_play(&emusic_core::PlayEvent::new(id, unix_now(), 120_000, true))
        .unwrap();
    store
        .record_play(&emusic_core::PlayEvent::new(
            id,
            unix_now() - 10,
            5_000,
            false,
        ))
        .unwrap();

    let mut snapshot = Snapshot::from_store(&store, &[]).unwrap();

    // History: newest first, both the completed play and the skip, with the
    // played track resolved for double-click playback.
    assert_eq!(snapshot.history.len(), 2);
    let entry = &snapshot.history[0];
    assert!(entry.id > 0);
    assert_eq!(entry.track_id, id.0 as u64);
    assert_eq!(entry.title, "Song");
    assert_eq!(entry.artist, "Artist");
    assert_eq!(entry.played_ms, 120_000);
    assert!(entry.completed);
    assert!(!snapshot.history[1].completed);

    // Most played counts only completed plays, but the same ranking is
    // available for every window.
    for window in StatsWindow::ALL {
        let ranked = snapshot.most_played(window);
        assert_eq!(ranked.len(), 1, "{window:?}");
        assert_eq!(ranked[0].play_count, 1, "{window:?}");
    }

    // Removing the newest entry leaves only the skip in the in-memory list.
    let removed = snapshot.history[0].id;
    snapshot.remove_history(removed);
    assert_eq!(snapshot.history.len(), 1);
    assert!(!snapshot.history[0].completed);

    snapshot.clear_history();
    assert!(snapshot.history.is_empty());
    for window in StatsWindow::ALL {
        assert!(snapshot.most_played(window).is_empty(), "{window:?}");
    }
}

#[test]
fn record_play_updates_snapshot_play_count() {
    let mut snapshot = Snapshot::default();
    snapshot.tracks.push(TrackInfo {
        id: 1,
        path: r"C:\music\song.flac".to_string(),
        play_count: 3,
        ..Default::default()
    });

    let record = emusic_library::stats::PlayRecord {
        path: PathBuf::from(r"C:\music\song.flac"),
        started_at: 1_700_000_000,
        listened_ms: 120_000,
        completed: true,
    };
    snapshot.record_play(&record);

    assert_eq!(snapshot.tracks[0].play_count, 4);
    assert!(snapshot.tracks[0].last_played_minutes_ago.is_some());
}

#[test]
fn from_store_carries_the_starred_flag() {
    let mut store = Store::open_in_memory().unwrap();
    let mut track = sample_track(r"C:\music\song.flac");
    store
        .upsert_tracks(std::slice::from_mut(&mut track))
        .unwrap();
    store.set_starred(track.id, true).unwrap();

    let snapshot = Snapshot::from_store(&store, &[]).unwrap();
    assert!(snapshot.tracks[0].starred);
}

#[test]
fn set_starred_updates_the_track_and_rankings() {
    let mut snapshot = Snapshot::default();
    snapshot.tracks.push(TrackInfo {
        id: 1,
        ..Default::default()
    });
    snapshot.most_played_all.push(TrackInfo {
        id: 1,
        ..Default::default()
    });

    snapshot.set_starred(1, true);
    assert!(snapshot.tracks[0].starred);
    assert!(snapshot.most_played_all[0].starred);
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
