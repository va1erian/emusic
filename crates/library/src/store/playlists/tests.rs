use std::path::PathBuf;

use emusic_core::{ArtSource, Track, TrackKind};

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
        starred: false,
    }
}

/// A store with three tracks and an empty playlist.
fn setup() -> (Store, PlaylistId, [TrackId; 3]) {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks: Vec<Track> = ["a", "b", "c"]
        .iter()
        .map(|n| sample_track(&format!(r"C:\music\{n}.flac")))
        .collect();
    store.upsert_tracks(&mut tracks).unwrap();
    let playlist = store.create_playlist("Mix", 10).unwrap();
    (store, playlist, [tracks[0].id, tracks[1].id, tracks[2].id])
}

fn order(store: &Store, id: PlaylistId) -> Vec<TrackId> {
    store
        .playlist_entries(id)
        .unwrap()
        .iter()
        .map(|e| e.track_id)
        .collect()
}

#[test]
fn create_rename_delete_and_list() {
    let (store, id, _) = setup();
    let other = store.create_playlist("Other", 20).unwrap();

    assert!(store.rename_playlist(id, "Renamed").unwrap());
    assert!(!store.rename_playlist(PlaylistId(999), "x").unwrap());
    let names: Vec<_> = store
        .playlists()
        .unwrap()
        .into_iter()
        .map(|p| (p.name, p.track_count))
        .collect();
    assert_eq!(names, [("Renamed".into(), 0), ("Other".into(), 0)]);

    assert!(store.delete_playlist(other).unwrap());
    assert!(!store.delete_playlist(other).unwrap());
    assert_eq!(store.playlists().unwrap().len(), 1);
}

#[test]
fn add_appends_in_order_and_allows_duplicates() {
    let (store, id, [a, b, c]) = setup();
    assert_eq!(store.add_to_playlist(id, &[c, a]).unwrap(), 2);
    assert_eq!(store.add_to_playlist(id, &[a, b]).unwrap(), 2);

    assert_eq!(order(&store, id), [c, a, a, b]);
    assert_eq!(store.playlists().unwrap()[0].track_count, 4);
}

#[test]
fn add_skips_unknown_tracks_and_missing_playlists() {
    let (store, id, [a, ..]) = setup();
    assert_eq!(store.add_to_playlist(id, &[TrackId(999), a]).unwrap(), 1);
    assert_eq!(store.add_to_playlist(PlaylistId(999), &[a]).unwrap(), 0);
    assert_eq!(order(&store, id), [a]);
}

#[test]
fn remove_closes_gaps_and_only_touches_its_playlist() {
    let (store, id, [a, b, c]) = setup();
    let other = store.create_playlist("Other", 20).unwrap();
    store.add_to_playlist(id, &[a, b, c]).unwrap();
    store.add_to_playlist(other, &[a]).unwrap();
    let entries = store.playlist_entries(id).unwrap();
    let foreign = store.playlist_entries(other).unwrap()[0].id;

    let removed = store
        .remove_playlist_entries(id, &[entries[1].id, foreign])
        .unwrap();

    assert_eq!(removed, 1, "an entry of another playlist is not removed");
    assert_eq!(order(&store, id), [a, c]);
    assert_eq!(order(&store, other), [a]);
    store.add_to_playlist(id, &[b]).unwrap();
    assert_eq!(order(&store, id), [a, c, b], "positions stay dense");
}

#[test]
fn move_places_the_selection_at_the_target_index() {
    let (store, id, [a, b, c]) = setup();
    store.add_to_playlist(id, &[a, b, c, a]).unwrap();
    let e: Vec<i64> = store
        .playlist_entries(id)
        .unwrap()
        .iter()
        .map(|e| e.id)
        .collect();

    // Last entry to the front.
    store.move_playlist_entries(id, &[e[3]], 0).unwrap();
    assert_eq!(order(&store, id), [a, a, b, c]);

    // First two entries (a, a) to the end.
    store.move_playlist_entries(id, &[e[3], e[0]], 4).unwrap();
    assert_eq!(order(&store, id), [b, c, a, a]);

    // Moving downwards: the index is in the list before the move, so
    // moving `b` to index 2 puts it between c and a.
    store.move_playlist_entries(id, &[e[1]], 2).unwrap();
    assert_eq!(order(&store, id), [c, b, a, a]);
}

#[test]
fn deleting_a_playlist_or_track_removes_its_entries() {
    let (mut store, id, [a, b, _]) = setup();
    store.add_to_playlist(id, &[a, b]).unwrap();

    store
        .delete_tracks_by_paths(&[PathBuf::from(r"C:\music\a.flac")])
        .unwrap();
    assert_eq!(order(&store, id), [b]);

    store.delete_playlist(id).unwrap();
    let left: i64 = store
        .conn
        .query_row("SELECT count(*) FROM playlist_tracks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
}
