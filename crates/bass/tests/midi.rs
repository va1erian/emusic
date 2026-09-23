//! MIDI playback through the `bassmidi` add-on plugin.
//!
//! Skips (rather than fails) when `bass.dll` or `bassmidi.dll` is missing.
//! The soundfont is generated on the fly (see [`sf2`]), so no external
//! soundfont is needed to prove that BASSMIDI renders real audio.

mod sf2;
mod silent;

use std::path::Path;

use bass::{Bass, Channel, StreamFlags};

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

/// Loads the plugins from `EMUSIC_BASS_DIR`; false when `bassmidi.dll` isn't
/// among the ones that loaded.
fn load_midi_plugin(bass: &Bass) -> bool {
    let dir = std::env::var_os("EMUSIC_BASS_DIR").unwrap_or_default();
    bass.load_plugins(dir).iter().any(|plugin| {
        plugin.result.is_ok()
            && plugin
                .path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("bassmidi.dll"))
    })
}

/// The loudest sample in the first second of `midi`, decoded.
fn first_second_peak(bass: &Bass, midi: &Path) -> f32 {
    let stream = bass
        .open_stream(midi, StreamFlags::DECODE | StreamFlags::FLOAT)
        .expect("open midi decode stream");
    let mut buffer = vec![0f32; 44100 * 2];
    let read = stream.get_data_f32(&mut buffer).expect("decode samples");
    buffer[..read]
        .iter()
        .fold(0f32, |peak, s| peak.max(s.abs()))
}

// One test: the default soundfont is process-global BASS state, so the
// silent-without / audible-with comparison must run in a fixed order.
#[test]
fn midi_is_silent_without_a_soundfont_and_audible_with_one() {
    let Some((_guard, bass)) = silent::init_silent() else {
        return;
    };
    if !load_midi_plugin(&bass) {
        eprintln!("skipping: bassmidi.dll not available");
        return;
    }
    let dir = std::env::temp_dir().join(format!("emusic-midi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let midi = dir.join("two_seconds.mid");
    std::fs::write(&midi, two_second_midi()).expect("write midi");
    let font = dir.join("sine.sf2");
    std::fs::write(&font, sf2::sine_soundfont()).expect("write soundfont");

    // Without any soundfont BASSMIDI still opens the file (so scanning gets
    // a length) but renders silence.
    assert_eq!(first_second_peak(&bass, &midi), 0.0);

    // A stream opened before the soundfont is set picks it up live.
    let early = bass
        .open_stream(&midi, StreamFlags::DECODE | StreamFlags::FLOAT)
        .expect("open early stream");
    bass.config()
        .set_midi_default_font(&font)
        .expect("set default soundfont");
    let mut buffer = vec![0f32; 44100 * 2];
    let read = early.get_data_f32(&mut buffer).expect("decode early");
    assert!(buffer[..read].iter().any(|s| s.abs() > 0.01));
    drop(early);
    assert!(first_second_peak(&bass, &midi) > 0.01);

    let stream = bass
        .open_stream(&midi, StreamFlags::FLOAT | StreamFlags::PRESCAN)
        .expect("open midi stream");
    let length = stream.length_seconds().expect("length");
    assert!((length - 2.0).abs() < 0.1, "unexpected length {length}");
    stream.play(true).expect("play");
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(stream.is_active());
    let position = stream.position_seconds().expect("position");
    assert!(position > 0.2, "position did not advance: {position}");
    stream.seek(1.0).expect("seek");
    assert!(stream.position_seconds().expect("position") >= 0.9);

    let _ = std::fs::remove_dir_all(&dir);
}
