//! Integration tests that exercise a real `bass.dll`.
//!
//! BASS DLLs are never committed to this repo (see `AGENTS.md`), so every
//! test here must degrade gracefully — skipping with a message rather than
//! failing — when [`Bass::init`] can't find the DLL. This lets
//! `cargo test --workspace` pass on any machine, while still giving real
//! coverage on a dev machine (or CI runner) that has BASS installed under
//! `EMUSIC_BASS_DIR` or `<exe dir>/bass/`.

use bass::{Bass, BassError};

/// Tries to initialize BASS, returning `None` (and printing why) if the DLL
/// simply isn't present — the expected case in this sandboxed environment.
fn try_init() -> Option<Bass> {
    match Bass::init(-1, 44100) {
        Ok(bass) => Some(bass),
        Err(BassError::DllNotFound(detail)) => {
            eprintln!("skipping: bass.dll not available ({detail})");
            None
        }
        Err(other) => panic!("unexpected error initializing BASS: {other}"),
    }
}

#[test]
fn init_and_query_version() {
    let Some(bass) = try_init() else {
        return;
    };
    // Any successfully-parsed version is fine; we're just checking the
    // call round-trips through the loaded library without crashing.
    let _version = bass.version();
}

#[test]
fn enumerate_devices() {
    let Some(bass) = try_init() else {
        return;
    };
    let devices = bass.devices().expect("BASS_GetDeviceInfo should succeed");
    // We can't assert anything about *which* devices exist on an arbitrary
    // machine, only that enumeration itself works.
    for device in &devices {
        assert!(!device.name.is_empty() || !device.driver.is_empty());
    }
}

#[test]
fn load_plugins_from_missing_dir_reports_nothing() {
    let Some(bass) = try_init() else {
        return;
    };
    let results = bass.load_plugins("this/directory/does/not/exist");
    assert!(results.is_empty());
}

#[test]
fn opening_a_missing_file_returns_an_error() {
    let Some(bass) = try_init() else {
        return;
    };
    match bass.open_stream("this/file/does/not/exist.flac", bass::StreamFlags::empty()) {
        Err(BassError::FileOpen | BassError::Unknown(_)) => {}
        Err(other) => panic!("expected FileOpen/Unknown, got {other}"),
        Ok(_) => panic!("expected an error opening a nonexistent file"),
    }
}
