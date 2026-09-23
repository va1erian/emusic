//! Plays MIDI files through the real [`BassBackend`] / [`Player`] with the
//! `bassmidi` plugin loaded: stream creation, duration, advancing position,
//! seeking and end-of-track handling.
//!
//! Skips gracefully when the BASS DLLs or `bassmidi.dll` are missing (see
//! `AGENTS.md`). Optional environment variables:
//! - `EMUSIC_TEST_SOUNDFONT`: a `.sf2` to render with (playback works
//!   without one, but is silent).
//! - `EMUSIC_TEST_MIDI`: an extra real `.mid` file to play the same way.
//!
//! One test in this binary: BASS's init state is process-global.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bass::Bass;
use emusic_player::{BassBackend, PlaybackState, Player, PlayerEvent};

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

/// Initialises BASS on the "no sound" device with the MIDI plugin loaded, or
/// `None` (after printing why) when either is unavailable.
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
    let loaded = bass.load_plugins(dir).iter().any(|plugin| {
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

/// Ticks `player` until `done` holds, or fails after a generous timeout.
fn wait_until(player: &mut Player, what: &str, done: impl Fn(&Player) -> bool) {
    for _ in 0..5000 {
        player.tick();
        if done(player) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("timed out waiting for {what}");
}

/// Plays `path` and checks duration, position, seeking and (when
/// `to_end`) end-of-track; returns the duration.
fn check_playback(player: &mut Player, path: &Path, to_end: bool) -> Duration {
    player.replace_and_play(vec![path.to_path_buf()], 0);
    wait_until(player, "Playing", |p| p.state() == PlaybackState::Playing);
    let duration = player.duration().expect("duration should be known");
    assert!(duration > Duration::ZERO, "MIDI duration should be known");

    wait_until(player, "position to advance", |p| {
        p.position()
            .is_some_and(|pos| pos > Duration::from_millis(100))
    });
    player.seek(duration / 2);
    let position = player.position().expect("position after seek");
    assert!(
        position >= duration / 2 - Duration::from_millis(100),
        "seek to {:?} landed at {position:?}",
        duration / 2
    );

    if to_end {
        wait_until(player, "the track to end", |p| {
            p.state() == PlaybackState::Stopped
        });
        assert!(
            player
                .events()
                .try_iter()
                .any(|event| matches!(event, PlayerEvent::PlayFinished { .. })),
            "expected a PlayFinished accounting event"
        );
    }
    duration
}

#[test]
fn plays_midi_files_through_the_real_backend() {
    let Some(bass) = init_with_midi() else {
        return;
    };
    let path: PathBuf =
        std::env::temp_dir().join(format!("emusic_player_midi_{}.mid", std::process::id()));
    std::fs::write(&path, two_second_midi()).expect("write temp MIDI fixture");

    let backend = Arc::new(BassBackend::new(Arc::new(bass)));
    let mut player = Player::new(backend);
    let font = std::env::var_os("EMUSIC_TEST_SOUNDFONT").map(PathBuf::from);
    player.set_midi_soundfont(font.as_deref());

    let duration = check_playback(&mut player, &path, true);
    assert!(
        (duration.as_secs_f64() - 2.0).abs() < 0.1,
        "expected ~2s, got {duration:?}"
    );
    let _ = std::fs::remove_file(&path);

    if let Some(real) = std::env::var_os("EMUSIC_TEST_MIDI") {
        let duration = check_playback(&mut player, Path::new(&real), false);
        eprintln!("real MIDI file plays, duration {duration:?}");
    }
}
