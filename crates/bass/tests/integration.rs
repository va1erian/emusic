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

mod signal;
mod silent;
mod temp;
mod wav;

use bass::{Bass, BassError, Channel, PositionMode, PushFlags, StreamFlags};

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

#[test]
fn push_decode_stream_decodes_pushed_samples() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let stream = bass
        .open_push_stream(44_100, 1, PushFlags::DECODE | PushFlags::FLOAT)
        .expect("creating a push stream should succeed");

    let bytes = signal::f32_bytes(&signal::f32_sine(44_100.0, 440.0, 0.1));
    let queued = stream.push_data(&bytes).expect("push_data");
    // A decoding stream has no playback buffer, so everything is queued.
    assert_eq!(queued, bytes.len() as u32);
    assert_eq!(stream.queued_bytes().expect("queued"), queued);

    let mut buffer = vec![0f32; 256];
    let read = stream.get_data_f32(&mut buffer).expect("get_data");
    assert_eq!(read, 256);
    assert!(buffer.iter().any(|&s| s != 0.0));
}

#[test]
fn fft_reports_one_bin_per_positive_frequency() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let path = temp::write_file("bass_fft_test", "wav", &wav::mono_file_1s().0);

    let stream = bass
        .open_stream(&path, StreamFlags::DECODE | StreamFlags::FLOAT)
        .expect("decoding a valid WAV file should succeed");

    // Feed the FFT some decoded data first, then read the magnitudes.
    let mut samples = vec![0f32; 1024];
    stream.get_data_f32(&mut samples).expect("get_data");

    let bins = stream
        .get_data_fft(bass::FftSize::Fft1024)
        .expect("BASS_ChannelGetData should return FFT data");
    assert_eq!(bins.len(), bass::FftSize::Fft1024.output_len());
    assert!(bins.iter().all(|b| b.is_finite()));
}

#[test]
fn push_limit_round_trips() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let stream = bass
        .open_push_stream(44_100, 1, PushFlags::FLOAT)
        .expect("create push stream");

    stream.set_push_limit(4096).expect("set push limit");
    assert_eq!(stream.push_limit().expect("push limit"), 4096);
    stream.set_push_limit(0).expect("clear push limit");
    assert_eq!(stream.push_limit().expect("push limit"), 0);
}

#[test]
fn push_data_rejects_partial_frames() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let stream = bass
        .open_push_stream(44_100, 2, PushFlags::FLOAT)
        .expect("create push stream");
    assert_eq!(stream.frame_bytes(), 8);
    // 4 bytes is half a stereo f32 sample frame.
    assert!(matches!(
        stream.push_data(&[0; 4]),
        Err(BassError::IllParam)
    ));
}

#[test]
fn push_stream_duration_comes_from_the_owner() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let stream = bass
        .open_push_stream(44_100, 1, PushFlags::FLOAT)
        .expect("create push stream");

    // The length is unknown until the owner supplies it.
    assert!(matches!(stream.length_seconds(), Err(BassError::NotAvail)));

    stream.set_duration(2.0);
    assert_eq!(stream.duration(), Some(2.0));
    assert_eq!(stream.length_seconds().expect("length"), 2.0);
    assert_eq!(
        stream.length(PositionMode::Bytes).expect("length bytes"),
        2 * 44_100 * 4
    );
}

#[test]
fn push_playback_stream_advances_and_ends() {
    let Some((_guard, bass)) = init_silent() else {
        return;
    };
    let stream = bass
        .open_push_stream(44_100, 1, PushFlags::FLOAT)
        .expect("create push stream");

    let bytes = signal::f32_bytes(&signal::f32_sine(44_100.0, 440.0, 0.5));
    stream.push_data(&bytes).expect("push_data");
    stream.play(false).expect("play");

    std::thread::sleep(std::time::Duration::from_millis(150));
    let position = stream.position(PositionMode::Bytes).expect("position");
    assert!(position > 0, "position did not advance: {position}");

    stream.end_of_stream().expect("end_of_stream");
}
