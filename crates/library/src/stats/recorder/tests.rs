use std::path::PathBuf;

use emusic_core::{ArtSource, Track, TrackKind};

use super::*;

fn sample_track(path: &str) -> Track {
    Track {
        id: emusic_core::TrackId::UNASSIGNED,
        path: PathBuf::from(path),
        dir: PathBuf::from(r"C:\music"),
        filename: "a.flac".to_string(),
        ext: "flac".to_string(),
        size: 1_000,
        mtime: 1_700_000_000,
        kind: TrackKind::Stream,
        duration_ms: 200_000,
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
        added_at: 1_700_000_000,
    }
}

#[test]
fn recorder_resolves_path_and_records_a_completed_play() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let track_id = tracks[0].id;
    let store = Arc::new(Mutex::new(store));

    let recorder = StatsRecorder::spawn(store.clone());
    recorder.record(PlayRecord {
        path: PathBuf::from(r"C:\music\a.flac"),
        started_at: 1_000,
        listened_ms: 200_000,
        completed: true,
    });
    recorder.flush();

    let stats = store
        .lock()
        .unwrap()
        .track_stats(track_id)
        .unwrap()
        .unwrap();
    assert_eq!(stats.play_count, 1);
    assert_eq!(stats.skip_count, 0);
}

#[test]
fn recorder_records_a_skip_as_skip_count() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let track_id = tracks[0].id;
    let store = Arc::new(Mutex::new(store));

    let recorder = StatsRecorder::spawn(store.clone());
    recorder.record(PlayRecord {
        path: PathBuf::from(r"C:\music\a.flac"),
        started_at: 1_000,
        listened_ms: 5_000,
        completed: false,
    });
    recorder.flush();

    let stats = store
        .lock()
        .unwrap()
        .track_stats(track_id)
        .unwrap()
        .unwrap();
    assert_eq!(stats.play_count, 0);
    assert_eq!(stats.skip_count, 1);
}

#[test]
fn recorder_drops_records_for_unknown_paths_without_panicking() {
    let store = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
    let recorder = StatsRecorder::spawn(store.clone());

    recorder.record(PlayRecord {
        path: PathBuf::from(r"C:\music\missing.flac"),
        started_at: 1_000,
        listened_ms: 200_000,
        completed: true,
    });
    recorder.flush();

    // No track exists, so nothing should have been written; the important
    // assertion is just that this didn't panic or hang.
}

#[test]
fn dropping_the_recorder_joins_the_writer_thread() {
    let store = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
    let recorder = StatsRecorder::spawn(store);
    drop(recorder);
}
