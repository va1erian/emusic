use super::*;

fn view() -> TrackView {
    TrackView {
        id: "abc".into(),
        filename: Some("song.xm".into()),
        directory: String::new(),
        format: "xm".into(),
        kind: "module".into(),
        specialized: true,
        title: Some("Song".into()),
        artist: Some("Artist".into()),
        album_artist: None,
        album: Some("Album".into()),
        album_id: None,
        genre: None,
        year: Some(1994),
        track_no: Some(3),
        disc_no: None,
        duration_secs: Some(125.5),
        subtunes: 1,
        channels: Some(8),
        file_size: 4096,
        hash: "h".into(),
        has_art: false,
        sync_version: 7,
        added_at: 1_600_000_000,
    }
}

// The literal path uses Windows separators, which `Path` only splits on
// Windows; `track_from_view` itself is portable.
#[cfg(windows)]
#[test]
fn maps_a_view_to_a_track() {
    let path = PathBuf::from(r"C:\cache\srv\abc.xm");
    let track = track_from_view(&view(), path.clone());
    assert_eq!(track.path, path);
    assert_eq!(track.filename, "abc.xm");
    assert_eq!(track.ext, "xm");
    assert_eq!(track.kind, TrackKind::Module);
    assert_eq!(track.duration_ms, 125_500);
    assert_eq!(track.channels, Some(8));
    assert_eq!(track.size, 4096);
    assert_eq!(track.title.as_deref(), Some("Song"));
}

#[test]
fn falls_back_to_the_file_name_without_a_title() {
    let mut view = view();
    view.title = None;
    let track = track_from_view(&view, PathBuf::from(r"C:\cache\srv\abc.xm"));
    assert_eq!(track.title.as_deref(), Some("song.xm"));
}

fn set(items: &[&str]) -> std::collections::HashSet<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn incremental_sync_keeps_unchanged_rows() {
    // Existing {a, b}, delta contains only the changed a, no tombstones:
    // b is unchanged and must be kept, not deleted.
    let removed = ids_to_delete(5, set(&["a", "b"]), &set(&["a"]), &[]);
    assert!(removed.is_empty(), "unchanged rows must survive a delta");
}

#[test]
fn incremental_sync_applies_tombstones() {
    let removed = ids_to_delete(5, set(&["a", "b"]), &set(&["a"]), &["b".to_string()]);
    assert_eq!(removed, vec!["b".to_string()]);
}

#[test]
fn summary_mentions_server_and_starred_failures() {
    assert_eq!(summary(3, 0, 0), "Remote sync: 3 track(s)");
    assert_eq!(
        summary(3, 1, 2),
        "Remote sync: 3 track(s), 1 server(s) failed, starred sync failed on 2 server(s)"
    );
}

#[test]
fn full_sync_drops_absent_rows() {
    let removed = ids_to_delete(0, set(&["a", "b", "c"]), &set(&["a"]), &[]);
    let mut removed = removed;
    removed.sort();
    assert_eq!(removed, vec!["b".to_string(), "c".to_string()]);
}
