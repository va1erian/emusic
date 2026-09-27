//! Real-file SID integration: verifies that the cRSID engine renders audible
//! PCM and that switching subtunes actually changes the output, and that the
//! BASS-backed [`SidChannel`] reports and plays a SID tune's subtunes.
//!
//! Nothing is audible: the engine tests render PCM into memory, and the
//! channel test initializes BASS on its "no sound" device (`Bass::init(0, ..)`).
//!
//! The fixture tunes are bundled under `tests/fixtures/sid/` (see
//! [docs/test-fixtures.md] for credits), so these tests run in CI without any
//! BASS install; only the channel test needs `bass.dll` and skips (prints why)
//! when it is absent. Either file can still be overridden with
//! `EMUSIC_SID_TEST_FILE` / `EMUSIC_SID_TEST_MULTI`.
//!
//! `Winners.sid` is a single-subtune tune, so it only covers playback; the
//! switching assertions use a multi-subtune tune.
//!
//! [docs/test-fixtures.md]: ../../../docs/test-fixtures.md

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bass::Bass;
use emusic_player::sid::{CrsidDecoder, SidDecoder};
use emusic_player::{AudioBackend, BassBackend};

/// Sample rate the engine renders at, matching [`emusic_player`]'s SID path.
const SAMPLE_RATE: u32 = 44_100;
/// Samples rendered per probe (half a second, enough for the envelope to open).
const WINDOW: usize = SAMPLE_RATE as usize / 2;
/// Peak amplitude above which a render is considered audible.
const AUDIBLE: i32 = 100;

/// The bundled SID fixtures (credits in [docs/test-fixtures.md]).
///
/// [docs/test-fixtures.md]: ../../../docs/test-fixtures.md
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sid");
/// A single-subtune tune (`songs = 1`): plain playback.
const SINGLE_TUNE: &str = "Winners.sid";
/// A multi-subtune tune (`songs >= 2`): subtune switching.
const MULTI_TUNE: &str = "Mini_Melodies_Compilation.sid";

/// Resolves a fixture, preferring an `env_key` override and otherwise the
/// bundled copy under `tests/fixtures/sid/`. `None` (after printing why) when
/// the file is missing, so the test can skip.
fn fixture(env_key: &str, name: &str) -> Option<PathBuf> {
    let path = std::env::var_os(env_key)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(FIXTURES).join(name));
    if path.is_file() {
        return Some(path);
    }
    eprintln!(
        "skipping: SID fixture {name} not found at {}",
        path.display()
    );
    None
}

/// Loads a fixture with the default SID config.
fn load(path: &Path) -> CrsidDecoder {
    let data = std::fs::read(path).expect("read SID fixture");
    CrsidDecoder::from_bytes(data, SAMPLE_RATE, Default::default()).expect("load SID tune")
}

/// Renders one [`WINDOW`] of PCM, restarting the current subtune first so every
/// probe starts from a clean engine state.
fn render(decoder: &mut CrsidDecoder) -> Vec<i16> {
    let mut pcm = vec![0i16; WINDOW];
    let written = decoder.render(&mut pcm).expect("render PCM");
    assert_eq!(
        written,
        pcm.len(),
        "the engine should fill the whole buffer"
    );
    pcm
}

/// The highest absolute sample in `pcm`.
fn peak(pcm: &[i16]) -> i32 {
    pcm.iter()
        .map(|s| s.unsigned_abs() as i32)
        .max()
        .unwrap_or(0)
}

#[test]
fn engine_renders_audible_pcm_for_a_real_tune() {
    let Some(path) = fixture("EMUSIC_SID_TEST_FILE", SINGLE_TUNE) else {
        return;
    };
    let mut decoder = load(&path);
    assert_eq!(
        decoder.subtune_count(),
        1,
        "Winners.sid is a single-subtune tune"
    );

    let pcm = render(&mut decoder);
    assert!(
        peak(&pcm) > AUDIBLE,
        "expected audible output from {}, peak was {}",
        path.display(),
        peak(&pcm)
    );
}

#[test]
fn switching_subtunes_changes_the_rendered_audio() {
    let Some(path) = fixture("EMUSIC_SID_TEST_MULTI", MULTI_TUNE) else {
        return;
    };
    let mut decoder = load(&path);
    let count = decoder.subtune_count();
    assert!(
        count >= 2,
        "{} should have multiple subtunes, got {count}",
        path.display()
    );

    // Render every subtune from a clean start; switching must re-init the tune.
    let renders: Vec<Vec<i16>> = (1..=count)
        .map(|subtune| {
            decoder.select_subtune(subtune).expect("select subtune");
            render(&mut decoder)
        })
        .collect();

    assert!(
        renders.iter().any(|pcm| peak(pcm) > AUDIBLE),
        "at least one subtune should be audible"
    );
    assert!(
        renders.iter().any(|pcm| pcm != &renders[0]),
        "at least two subtunes should render differently"
    );
}

/// Initializes BASS on device `0`, the "no sound" device, so the channel test
/// takes the real mixing path without any audible output. Returns `None` when
/// `bass.dll` is absent.
fn init_silent() -> Option<Bass> {
    match Bass::init(0, SAMPLE_RATE) {
        Ok(bass) => Some(bass),
        Err(bass::BassError::DllNotFound(detail)) => {
            eprintln!("skipping: bass.dll not available ({detail})");
            None
        }
        Err(other) => panic!("unexpected error initializing BASS: {other}"),
    }
}

/// Opens `path` through the real backend and returns the channel plus its
/// reported subtune count.
fn open_channel(backend: &BassBackend, path: &Path) -> Box<dyn emusic_player::BackendChannel> {
    backend.open(path).expect("open SID through BassBackend")
}

#[test]
fn channel_reports_and_plays_subtunes() {
    // `BASS_Init`/`BASS_Free` is process-global; this is the only test in the
    // binary that initializes BASS, so no serialization is needed.
    let Some(bass) = init_silent() else {
        return;
    };
    let backend = BassBackend::new(Arc::new(bass));

    // Playback: a single-subtune tune still reports its one subtune and plays.
    if let Some(path) = fixture("EMUSIC_SID_TEST_FILE", SINGLE_TUNE) {
        let channel = open_channel(&backend, &path);
        let state = channel.subsong().expect("a SID channel reports subsongs");
        assert_eq!(
            state.count,
            1,
            "{} is a single-subtune tune",
            path.display()
        );
        assert_eq!(state.current, 1);

        channel.play(false).expect("start SID playback");
        std::thread::sleep(Duration::from_millis(250));
        assert!(
            channel.position().expect("position").as_secs_f64() > 0.0,
            "the silent BASS device should still advance the push stream"
        );
        channel.stop().expect("stop SID playback");
    }

    // Switching: the multi-subtune tune reports its count, and selecting a
    // subtune updates the current one and keeps playing.
    if let Some(path) = fixture("EMUSIC_SID_TEST_MULTI", MULTI_TUNE) {
        let channel = open_channel(&backend, &path);
        let state = channel.subsong().expect("a SID channel reports subsongs");
        assert!(
            state.count >= 2,
            "{} should have multiple subtunes",
            path.display()
        );

        channel.play(false).expect("start SID playback");
        std::thread::sleep(Duration::from_millis(100));
        channel.select_subsong(2).expect("switch subtune");
        assert_eq!(
            channel.subsong().expect("subsong state").current,
            2,
            "selecting subtune 2 should make it current"
        );

        std::thread::sleep(Duration::from_millis(250));
        assert!(
            channel.position().expect("position").as_secs_f64() > 0.0,
            "playback should continue after a subtune switch"
        );
        channel.stop().expect("stop SID playback");
    }
}
