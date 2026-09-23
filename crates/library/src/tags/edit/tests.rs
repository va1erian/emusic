//! Tests for orchestrating a tag edit across the file and the store.

use std::path::Path;
use std::time::UNIX_EPOCH;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use super::{EditRequest, edit_tags};
use crate::store::Store;
use crate::tags::EditableTags;
use crate::tags::test_support::{cleanup, tagged_fixture, temp_dir};

/// A store row whose path points at `path`, seeded with recognizable tags.
fn track_at(path: &Path) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: path.to_path_buf(),
        dir: path.parent().unwrap().to_path_buf(),
        filename: path.file_name().unwrap().to_string_lossy().into_owned(),
        ext: "wav".to_string(),
        size: 0,
        mtime: 0,
        kind: TrackKind::Stream,
        duration_ms: 0,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: Some("Seed Title".to_string()),
        artist: Some("Seed Artist".to_string()),
        album_artist: None,
        album: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        composer: None,
        comment: Some("Seed Comment".to_string()),
        art_source: ArtSource::None,
        added_at: 1_700_000_000,
        starred: false,
    }
}

/// An in-memory store holding one seeded row per path.
fn store_with(paths: &[&Path]) -> Store {
    let mut store = Store::open_in_memory().unwrap();
    let mut tracks: Vec<Track> = paths.iter().map(|path| track_at(path)).collect();
    store.upsert_tracks(&mut tracks).unwrap();
    store
}

/// Tags that only change the title.
fn edited_tags(title: &str) -> EditableTags {
    EditableTags {
        title: Some(title.to_string()),
        ..Default::default()
    }
}

/// The file's mtime as a Unix timestamp (seconds).
fn mtime_of(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn successful_edit_writes_file_and_syncs_store() {
    let path = tagged_fixture("edit-success");
    let mut store = store_with(&[&path]);
    let id = store.get_track_by_path(&path).unwrap().unwrap().id;
    assert!(store.set_starred(id, true).unwrap());

    let tags = EditableTags {
        title: Some("New Title".to_string()),
        artist: Some("New Artist".to_string()),
        ..Default::default()
    };
    let outcomes = edit_tags(&mut store, &[EditRequest::new(&path, tags.clone())]);

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.is_ok(), "{:?}", outcomes[0].result);
    assert_eq!(outcomes[0].path, path);
    // The file on disk now holds the requested tags.
    assert_eq!(crate::tags::read_tags(&path).unwrap(), tags);

    let loaded = store.get_track_by_path(&path).unwrap().unwrap();
    // Identity, added_at and the star survive the edit.
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.added_at, 1_700_000_000);
    assert!(loaded.starred);
    // Tags and file stats are refreshed.
    assert_eq!(loaded.title.as_deref(), Some("New Title"));
    assert_eq!(loaded.artist.as_deref(), Some("New Artist"));
    assert_eq!(loaded.comment, None);
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(loaded.size, metadata.len());
    assert_eq!(loaded.mtime, mtime_of(&metadata));

    cleanup(&path);
}

#[test]
fn missing_file_reports_error_and_leaves_row_unchanged() {
    let dir = temp_dir("edit-missing");
    let missing = dir.join("does-not-exist.wav");
    let mut store = store_with(&[&missing]);

    let outcomes = edit_tags(
        &mut store,
        &[EditRequest::new(&missing, edited_tags("Nope"))],
    );

    assert!(outcomes[0].result.is_err());
    let loaded = store.get_track_by_path(&missing).unwrap().unwrap();
    assert_eq!(loaded.title.as_deref(), Some("Seed Title"));
    assert_eq!(loaded.comment.as_deref(), Some("Seed Comment"));
    cleanup(&missing);
}

#[test]
fn read_only_file_reports_error_and_leaves_row_unchanged() {
    let path = tagged_fixture("edit-read-only");
    let mut store = store_with(&[&path]);

    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();

    let outcomes = edit_tags(
        &mut store,
        &[EditRequest::new(&path, edited_tags("Blocked"))],
    );
    assert!(outcomes[0].result.is_err());

    let loaded = store.get_track_by_path(&path).unwrap().unwrap();
    assert_eq!(loaded.title.as_deref(), Some("Seed Title"));
    assert_eq!(loaded.comment.as_deref(), Some("Seed Comment"));

    // Restore write access so the temp directory can be removed.
    std::fs::set_permissions(&path, original).unwrap();
    cleanup(&path);
}

#[test]
fn a_failed_edit_does_not_block_the_others() {
    let good = tagged_fixture("edit-batch");
    let missing = good.parent().unwrap().join("missing.wav");
    let mut store = store_with(&[&good, &missing]);

    let outcomes = edit_tags(
        &mut store,
        &[
            EditRequest::new(&good, edited_tags("Good")),
            EditRequest::new(&missing, edited_tags("Bad")),
        ],
    );

    assert!(outcomes[0].result.is_ok(), "{:?}", outcomes[0].result);
    assert!(outcomes[1].result.is_err());
    assert_eq!(
        store
            .get_track_by_path(&good)
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Good")
    );
    assert_eq!(
        store
            .get_track_by_path(&missing)
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Seed Title")
    );
    cleanup(&good);
}

#[test]
fn a_batch_of_successful_edits_updates_every_row() {
    let first = tagged_fixture("edit-many");
    let second = first.parent().unwrap().join("second.wav");
    std::fs::copy(&first, &second).unwrap();
    let mut store = store_with(&[&first, &second]);

    let outcomes = edit_tags(
        &mut store,
        &[
            EditRequest::new(&first, edited_tags("First")),
            EditRequest::new(&second, edited_tags("Second")),
        ],
    );

    assert!(outcomes.iter().all(|outcome| outcome.result.is_ok()));
    for (path, title) in [(&first, "First"), (&second, "Second")] {
        assert_eq!(
            store
                .get_track_by_path(path)
                .unwrap()
                .unwrap()
                .title
                .as_deref(),
            Some(title)
        );
    }
    cleanup(&first);
}
