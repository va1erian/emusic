//! Verifies the fix for #138 against a real, initialized [`bass::Bass`]
//! instance: tracker modules are actually scanned into the library, with
//! real tag data read from the file (not just a bare filename row), and a
//! plain incremental rescan picks up modules that a prior BASS-less scan
//! had skipped — no forced full rescan required.
//!
//! Per `AGENTS.md`, BASS DLLs are never committed: this test skips
//! gracefully (rather than failing) when `Bass::init` can't find them,
//! which is the expected case in CI and on a fresh checkout. On a machine
//! with `EMUSIC_BASS_DIR` (or `<exe dir>/bass/`) pointing at a real BASS
//! install, it exercises the real module tag reader end-to-end.
//!
//! The `.mod`/`.xm` fixtures here are minimal-but-valid files synthesized
//! by hand (a bare 31-instrument ProTracker header and a bare XM header,
//! each carrying a distinct title and instrument/sample name) rather than
//! binary fixtures, so the tags asserted on below are known in advance.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use bass::Bass;

use emusic_library::scanner::{CancelToken, ScanOptions, scan};
use emusic_library::{Store, TrackKind};

/// Initializes BASS on device `0`, the "no sound" device, so the test
/// never produces audible output and needs no real output device.
///
/// Returns `None` (after printing why) when `bass.dll` is absent, so the
/// test can skip instead of failing; any other error is unexpected.
fn init_silent() -> Option<Bass> {
    match Bass::init(0, 44100) {
        Ok(bass) => Some(bass),
        Err(bass::BassError::DllNotFound(detail)) => {
            eprintln!("skipping: bass.dll not available ({detail})");
            None
        }
        Err(other) => panic!("unexpected error initializing BASS: {other}"),
    }
}

fn temp_root(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("emusic-module-scan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A minimal valid 31-instrument ProTracker `.mod`: a 20-byte title, 31
/// instrument slots (only the first carries a name and one word of silent
/// sample data), a 1-entry order table, the `M.K.` (4-channel) signature,
/// one empty pattern (1024 bytes) and that one sample's 2 bytes of data.
/// Real enough for `BASS_MusicLoad` to open and tag, with no dependency on
/// a binary fixture file.
fn write_minimal_mod(path: &Path, title: &str, sample_name: &str) {
    let mut out = Vec::new();

    let mut title_bytes = [0u8; 20];
    let title_src = title.as_bytes();
    title_bytes[..title_src.len().min(20)].copy_from_slice(&title_src[..title_src.len().min(20)]);
    out.extend_from_slice(&title_bytes);

    for i in 0..31 {
        let mut name = [0u8; 22];
        let (length_words, volume, repeat_length) = if i == 0 {
            let src = sample_name.as_bytes();
            name[..src.len().min(22)].copy_from_slice(&src[..src.len().min(22)]);
            (1u16, 64u8, 1u16)
        } else {
            (0u16, 0u8, 0u16)
        };
        out.extend_from_slice(&name);
        out.extend_from_slice(&length_words.to_be_bytes());
        out.push(0); // finetune
        out.push(volume);
        out.extend_from_slice(&0u16.to_be_bytes()); // repeat offset
        out.extend_from_slice(&repeat_length.to_be_bytes());
    }

    out.push(1); // song length: one entry in the order table
    out.push(127); // historic restart byte
    let mut order = [0u8; 128];
    order[0] = 0;
    out.extend_from_slice(&order);
    out.extend_from_slice(b"M.K."); // 4-channel signature

    out.extend(std::iter::repeat_n(0u8, 1024)); // one empty pattern
    out.extend_from_slice(&[0u8, 0u8]); // instrument 0's 2 bytes of sample data

    std::fs::write(path, out).unwrap();
}

/// Scans `root` and returns the resulting tracks, using `bass` (or `None`)
/// for module tag reading.
fn scan_root(store: &mut Store, root: &Path, bass: Option<Arc<Bass>>) -> Vec<emusic_core::Track> {
    let options = ScanOptions {
        bass,
        ..Default::default()
    };
    let (tx, _rx) = mpsc::channel();
    scan(
        store,
        std::slice::from_ref(&root.to_path_buf()),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();
    store.load_all_tracks().unwrap()
}

// Both scenarios below share a single `#[test]` function and a single
// `Bass` instance: BASS's `BASS_Init` state is process-global (`bass`
// enforces a single live instance), and Rust runs a test binary's `#[test]`
// functions concurrently by default, so two tests each calling `Bass::init`
// would race and one would see `BassError::AlreadyInitialized`. This
// mirrors `crates/player/tests/bass_integration.rs`'s same constraint.
#[test]
fn module_scanning_with_a_real_bass_instance() {
    let Some(bass) = init_silent() else {
        return;
    };
    let bass = Arc::new(bass);

    // Scenario 1: a module is scanned into the library with real tag data,
    // not just a bare filename row.
    let root = temp_root("with-bass");
    write_minimal_mod(
        &root.join("tune.mod"),
        "Emusic Test Tune",
        "Kick Drum Sample",
    );

    let mut store = Store::open_in_memory().unwrap();
    let tracks = scan_root(&mut store, &root, Some(bass.clone()));

    assert_eq!(
        tracks.len(),
        1,
        "the module should be scanned into the library"
    );
    let track = &tracks[0];
    assert_eq!(track.kind, TrackKind::Module);
    // Real tag data, not a bare filename row: title comes from the module
    // header, genre is derived, and the sample name shows up in the
    // comment BASS reads via BASS_ChannelGetTags.
    assert_eq!(track.title.as_deref(), Some("Emusic Test Tune"));
    assert_eq!(track.genre.as_deref(), Some("Tracker (MOD)"));
    let comment = track.comment.as_deref().unwrap_or_default();
    assert!(
        comment.contains("Kick Drum Sample"),
        "comment should include the sample name, got: {comment:?}"
    );
    std::fs::remove_dir_all(&root).ok();

    // Scenario 2 (regression check for the PR's central claim): a module
    // previously skipped by a BASS-less scan is never written as a DB row
    // at all, so a later scan with BASS wired through picks it up as "new"
    // through the ordinary incremental path (new/changed files only) — no
    // forced full rescan is needed.
    let root = temp_root("incremental-pickup");
    write_minimal_mod(&root.join("tune.mod"), "Picked Up Later", "Lead Synth");

    let mut store = Store::open_in_memory().unwrap();

    // First scan: no BASS, matching today's `ScanOptions::default()` bug.
    // The module is skipped and — critically — no row is inserted for it.
    let first_pass = scan_root(&mut store, &root, None);
    assert!(
        first_pass.is_empty(),
        "without bass the module must be skipped, not stored as a bare row"
    );

    // Second scan: same store, same root, now with bass wired through, and
    // nothing else changed on disk. This is exactly an ordinary
    // incremental rescan (e.g. a watch-triggered one), not a forced full
    // rescan.
    let second_pass = scan_root(&mut store, &root, Some(bass));
    assert_eq!(
        second_pass.len(),
        1,
        "an ordinary incremental rescan should pick up the module as new"
    );
    assert_eq!(second_pass[0].title.as_deref(), Some("Picked Up Later"));
    std::fs::remove_dir_all(&root).ok();
}
