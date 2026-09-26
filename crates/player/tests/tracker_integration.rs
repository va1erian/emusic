//! Plays a generated tracker module through the real [`BassBackend`] /
//! [`Player`], proving modules are opened with `PRESCAN`: a known duration and
//! a working seek (the transport bar otherwise stays dead, #362).
//!
//! Per `AGENTS.md`, BASS DLLs are never committed: this test skips gracefully
//! when `Bass::init` can't find them. On a machine with `EMUSIC_BASS_DIR` (or
//! `<exe dir>/bass/`) pointing at a real BASS install it exercises the real
//! backend on the "no sound" device.
//!
//! BASS's `BASS_Init`/`BASS_Free` state is process-global, so this is the only
//! test in the binary.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bass::Bass;
use emusic_player::{BassBackend, PlaybackState, Player, PlayerEvent};

/// Initializes BASS on device `0`, the "no sound" device, so playback runs
/// the same mixing path as a real device without any audible output.
///
/// Returns `None` (after printing why) when `bass.dll` is absent, so the
/// test can skip instead of failing.
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

/// A minimal, valid ProTracker `M.K.` module: 20-byte empty title, 31 empty
/// sample headers, one order, the `M.K.` signature and one empty 64-row
/// pattern. With the default tempo (125 BPM, speed 6) one pattern lasts
/// exactly 64 * 6 * 2.5 / 125 = 7.68 s.
fn minimal_mod() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(std::iter::repeat_n(0u8, 20)); // title
    bytes.extend(std::iter::repeat_n(0u8, 31 * 30)); // sample headers
    bytes.push(1); // song length: one order
    bytes.push(0); // restart position
    bytes.extend(std::iter::repeat_n(0u8, 128)); // order table
    bytes.extend(b"M.K."); // signature
    bytes.extend(std::iter::repeat_n(0u8, 64 * 4 * 4)); // one empty pattern
    bytes
}

fn write_temp_mod(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("emusic_player_tracker_{name}.mod"));
    std::fs::write(&path, minimal_mod()).expect("write temp MOD fixture");
    path
}

#[test]
fn tracker_module_reports_duration_and_seeks() {
    let Some(bass) = init_silent() else {
        return;
    };

    let path = write_temp_mod(&std::process::id().to_string());
    let backend = Arc::new(BassBackend::new(Arc::new(bass)));
    let mut player = Player::new(backend);

    player.replace_and_play(vec![path.clone()], 0);

    // Opening happens on a worker thread; wait (with a generous timeout)
    // for it to come up playing.
    let mut playing = false;
    for _ in 0..2000 {
        player.tick();
        if player.state() == PlaybackState::Playing {
            playing = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(playing, "player should reach Playing for a valid MOD file");

    // Without PRESCAN the backend can't report a length at all (#362).
    let duration = player
        .duration()
        .expect("tracker module duration should be known");
    assert!(
        (duration.as_secs_f64() - 7.68).abs() < 0.1,
        "expected ~7.68s, got {duration:?}"
    );

    // A seek must reach BASS and land near the requested position rather
    // than failing with `BASS_ERROR_POSITION`.
    player.seek(duration / 2);
    assert!(
        !player
            .events()
            .try_iter()
            .any(|event| matches!(event, PlayerEvent::Error(_))),
        "seek should not emit an error event"
    );
    let position = player.position().expect("position after seek");
    assert!(
        position >= duration / 4,
        "seek to {:?} should move the position, got {position:?}",
        duration / 2
    );

    let _ = std::fs::remove_file(&path);
}
