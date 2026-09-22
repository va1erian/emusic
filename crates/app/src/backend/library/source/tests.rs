use std::path::PathBuf;
use std::time::Duration;

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
