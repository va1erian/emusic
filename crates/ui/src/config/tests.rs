//! Unit tests for config round-trips and the recovery policy from #8
//! (missing/unknown fields tolerated, bad file backed up).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use emusic_player::tracker::{
    Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings,
};
use emusic_player::{ExplicitQueueSnapshot, QueueSnapshot, RepeatMode as PlayerRepeatMode};

use crate::config::{Config, PlaybackSession, UiState, load, save};
use crate::mock::MockPlayer;
use crate::player_api::{PlaybackStatus, PlayerApi, RepeatMode};
use crate::state::{
    Accent, AppState, PanelVisibility, Rgb, Theme, View, VisualizerMode, WindowGeometry,
};

/// A single-track explicit queue snapshot, for session tests.
fn one_track_queue(path: &str) -> QueueSnapshot {
    QueueSnapshot::Explicit(ExplicitQueueSnapshot {
        items: vec![PathBuf::from(path)],
        order: vec![0],
        pos: Some(0),
        shuffle: false,
        repeat: PlayerRepeatMode::Off,
    })
}

/// A player with `path` loaded at `position`, playing when `play` is true.
fn player_with_session(path: &str, position: Duration, play: bool) -> MockPlayer {
    let mut player = MockPlayer::default();
    player.restore_queue(&one_track_queue(path), position, play);
    player
}

/// Unique scratch directory per test, so parallel tests never collide and
/// nothing is written to the real `%APPDATA%`.
fn scratch_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "emusic-config-test-{}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn config_file(dir: &Path) -> PathBuf {
    dir.join("config.toml")
}

/// A config with every field moved away from its default, so round-trip
/// tests catch fields silently reset to defaults.
fn non_default_config() -> Config {
    Config {
        volume: 0.35,
        repeat_mode: RepeatMode::One,
        shuffle: true,
        theme: Theme::Light,
        accent: Accent::Blue,
        panels: PanelVisibility {
            navigator: false,
            right_panel: true,
            status_bar: false,
        },
        column_browser_visible: false,
        column_browser_height: 222.0,
        last_view: View::MostPlayed,
        resume_playback: false,
        autoplay_on_restore: true,
        last_session: Some(PlaybackSession {
            queue: one_track_queue(r"C:\music\song.flac"),
            position_secs: 42.5,
        }),
        ui: UiState {
            window: WindowGeometry {
                size: Some([1000.0, 700.0]),
                position: Some([10.0, 20.0]),
                maximized: true,
            },
            search_query: "ambient".to_string(),
            music_selection: vec![1, 2, 3],
            ..UiState::default()
        },
        visualizer_enabled: true,
        visualizer: VisualizerMode::Oscilloscope,
        library_folders: vec![PathBuf::from(r"C:\music"), PathBuf::from(r"Z:\music")],
        tracker_settings: TrackerSettings {
            interpolation: Interpolation::Sinc,
            ramping: Ramping::Sensitive,
            stereo_separation: 42,
            amplify: 77,
            surround: Surround::Mode2,
            emulation: Emulation::Pt1,
            ft2_pan: true,
            end: EndBehavior::LoopTimes(3),
            resampling_quality: 4,
        },
        midi_soundfont: Some(PathBuf::from(r"C:onts\gm.sf2")),
        recent_soundfonts: vec![
            PathBuf::from(r"C:onts\gm.sf2"),
            PathBuf::from(r"C:onts\other.sf2"),
        ],
        songlengths_path: Some(PathBuf::from(r"C:\hvsc\DOCUMENTS\Songlengths.md5")),
        sid_fallback_secs: 240,
    }
}

#[test]
fn round_trip_preserves_all_fields() {
    let dir = scratch_dir("round-trip");
    let path = config_file(&dir);
    let config = non_default_config();

    save(&path, &config).expect("save config");
    assert_eq!(load(&path), config);

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn apply_to_state_restores_column_browser() {
    let config = Config {
        column_browser_visible: false,
        column_browser_height: 222.0,
        ..Config::default()
    };
    let mut state = AppState::default();
    config.apply_to_state(&mut state);

    assert!(!state.music.browser.visible);
    assert_eq!(state.music.browser.height, 222.0);
}

#[test]
fn missing_file_yields_defaults() {
    let dir = scratch_dir("missing");
    assert_eq!(load(&config_file(&dir)), Config::default());

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn missing_fields_fall_back_to_defaults() {
    let dir = scratch_dir("missing-fields");
    let path = config_file(&dir);
    fs::write(&path, "volume = 0.5\nshuffle = true\n").expect("write partial config");

    let config = load(&path);
    assert_eq!(config.volume, 0.5);
    assert!(config.shuffle);
    // Everything absent keeps its default.
    assert_eq!(config.theme, Theme::default());
    assert_eq!(config.accent, Accent::default());
    assert_eq!(config.panels, PanelVisibility::default());
    assert_eq!(
        config.column_browser_visible,
        Config::default().column_browser_visible
    );
    assert_eq!(
        config.column_browser_height,
        Config::default().column_browser_height
    );
    assert_eq!(config.last_view, View::default());
    // Resuming is on by default, autoplay is off, and an older config has no
    // session to restore.
    assert!(config.resume_playback);
    assert!(!config.autoplay_on_restore);
    assert_eq!(config.last_session, None);
    assert_eq!(config.ui, UiState::default());
    // The visualizer is opt-in, so an older config without the field keeps it
    // off (and thus keeps the app from repainting continuously while playing).
    assert!(!config.visualizer_enabled);

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn unknown_fields_are_ignored() {
    let dir = scratch_dir("unknown-fields");
    let path = config_file(&dir);
    fs::write(&path, "volume = 0.5\nfuture_option = \"x\"\n").expect("write config");

    let config = load(&path);
    assert_eq!(config.volume, 0.5);
    let defaults = Config::default();
    assert_eq!(config.repeat_mode, defaults.repeat_mode);
    assert_eq!(config.theme, defaults.theme);
    assert_eq!(config.accent, defaults.accent);
    assert_eq!(config.panels, defaults.panels);
    assert_eq!(
        config.column_browser_visible,
        defaults.column_browser_visible
    );
    assert_eq!(config.column_browser_height, defaults.column_browser_height);
    assert_eq!(config.last_view, defaults.last_view);

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn custom_accent_round_trips_as_hex() {
    let dir = scratch_dir("accent-hex");
    let path = config_file(&dir);
    let config = Config {
        accent: Accent::Custom(Rgb::from_rgb(0xCA, 0xFE, 0xBA)),
        ..Config::default()
    };

    save(&path, &config).expect("save config");
    assert_eq!(load(&path), config);

    // Custom accents land in the file as an editable `#rrggbb` string.
    let text = fs::read_to_string(&path).expect("read config");
    assert!(text.contains("accent = \"#cafeba\""), "toml was: {text}");

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn preset_accent_stored_by_name() {
    let dir = scratch_dir("accent-preset");
    let path = config_file(&dir);
    let config = Config {
        accent: Accent::Teal,
        ..Config::default()
    };

    save(&path, &config).expect("save config");
    let text = fs::read_to_string(&path).expect("read config");
    assert!(text.contains("accent = \"teal\""), "toml was: {text}");
    assert_eq!(load(&path), config);

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn unparsable_file_falls_back_and_is_backed_up() {
    let dir = scratch_dir("bad-file");
    let path = config_file(&dir);
    fs::write(&path, "volume = not a number").expect("write bad config");

    assert_eq!(load(&path), Config::default());
    assert!(!path.exists(), "bad file should have been renamed");
    assert!(
        path.with_extension("toml.bak").exists(),
        "bad file should be kept as config.toml.bak"
    );

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn save_creates_missing_directories() {
    let dir = scratch_dir("nested");
    let path = dir.join("emusic").join("config.toml");

    save(&path, &Config::default()).expect("save config");
    assert_eq!(load(&path), Config::default());

    fs::remove_dir_all(&dir).expect("clean up scratch dir");
}

#[test]
fn capture_leaves_the_volatile_session_out_of_the_settings_snapshot() {
    let player = player_with_session(r"C:\music\song.flac", Duration::from_secs(12), true);

    let config = Config::capture(&AppState::default(), &player);

    // The session position changes every frame, so it must not take part in
    // the per-frame dirty check.
    assert_eq!(config.last_session, None);
    assert!(config.resume_playback);
}

#[test]
fn session_capture_reads_the_whole_queue() {
    let player = player_with_session(r"C:\music\song.flac", Duration::from_secs(7), false);

    let session = PlaybackSession::capture(&player).expect("a loaded track is a session");

    assert_eq!(session.position(), Duration::from_secs(7));
    assert!(!session.queue.is_empty());
    // Nothing loaded and an empty queue means nothing to resume.
    assert_eq!(PlaybackSession::capture(&MockPlayer::default()), None);
}

#[test]
fn apply_to_player_restores_the_saved_session_and_autoplays() {
    let config = Config {
        autoplay_on_restore: true,
        last_session: Some(PlaybackSession {
            queue: one_track_queue(r"C:\music\song.flac"),
            position_secs: 12.0,
        }),
        ..Config::default()
    };
    let mut player = MockPlayer::default();

    config.apply_to_player(&mut player);

    assert_eq!(player.status(), PlaybackStatus::Playing);
    assert_eq!(player.position(), Duration::from_secs(12));
    assert_eq!(
        player.now_playing().map(|np| np.path.as_str()),
        Some(r"C:\music\song.flac")
    );
}

#[test]
fn apply_to_player_restores_the_saved_session_paused_without_autoplay() {
    let config = Config {
        autoplay_on_restore: false,
        last_session: Some(PlaybackSession {
            queue: one_track_queue(r"C:\music\song.flac"),
            position_secs: 12.0,
        }),
        ..Config::default()
    };
    let mut player = MockPlayer::default();

    config.apply_to_player(&mut player);

    assert_eq!(player.status(), PlaybackStatus::Paused);
    assert_eq!(player.position(), Duration::from_secs(12));
}

#[test]
fn apply_to_player_skips_the_session_when_resuming_is_off() {
    let config = Config {
        resume_playback: false,
        last_session: Some(PlaybackSession {
            queue: one_track_queue(r"C:\music\song.flac"),
            position_secs: 12.0,
        }),
        ..Config::default()
    };
    let mut player = MockPlayer::default();

    config.apply_to_player(&mut player);

    assert!(player.now_playing().is_none());
}
