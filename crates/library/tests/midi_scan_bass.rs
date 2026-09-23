//! Scans a real `.mid` file through a real BASS instance with the
//! `bassmidi` plugin loaded, checking the track lands in the library with
//! its title (from the file name) and its true duration.
//!
//! Skips gracefully when the BASS DLLs or `bassmidi.dll` are unavailable
//! (see `AGENTS.md`): set `EMUSIC_BASS_DIR` to a folder containing them.

use std::sync::Arc;
use std::sync::mpsc;

use bass::Bass;

use emusic_library::scanner::{CancelToken, ScanOptions, scan};
use emusic_library::{Store, TrackKind};

/// A format-0 MIDI file with one middle-C note lasting exactly two seconds
/// (480 ticks per quarter note at 120 bpm, four quarter notes).
fn two_second_midi() -> Vec<u8> {
    let track: &[u8] = &[
        0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // tempo: 500000 us/quarter
        0x00, 0x90, 0x3C, 0x64, // note on, middle C
        0x8F, 0x00, 0x80, 0x3C, 0x40, // 1920 ticks later: note off
        0x00, 0xFF, 0x2F, 0x00, // end of track
    ];
    let mut bytes = b"MThd".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xE0]);
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(track);
    bytes
}

/// Initialises BASS on the "no sound" device with plugins loaded, or `None`
/// (after printing why) when BASS or the MIDI plugin isn't available.
fn init_with_midi() -> Option<Bass> {
    let bass = match Bass::init(0, 44100) {
        Ok(bass) => bass,
        Err(bass::BassError::DllNotFound(detail)) => {
            eprintln!("skipping: bass.dll not available ({detail})");
            return None;
        }
        Err(other) => panic!("unexpected error initializing BASS: {other}"),
    };
    let dir = std::env::var_os("EMUSIC_BASS_DIR")?;
    let loaded = bass.load_plugins(dir).into_iter().any(|plugin| {
        plugin.result.is_ok()
            && plugin
                .path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("bassmidi.dll"))
    });
    if !loaded {
        eprintln!("skipping: bassmidi.dll not available");
        return None;
    }
    Some(bass)
}

#[test]
fn midi_files_are_scanned_with_title_and_duration() {
    let Some(bass) = init_with_midi() else {
        return;
    };
    let root = std::env::temp_dir().join(format!("emusic-midi-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Two Seconds.mid"), two_second_midi()).unwrap();
    std::fs::write(root.join("other.MIDI"), two_second_midi()).unwrap();
    // Optionally scan one real-world file too (`EMUSIC_TEST_MIDI`).
    let real = std::env::var_os("EMUSIC_TEST_MIDI").map(std::path::PathBuf::from);
    if let Some(real) = &real {
        std::fs::copy(real, root.join("real.mid")).unwrap();
    }

    let mut store = Store::open_in_memory().unwrap();
    let options = ScanOptions {
        bass: Some(Arc::new(bass)),
        ..Default::default()
    };
    let (tx, _rx) = mpsc::channel();
    let summary = scan(
        &mut store,
        std::slice::from_ref(&root),
        &options,
        &tx,
        &CancelToken::default(),
    )
    .unwrap();

    assert_eq!(summary.tracks_added, if real.is_some() { 3 } else { 2 });
    let tracks = store.load_all_tracks().unwrap();
    if real.is_some() {
        let real_track = tracks.iter().find(|t| t.filename == "real.mid").unwrap();
        assert!(real_track.duration_ms > 1000, "real MIDI duration unknown");
        eprintln!("real MIDI scanned: {} ms", real_track.duration_ms);
    }
    let track = tracks
        .iter()
        .find(|track| track.filename == "Two Seconds.mid")
        .expect("the .mid file should be in the library");
    assert_eq!(track.kind, TrackKind::Stream);
    assert_eq!(track.title.as_deref(), Some("Two Seconds"));
    assert_eq!(track.genre.as_deref(), Some("MIDI"));
    assert!(
        (1900..=2100).contains(&track.duration_ms),
        "unexpected duration {} ms",
        track.duration_ms
    );
    std::fs::remove_dir_all(&root).ok();
}
