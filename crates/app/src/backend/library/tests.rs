//! End-to-end tests for the real library backend: a generated WAV is scanned
//! on a background thread, swapped into the snapshot, and a play recorded
//! through the player's channel updates the in-memory stats.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use emusic_library::Store;
use emusic_library::scanner::CancelToken;
use emusic_library::stats::PlayRecord;

use super::scan::ScanHandle;
use super::{LibraryBackend, Update, scan};
use crate::library_api::LibraryDataSource;

#[test]
fn backend_starts_empty_and_accepts_folders() {
    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store);
    assert!(backend.tracks().is_empty());

    // A folder that does not exist yields no tracks, but the backend stays
    // usable and never panics while the loader runs.
    backend.set_folders(&[PathBuf::from(r"Z:\definitely\missing")]);
    backend.tick();
    assert!(backend.tracks().is_empty());
}

#[test]
fn scans_generated_wav_and_tracks_play() {
    let dir = unique_temp_dir("scan");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("track.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store);
    backend.set_folders(std::slice::from_ref(&dir));

    let tracks = wait_for_tracks(&mut backend);
    assert_eq!(tracks.len(), 1, "expected the generated WAV to be scanned");
    let track = &tracks[0];
    assert!(track.path.ends_with("track.wav"));
    assert!(track.duration > Duration::ZERO);
    assert_eq!(backend.folders()[0].track_count, 1);

    let path = PathBuf::from(&track.path);
    backend
        .play_record_tx()
        .send(PlayRecord {
            path,
            started_at: unix_now(),
            listened_ms: 1_000,
            completed: true,
        })
        .unwrap();
    backend.tick();
    assert_eq!(backend.tracks()[0].play_count, 1);
    assert!(backend.tracks()[0].last_played_minutes_ago.is_some());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rescan_picks_up_files_added_after_startup() {
    let dir = unique_temp_dir("rescan");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("first.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store);
    backend.set_folders(std::slice::from_ref(&dir));
    assert_eq!(wait_for_tracks(&mut backend).len(), 1);

    write_wav(&dir.join("second.wav"), 8_000, 1);
    backend.rescan();
    wait_for_track_count(&mut backend, 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn removing_a_folder_purges_its_tracks() {
    let dir = unique_temp_dir("remove");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("track.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store);
    backend.set_folders(std::slice::from_ref(&dir));
    assert_eq!(wait_for_tracks(&mut backend).len(), 1);

    backend.set_folders(&[]);
    wait_for_track_count(&mut backend, 0);
    assert!(backend.folders().is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

/// A file-backed store so the scan can open its own connection (#69), unlike
/// the in-memory store the other tests use.
///
/// Regression: the scan used to keep the shared `Mutex<Store>` locked for its
/// whole run, so every UI-side store call (folder edits, snapshot refreshes)
/// blocked behind a minutes-long network scan. The scan now writes through a
/// private connection, so the shared lock stays free while it runs; this test
/// asserts that an unrelated `list_folders` succeeds *during* the scan.
#[test]
fn scan_does_not_hold_the_shared_store_lock() {
    let dir = unique_temp_dir("nonblocking");
    std::fs::create_dir_all(&dir).unwrap();
    // Enough files that the scan is observably in progress while the loop
    // below runs; a couple of thousand keeps it comfortably past the first
    // progress event.
    for i in 0..500 {
        write_wav(&dir.join(format!("track-{i:03}.wav")), 8_000, 1);
    }

    let db_path = dir.join("library.db");
    let store = Store::open(&db_path).unwrap();
    let folder = store.add_folder(&dir).unwrap();
    let shared = Arc::new(Mutex::new(store));

    let (tx, rx) = std::sync::mpsc::channel();
    let handle = ScanHandle {
        cancel: CancelToken::default(),
        id: 1,
    };
    scan::spawn(
        shared.clone(),
        vec![folder],
        vec![dir.clone()],
        tx,
        handle,
        Vec::new(),
    );

    let mut scanning = false;
    let mut finished = false;
    let mut lock_free_during_scan = false;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && !finished {
        while let Ok(update) = rx.try_recv() {
            match update {
                Update::Status(text) if !text.is_empty() => scanning = true,
                Update::ScanFinished(_) => finished = true,
                _ => {}
            }
        }
        if scanning {
            // A UI-side store call must not wait on the scan: acquire the
            // shared lock and read the folder list, releasing immediately.
            if let Ok(store) = shared.try_lock() {
                lock_free_during_scan = true;
                assert_eq!(store.list_folders().unwrap().len(), 1);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(finished, "scan did not finish within the timeout");
    assert!(
        lock_free_during_scan,
        "the shared store lock was held for the whole scan"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Pumps the backend until the background scan produces a track, up to a
/// generous timeout (network drives are slow; local temp dirs are not).
fn wait_for_tracks(backend: &mut LibraryBackend) -> Vec<crate::library_api::TrackInfo> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        backend.tick();
        if !backend.tracks().is_empty() {
            return backend.tracks().to_vec();
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("scan did not produce a track within the timeout");
}

/// Pumps the backend until it reports exactly `expected` tracks.
fn wait_for_track_count(backend: &mut LibraryBackend, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        backend.tick();
        if backend.tracks().len() == expected {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!(
        "expected {expected} tracks, still have {} after the timeout",
        backend.tracks().len()
    );
}

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("emusic-lib-{tag}-{}-{nanos}", std::process::id()))
}

/// Writes a minimal valid 16-bit PCM mono WAV with a short silent tone.
fn write_wav(path: &Path, sample_rate: u32, seconds: u32) {
    let samples = sample_rate * seconds;
    let data_len = samples * 2; // 16-bit mono
    let byte_rate = sample_rate * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let phase = (i as f32 / sample_rate as f32) * 440.0 * std::f32::consts::TAU;
        let sample = (phase.sin() * 1000.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
