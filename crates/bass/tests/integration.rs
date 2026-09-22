//! Integration tests that exercise a real `bass.dll`.
//!
//! BASS DLLs are never committed to this repo (see `AGENTS.md`), so every
//! test here must degrade gracefully — skipping with a message rather than
//! failing — when [`Bass::init`] can't find the DLL. This lets
//! `cargo test --workspace` pass on any machine, while still giving real
//! coverage on a dev machine (or CI runner) that has BASS installed under
//! `EMUSIC_BASS_DIR` or `<exe dir>/bass/`.
//!
//! Tests initialize BASS on the "no sound" device (see [`silent`]) so they
//! mix/process audio identically without beeping out of the speakers.
//!
//! BASS's `BASS_Init`/`BASS_Free` state is process-global (and the crate
//! enforces exactly one live [`Bass`]), so [`silent::init_silent`]
//! serializes access to it.

mod silent;
mod temp;
mod wav;

use bass::{Bass, BassError, Channel, PositionMode, StreamFlags};

use silent::init_silent;

#[test]
fn init_and_query_version() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    // Any successfully-parsed version is fine; we're just checking the
    // call round-trips through the loaded library without crashing.
    let _version = bass.version();
}

#[test]
fn second_init_in_a_process_is_a_clear_error() {
    let Some((_guard, _bass)) = init_silent() else {
        return;
    };
    let second = Bass::init(0, 44100);
    assert!(matches!(second, Err(BassError::AlreadyInitialized)));
}

#[test]
fn enumerate_devices() {
    let Some((_guard, bass)) = init_silent() else {
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
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let results = bass.load_plugins("this/directory/does/not/exist");
    assert!(results.is_empty());
}

#[test]
fn opening_a_missing_file_returns_an_error() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    match bass.open_stream("this/file/does/not/exist.flac", StreamFlags::empty()) {
        Err(BassError::FileOpen | BassError::Unknown(_)) => {}
        Err(other) => panic!("expected FileOpen/Unknown, got {other}"),
        Ok(_) => panic!("expected an error opening a nonexistent file"),
    }
}

#[test]
fn decoding_a_wav_reports_info_length_and_samples() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    // 1 second of 44100 Hz mono 16-bit samples.
    let path = temp::write_file("bass_decode_test", "wav", &wav::mono_file_1s().0);

    let stream = bass
        .open_stream(&path, StreamFlags::DECODE | StreamFlags::FLOAT)
        .expect("decoding a valid WAV file should succeed");

    let info = stream.info().expect("BASS_ChannelGetInfo should succeed");
    assert_eq!(info.freq, 44_100);
    assert_eq!(info.channels, 1);
    assert_eq!(info.format_name, "WAV PCM");

    // Because the stream was created with FLOAT, BASS reports the length
    // in decoded 32-bit-float bytes (44100 samples * 4).
    let bytes = stream.length(PositionMode::Bytes).expect("length");
    assert_eq!(bytes, 176_400);
    let seconds = stream.length_seconds().expect("length in seconds");
    assert!((seconds - 1.0).abs() < 0.01, "got {seconds} seconds");

    // A decode channel with the FLOAT flag yields f32 samples via
    // `Channel::get_data_f32`, without needing an output device.
    let mut buffer = vec![0f32; 1024];
    let read = stream.get_data_f32(&mut buffer).expect("get_data");
    assert_eq!(read, 1024);
    assert!(buffer.iter().any(|&s| s != 0.0));
}
