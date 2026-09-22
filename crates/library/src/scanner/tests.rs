//! Integration tests for the library scanner.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use emusic_core::TrackKind;

use crate::scanner::{CancelToken, ScanEvent, ScanOptions, scan};
use crate::store::Store;

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emusic-scan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes a tiny valid PCM WAV file so lofty can parse it.
fn write_wav(path: &Path, duration_ms: u32) {
    let sample_rate = 44_100u32;
    let channels = 2u16;
    let bits = 16u16;
    let bytes_per_sample = (channels * bits / 8) as u32;
    let data_bytes = (duration_ms * sample_rate / 1000) * bytes_per_sample;
    let file_size = 36 + data_bytes;

    let mut file = File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(file_size).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * bytes_per_sample).to_le_bytes())
        .unwrap(); // byte rate
    file.write_all(&(bytes_per_sample as u16).to_le_bytes())
        .unwrap(); // block align
    file.write_all(&bits.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&data_bytes.to_le_bytes()).unwrap();
    file.write_all(&vec![0u8; data_bytes as usize]).unwrap();
}

#[test]
fn scan_adds_new_files() {
    let root = temp_root("adds");
    write_wav(&root.join("a.wav"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &ScanOptions::default(),
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.files_found, 1);
    assert_eq!(summary.tracks_added, 1);
    assert!(!summary.partial);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn unchanged_file_is_not_re_added() {
    let root = temp_root("unchanged");
    write_wav(&root.join("a.wav"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.tracks_added, 0);
    assert_eq!(summary.tracks_updated, 0);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn changed_file_updates_row_in_place() {
    let root = temp_root("changed");
    let path = root.join("a.wav");
    write_wav(&path, 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    write_wav(&path, 200);

    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.tracks_updated, 1);
    assert_eq!(summary.tracks_added, 0);
    assert_eq!(summary.tracks_deleted, 0);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn vanished_file_is_deleted() {
    let root = temp_root("deleted");
    write_wav(&root.join("a.wav"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);

    std::fs::remove_file(root.join("a.wav")).unwrap();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.tracks_deleted, 1);
    assert!(store.load_all_tracks().unwrap().is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn missing_root_makes_scan_partial_without_deleting_rows() {
    let root = temp_root("missing-root");
    write_wav(&root.join("a.wav"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    let missing = std::env::temp_dir().join("emusic-scan-does-not-exist");
    let summary = scan(
        &mut store,
        &[missing],
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert!(summary.partial);
    assert_eq!(summary.unreachable_roots.len(), 1);
    assert_eq!(summary.tracks_deleted, 0);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn cancellation_leaves_rows_intact() {
    let root = temp_root("cancel");
    write_wav(&root.join("a.wav"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    std::fs::remove_file(root.join("a.wav")).unwrap();
    let cancel = CancelToken::default();
    cancel.cancel();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &cancel,
    )
    .unwrap();

    assert!(summary.cancelled);
    assert_eq!(summary.tracks_deleted, 0);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn progress_events_are_sent() {
    let root = temp_root("progress");
    write_wav(&root.join("a.wav"), 100);
    std::fs::create_dir(root.join("sub")).unwrap();
    std::fs::write(root.join("sub").join("b.xm"), b"fake xm").unwrap();

    let mut store = Store::open_in_memory().unwrap();
    let (tx, rx) = mpsc::channel();
    let options = ScanOptions::default();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    // Collect events, ignoring send failures after the receiver is dropped.
    let events: Vec<ScanEvent> = rx.try_iter().collect();
    let found = events
        .iter()
        .filter(|e| matches!(e, ScanEvent::FilesFound { .. }))
        .count();
    let processed = events
        .iter()
        .filter(|e| matches!(e, ScanEvent::FileProcessed { .. }))
        .count();

    assert_eq!(found, 2);
    assert_eq!(processed, 2);
    assert_eq!(summary.files_found, 2);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn move_detection_preserves_history() {
    use emusic_core::PlayEvent;

    let root = temp_root("move");
    let old_path = root.join("a.wav");
    write_wav(&old_path, 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    let id = store.load_all_tracks().unwrap()[0].id;
    store
        .record_play(&PlayEvent::new(id, 1_000, 30_000, true))
        .unwrap();

    std::fs::rename(&old_path, root.join("renamed.wav")).unwrap();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.tracks_moved, 1);
    assert_eq!(summary.tracks_deleted, 0);
    assert_eq!(summary.tracks_added, 0);
    let stats = store.track_stats(id).unwrap().unwrap();
    assert_eq!(stats.play_count, 1);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn module_is_skipped_without_bass() {
    let root = temp_root("module-no-bass");
    std::fs::write(root.join("tune.xm"), b"fake xm").unwrap();

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &ScanOptions::default(),
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.files_found, 1);
    assert_eq!(summary.modules_skipped, 1);
    assert_eq!(summary.tracks_added, 0);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn normalised_path_keys_prevent_duplicates() {
    let root = temp_root("norm");
    write_wav(&root.join("A.WAV"), 100);

    let mut store = Store::open_in_memory().unwrap();
    let (tx, _rx) = mpsc::channel();
    let options = ScanOptions::default();
    scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();
    // Same file spelled with different case/separators must not create a
    // second row on the next scan.
    let lower = PathBuf::from(format!("{}/a.wav", root.to_string_lossy()));
    let summary = scan(&mut store, &[lower], &options, &tx, &CancelToken::default()).unwrap();

    assert_eq!(summary.tracks_added, 0);
    assert_eq!(store.load_all_tracks().unwrap().len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn stream_extensions_are_classified_correctly() {
    use crate::scanner::paths::classify;

    assert_eq!(classify(Path::new("song.flac")), Some(TrackKind::Stream));
    assert_eq!(classify(Path::new("song.mp3")), Some(TrackKind::Stream));
    assert_eq!(classify(Path::new("song.xm")), Some(TrackKind::Module));
    assert_eq!(classify(Path::new("song.txt")), None);
}
