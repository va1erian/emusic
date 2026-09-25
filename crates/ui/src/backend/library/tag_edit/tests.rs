//! End-to-end tests for the background tag-edit worker: a batch is requested
//! through the [`LibraryDataSource`] trait and, once the worker finishes, the
//! file, the store and the UI snapshot all reflect the edit.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use emusic_library::Store;
use emusic_library::tags::{EditOutcome, EditRequest, EditableTags, read_tags};

use super::super::LibraryBackend;
use super::super::test_support::{unique_temp_dir, wait_for_tracks, write_wav};
use crate::library_api::LibraryDataSource;

#[test]
fn tag_edit_updates_file_store_and_snapshot() {
    let dir = unique_temp_dir("tag-edit");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("track.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
    backend.set_folders(std::slice::from_ref(&dir));
    let path = PathBuf::from(&wait_for_tracks(&mut backend)[0].path);

    backend.request_tag_edits(vec![EditRequest::new(
        &path,
        EditableTags {
            title: Some("Edited Title".to_string()),
            artist: Some("Edited Artist".to_string()),
            ..Default::default()
        },
    )]);

    let outcomes = wait_for_tag_edit_results(&mut backend);
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.is_ok(), "{:?}", outcomes[0].result);

    // The file on disk holds the requested tags.
    let tags = read_tags(&path).unwrap();
    assert_eq!(tags.title.as_deref(), Some("Edited Title"));
    assert_eq!(tags.artist.as_deref(), Some("Edited Artist"));

    // The snapshot was reloaded from the store, so the UI sees the new tags.
    assert_eq!(backend.tracks()[0].title, "Edited Title");
    assert_eq!(backend.tracks()[0].artist, "Edited Artist");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn failed_tag_edit_reports_error_and_leaves_snapshot_unchanged() {
    let dir = unique_temp_dir("tag-edit-missing");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("track.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
    backend.set_folders(std::slice::from_ref(&dir));
    let original_title = wait_for_tracks(&mut backend)[0].title.clone();

    backend.request_tag_edits(vec![EditRequest::new(
        dir.join("does-not-exist.wav"),
        EditableTags {
            title: Some("Nope".to_string()),
            ..Default::default()
        },
    )]);

    let outcomes = wait_for_tag_edit_results(&mut backend);
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.is_err());

    // No edit succeeded, so no snapshot reload happened: the scanned track is
    // unchanged.
    assert_eq!(backend.tracks()[0].title, original_title);

    std::fs::remove_dir_all(&dir).ok();
}

/// Pumps the backend until the tag-edit worker reports an outcome.
fn wait_for_tag_edit_results(backend: &mut LibraryBackend) -> Vec<EditOutcome> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        backend.tick();
        let results = backend.take_tag_edit_results();
        if !results.is_empty() {
            return results;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("tag edit did not finish within the timeout");
}
