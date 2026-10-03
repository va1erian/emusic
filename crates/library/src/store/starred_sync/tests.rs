use std::collections::HashSet;
use std::path::PathBuf;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use super::{StarredBase, Store};
use crate::RemoteTrack;

fn track(path: &str, title: &str) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: PathBuf::from(path),
        dir: PathBuf::from("dir"),
        filename: path.to_string(),
        ext: "flac".into(),
        size: 1,
        mtime: 1,
        kind: TrackKind::Stream,
        duration_ms: 1000,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: Some(title.into()),
        artist: Some("Artist".into()),
        album_artist: None,
        album: Some("Album".into()),
        genre: None,
        year: None,
        track_no: Some(2),
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: 1,
        starred: false,
    }
}

/// A store with one local track and one remote row for each of two servers.
fn store() -> (Store, TrackId, TrackId) {
    let mut store = Store::open_in_memory().unwrap();
    let mut local = vec![track("local.flac", "Local")];
    store.upsert_tracks(&mut local).unwrap();
    let mut remote = vec![RemoteTrack {
        remote_id: "r1".into(),
        sync_version: 1,
        track: track("srv-r1.flac", "Remote"),
    }];
    store.upsert_remote_tracks("srv", &mut remote).unwrap();
    let mut other = vec![RemoteTrack {
        remote_id: "o1".into(),
        sync_version: 1,
        track: track("other-o1.flac", "Other"),
    }];
    store.upsert_remote_tracks("other", &mut other).unwrap();
    (store, local[0].id, remote[0].track.id)
}

fn row(store: &Store, id: TrackId) -> super::StarSyncRow {
    store
        .star_sync_rows("srv")
        .unwrap()
        .into_iter()
        .find(|row| row.id == id)
        .unwrap()
}

#[test]
fn rows_cover_local_tracks_and_this_server_only() {
    let (store, local, remote) = store();
    let rows = store.star_sync_rows("srv").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(row(&store, local).remote_track_id, None);
    let remote = row(&store, remote);
    assert_eq!(remote.remote_track_id.as_deref(), Some("r1"));
    assert_eq!(remote.title.as_deref(), Some("Remote"));
    assert_eq!(remote.track_no, Some(2));
}

#[test]
fn set_starred_at_stamps_only_actual_changes() {
    let (store, local, _) = store();
    assert_eq!(row(&store, local).changed_at_ms, None);
    assert!(store.set_starred_at(local, true, 100).unwrap());
    assert_eq!(row(&store, local).changed_at_ms, Some(100));
    assert!(row(&store, local).starred);
    // Starring an already starred track keeps the original stamp.
    store.set_starred_at(local, true, 200).unwrap();
    assert_eq!(row(&store, local).changed_at_ms, Some(100));
    store.set_starred_at(local, false, 300).unwrap();
    assert_eq!(row(&store, local).changed_at_ms, Some(300));
    assert!(!store.set_starred_at(TrackId(999), true, 1).unwrap());
}

#[test]
fn base_round_trips_and_is_empty_before_the_first_sync() {
    let (mut store, _, _) = store();
    assert_eq!(store.starred_base("srv").unwrap(), StarredBase::default());
    let base = StarredBase {
        version: Some(4),
        synced_at_ms: 1_000,
        ids: HashSet::from(["a".to_string(), "b".to_string()]),
    };
    store.commit_star_sync("srv", &base, &[]).unwrap();
    assert_eq!(store.starred_base("srv").unwrap(), base);
    assert_eq!(store.starred_base("other").unwrap(), StarredBase::default());
    // The delta-sync version shares the row and is left alone.
    store.set_remote_since_version("srv", 9).unwrap();
    assert_eq!(store.starred_base("srv").unwrap(), base);
    assert_eq!(store.remote_since_version("srv").unwrap(), 9);

    let next = StarredBase {
        version: Some(5),
        synced_at_ms: 2_000,
        ids: HashSet::from(["c".to_string()]),
    };
    store.commit_star_sync("srv", &next, &[]).unwrap();
    assert_eq!(store.starred_base("srv").unwrap(), next);

    store.forget_starred_base("srv").unwrap();
    assert_eq!(store.starred_base("srv").unwrap(), StarredBase::default());
    assert_eq!(store.remote_since_version("srv").unwrap(), 9);
}

#[test]
fn commit_applies_changes_without_stamping_and_spares_newer_clicks() {
    let (mut store, local, remote) = store();
    // The user unstarred the remote row after local state was read (t=1000).
    store.set_starred_at(remote, true, 10).unwrap();
    store.set_starred_at(remote, false, 1_500).unwrap();
    let base = StarredBase {
        version: Some(1),
        synced_at_ms: 1_000,
        ids: HashSet::new(),
    };
    let applied = store
        .commit_star_sync("srv", &base, &[(local, true), (remote, true)])
        .unwrap();
    assert_eq!(applied, 1);
    let local = row(&store, local);
    assert!(local.starred);
    assert_eq!(local.changed_at_ms, None, "applied changes are not stamped");
    assert!(!row(&store, remote).starred, "a newer click must survive");
}
