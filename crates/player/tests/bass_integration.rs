//! Plays a generated WAV file through the real [`BassBackend`] /
//! [`Player`], exercising the crate against an actual `bass.dll`.
//!
//! Per `AGENTS.md`, BASS DLLs are never committed: this test skips
//! gracefully (rather than failing) when `Bass::init` can't find them,
//! which is the expected case in CI and on a fresh checkout. On a machine
//! with `EMUSIC_BASS_DIR` (or `<exe dir>/bass/`) pointing at a real BASS
//! install, it exercises real playback end-to-end — on the "no sound"
//! device, so the generated tone stays silent on the developer's speakers.
//!
//! BASS's `BASS_Init`/`BASS_Free` state is process-global (`bass` enforces
//! a single live instance), so this is the only test in the binary and
//! doesn't need to serialize with anything else.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bass::Bass;
use emusic_player::{BassBackend, PlaybackState, Player};

/// Initializes BASS on device `0`, the "no sound" device, so playback runs
/// the same mixing path as a real device without any audible output.
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

/// One second of 44100 Hz mono 16-bit PCM WAV, generated in-memory (no
/// fixture file, no extra dependency).
fn one_second_wav() -> Vec<u8> {
    const RATE: u32 = 44_100;
    let samples: Vec<i16> = (0..RATE as usize)
        .map(|i| {
            let t = i as f64 / f64::from(RATE);
            ((t * 220.0 * std::f64::consts::TAU).sin() * (f64::from(i16::MAX) / 2.0)) as i16
        })
        .collect();

    let data_len = std::mem::size_of_val(samples.as_slice()) as u32;
    let byte_rate = RATE * 2; // 16-bit mono: 2 bytes/sample * 1 channel * rate
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(RATE.to_le_bytes());
    out.extend(byte_rate.to_le_bytes());
    out.extend(2u16.to_le_bytes()); // block align
    out.extend(16u16.to_le_bytes()); // bits per sample
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    out
}

fn write_temp_wav(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("emusic_player_bass_integration_{name}.wav"));
    std::fs::write(&path, one_second_wav()).expect("write temp WAV fixture");
    path
}

#[test]
fn plays_a_generated_wav_through_the_real_backend() {
    let Some(bass) = init_silent() else {
        return;
    };

    let path = write_temp_wav("plays_a_generated_wav_through_the_real_backend");
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
    assert!(playing, "player should reach Playing for a valid WAV file");
    assert_eq!(player.current_path(), Some(path.as_path()));

    let duration = player.duration().expect("duration should be known");
    assert!(
        (duration.as_secs_f64() - 1.0).abs() < 0.05,
        "expected ~1s, got {duration:?}"
    );

    // Let it play to the end and confirm the player stops (single-track
    // queue, repeat off) and reports a finished listen.
    let mut stopped = false;
    for _ in 0..3000 {
        player.tick();
        if player.state() == PlaybackState::Stopped {
            stopped = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(stopped, "player should stop once the only track ends");

    let saw_play_finished = player
        .events()
        .try_iter()
        .any(|event| matches!(event, emusic_player::PlayerEvent::PlayFinished { .. }));
    assert!(
        saw_play_finished,
        "expected a PlayFinished accounting event"
    );

    let _ = std::fs::remove_file(&path);
}
