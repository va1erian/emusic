use std::path::PathBuf;

use emusic_core::{ArtSource, PlayEvent, Track, TrackId, TrackKind};

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

const DAY: i64 = 24 * 60 * 60;

#[test]
fn history_page_computes_offset_from_page_number() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let id = tracks[0].id;
    for played_at in [1_000, 2_000, 3_000, 4_000, 5_000] {
        store
            .record_play(&PlayEvent::new(id, played_at, 30_000, true))
            .unwrap();
    }

    let first = history_page(&store, 0, 2).unwrap();
    assert_eq!(
        first.iter().map(|e| e.played_at).collect::<Vec<_>>(),
        vec![5_000, 4_000]
    );

    let second = history_page(&store, 1, 2).unwrap();
    assert_eq!(
        second.iter().map(|e| e.played_at).collect::<Vec<_>>(),
        vec![3_000, 2_000]
    );

    let third = history_page(&store, 2, 2).unwrap();
    assert_eq!(
        third.iter().map(|e| e.played_at).collect::<Vec<_>>(),
        vec![1_000]
    );
}

#[test]
fn most_played_windows_map_to_expected_cutoffs() {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks = vec![sample_track(r"C:\music\a.flac")];
    store.upsert_tracks(&mut tracks).unwrap();
    let id = tracks[0].id;

    let now = 1_000_000_000;
    // One play just inside the 30-day window, one well outside it but
    // inside the last-year window, one far outside both.
    store
        .record_play(&PlayEvent::new(id, now - DAY, 200_000, true))
        .unwrap();
    store
        .record_play(&PlayEvent::new(id, now - 200 * DAY, 200_000, true))
        .unwrap();
    store
        .record_play(&PlayEvent::new(id, now - 2_000 * DAY, 200_000, true))
        .unwrap();

    let last_30 = most_played(&store, StatsWindow::Last30Days, 10, now).unwrap();
    assert_eq!(
        last_30,
        vec![MostPlayedEntry {
            track_id: id,
            play_count: 1
        }]
    );

    let last_year = most_played(&store, StatsWindow::LastYear, 10, now).unwrap();
    assert_eq!(
        last_year,
        vec![MostPlayedEntry {
            track_id: id,
            play_count: 2
        }]
    );

    let all_time = most_played(&store, StatsWindow::AllTime, 10, now).unwrap();
    assert_eq!(
        all_time,
        vec![MostPlayedEntry {
            track_id: id,
            play_count: 3
        }]
    );
}
