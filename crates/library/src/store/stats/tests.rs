use std::path::PathBuf;

use emusic_core::{ArtSource, PlayEvent, Track, TrackKind};

use super::*;

fn sample_track(path: &str) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
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

fn store_with_one_track() -> (Store, TrackId) {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let track_id = tracks[0].id;
    (store, track_id)
}

#[test]
fn record_play_creates_and_updates_stats() {
    let (mut store, track_id) = store_with_one_track();

    store
        .record_play(&PlayEvent::new(track_id, 1_000, 30_000, true))
        .unwrap();
    let stats = store.track_stats(track_id).unwrap().unwrap();
    assert_eq!(stats.play_count, 1);
    assert_eq!(stats.skip_count, 0);
    assert_eq!(stats.last_played_at, Some(1_000));

    store
        .record_play(&PlayEvent::new(track_id, 2_000, 30_000, true))
        .unwrap();
    let stats = store.track_stats(track_id).unwrap().unwrap();
    assert_eq!(stats.play_count, 2);
    assert_eq!(stats.last_played_at, Some(2_000));
}

#[test]
fn record_play_tracks_skips_separately_from_plays() {
    let (mut store, track_id) = store_with_one_track();

    store
        .record_play(&PlayEvent::new(track_id, 1_000, 5_000, false))
        .unwrap();
    let stats = store.track_stats(track_id).unwrap().unwrap();
    assert_eq!(stats.play_count, 0);
    assert_eq!(stats.skip_count, 1);
    assert_eq!(stats.last_played_at, Some(1_000));

    store
        .record_play(&PlayEvent::new(track_id, 2_000, 200_000, true))
        .unwrap();
    let stats = store.track_stats(track_id).unwrap().unwrap();
    assert_eq!(stats.play_count, 1);
    assert_eq!(stats.skip_count, 1);
    assert_eq!(stats.last_played_at, Some(2_000));
}

#[test]
fn track_stats_none_when_never_played() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.track_stats(TrackId(42)).unwrap().is_none());
}

#[test]
fn resolve_track_id_matches_exact_path() {
    let (store, track_id) = store_with_one_track();
    let resolved = store
        .resolve_track_id(Path::new(r"C:\music\a.flac"))
        .unwrap();
    assert_eq!(resolved, Some(track_id));
}

#[test]
fn resolve_track_id_falls_back_to_normalized_key() {
    let (store, track_id) = store_with_one_track();
    // Same file, different spelling: forward slashes and different case.
    let resolved = store
        .resolve_track_id(Path::new("c:/MUSIC/A.FLAC"))
        .unwrap();
    assert_eq!(resolved, Some(track_id));
}

#[test]
fn resolve_track_id_none_for_unknown_path() {
    let (store, _track_id) = store_with_one_track();
    assert!(
        store
            .resolve_track_id(Path::new(r"C:\music\nope.flac"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn play_history_page_orders_newest_first_and_paginates() {
    let (mut store, track_id) = store_with_one_track();
    for played_at in [1_000, 2_000, 3_000] {
        store
            .record_play(&PlayEvent::new(track_id, played_at, 30_000, true))
            .unwrap();
    }

    let page = store.play_history_page(2, 0).unwrap();
    assert_eq!(
        page.iter().map(|e| e.played_at).collect::<Vec<_>>(),
        vec![3_000, 2_000]
    );

    let next_page = store.play_history_page(2, 2).unwrap();
    assert_eq!(
        next_page.iter().map(|e| e.played_at).collect::<Vec<_>>(),
        vec![1_000]
    );
}

#[test]
fn most_played_since_counts_only_completed_plays_in_window() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![
        sample_track(r"C:\music\a.flac"),
        sample_track(r"C:\music\b.flac"),
    ];
    store.upsert_tracks(&mut tracks).unwrap();
    let (a, b) = (tracks[0].id, tracks[1].id);

    // a: two completed plays, one within the window, one old.
    store
        .record_play(&PlayEvent::new(a, 100, 200_000, true))
        .unwrap();
    store
        .record_play(&PlayEvent::new(a, 10_000, 200_000, true))
        .unwrap();
    // a: one skip, should not count.
    store
        .record_play(&PlayEvent::new(a, 10_500, 5_000, false))
        .unwrap();
    // b: one completed play within the window.
    store
        .record_play(&PlayEvent::new(b, 10_100, 200_000, true))
        .unwrap();

    let all_time = store.most_played_since(None, 10).unwrap();
    assert_eq!(
        all_time[0],
        MostPlayedEntry {
            track_id: a,
            play_count: 2
        }
    );
    assert_eq!(
        all_time[1],
        MostPlayedEntry {
            track_id: b,
            play_count: 1
        }
    );

    let windowed = store.most_played_since(Some(5_000), 10).unwrap();
    assert_eq!(windowed.len(), 2);
    assert_eq!(
        windowed[0],
        MostPlayedEntry {
            track_id: a,
            play_count: 1
        }
    );
    assert_eq!(
        windowed[1],
        MostPlayedEntry {
            track_id: b,
            play_count: 1
        }
    );
}
