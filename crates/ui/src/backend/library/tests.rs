//! End-to-end tests for the real library backend: a generated WAV is scanned
//! on a background thread, swapped into the snapshot, and a play recorded
//! through the player's channel updates the in-memory stats.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use emusic_library::Store;
use emusic_library::scanner::CancelToken;
use emusic_library::stats::PlayRecord;
use emusic_metadata::{Candidate, MetadataError, Provider, TrackQuery};

use super::scan::ScanHandle;
use super::test_support::{
    unique_temp_dir, unix_now, wait_for_track_count, wait_for_tracks, write_wav,
};
use super::{LibraryBackend, Update, Updates, scan};
use crate::backend::PlayMessage;
use crate::library_api::{AutoTagOutcome, AutoTagRequest, LibraryDataSource};

/// A [`Waker`](crate::waker::Waker) that counts how often it was woken.
#[derive(Default)]
struct WakeCounter(std::sync::atomic::AtomicUsize);

impl crate::waker::Waker for Arc<WakeCounter> {
    fn wake(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn posting_an_update_wakes_the_frontend() {
    let slot = crate::waker::WakerSlot::new();
    let counter = Arc::new(WakeCounter::default());
    slot.bind(Arc::clone(&counter));
    let (tx, rx) = std::sync::mpsc::channel::<Update>();
    let updates = Updates::new(tx, slot.handle());

    updates.send(Update::Status("scanning".into())).unwrap();
    assert_eq!(counter.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(matches!(rx.try_recv(), Ok(Update::Status(text)) if text == "scanning"));

    // A dropped receiver fails the send and must not schedule a wake.
    drop(rx);
    let _ = updates.send(Update::Status("late".into()));
    assert_eq!(counter.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn backend_starts_empty_and_accepts_folders() {
    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
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
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
    backend.set_folders(std::slice::from_ref(&dir));

    let tracks = wait_for_tracks(&mut backend);
    assert_eq!(tracks.len(), 1, "expected the generated WAV to be scanned");
    let track = &tracks[0];
    assert!(track.path.ends_with("track.wav"));
    assert!(track.duration > Duration::ZERO);
    assert_eq!(backend.folders()[0].track_count, 1);

    let path = PathBuf::from(&track.path);
    let started_at = unix_now();
    // A track is logged the moment it starts, so it is visible in History
    // before it has finished.
    backend
        .play_message_tx()
        .send(PlayMessage::Started {
            path: path.clone(),
            started_at,
        })
        .unwrap();
    backend.tick();
    assert_eq!(backend.history().len(), 1);
    assert_eq!(backend.history()[0].track_id, track.id);
    assert!(!backend.history()[0].finished);

    backend
        .play_message_tx()
        .send(PlayMessage::Finished(PlayRecord {
            path,
            started_at,
            listened_ms: 1_000,
            completed: true,
        }))
        .unwrap();
    backend.tick();
    assert_eq!(backend.tracks()[0].play_count, 1);
    assert!(backend.tracks()[0].last_played_minutes_ago.is_some());
    assert_eq!(backend.history().len(), 1);
    assert!(backend.history()[0].finished);
    assert!(backend.history()[0].completed);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rescan_picks_up_files_added_after_startup() {
    let dir = unique_temp_dir("rescan");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("first.wav"), 8_000, 1);

    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
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
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
    backend.set_folders(std::slice::from_ref(&dir));
    assert_eq!(wait_for_tracks(&mut backend).len(), 1);

    backend.set_folders(&[]);
    wait_for_track_count(&mut backend, 0);
    assert!(backend.folders().is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn auto_tag_lookup_runs_off_thread_and_reports_candidates() {
    let store = Store::open_in_memory().unwrap();
    let mut backend = LibraryBackend::with_store(store, None, Default::default());
    backend.set_auto_tag_provider(Arc::new(StubProvider));

    backend.request_auto_tag(AutoTagRequest {
        path: PathBuf::from("C:/music/a.flac"),
        query: TrackQuery {
            title: Some("Title".to_string()),
            ..Default::default()
        },
    });
    assert!(
        backend.auto_tag_status().is_some(),
        "the status line is set the moment a lookup is requested"
    );

    let outcomes = wait_for_auto_tag_results(&mut backend);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, PathBuf::from("C:/music/a.flac"));
    let candidates = outcomes[0].result.as_ref().expect("the stub succeeds");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title.as_deref(), Some("Found"));
    assert!(
        backend.auto_tag_status().is_none(),
        "the status line clears once the outcome arrives"
    );
}

/// A provider that returns one canned candidate without touching the network.
struct StubProvider;

impl Provider for StubProvider {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn search(&self, _query: &TrackQuery) -> Result<Vec<Candidate>, MetadataError> {
        Ok(vec![Candidate {
            title: Some("Found".to_string()),
            score: 1.0,
            ..Default::default()
        }])
    }
}

/// Pumps the backend until the auto-tag worker reports an outcome.
fn wait_for_auto_tag_results(backend: &mut LibraryBackend) -> Vec<AutoTagOutcome> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        backend.tick();
        let results = backend.take_auto_tag_results();
        if !results.is_empty() {
            return results;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("auto-tag lookup did not finish within the timeout");
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
        Updates::new(tx, Default::default()),
        handle,
        Vec::new(),
        None,
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
