//! Unit tests for config round-trips and the recovery policy from #8
//! (missing/unknown fields tolerated, bad file backed up).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui::Color32;

use crate::config::{Config, load, save};
use crate::player_api::RepeatMode;
use crate::state::{Accent, AppState, PanelVisibility, Theme, View, VisualizerMode};

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
        visualizer: VisualizerMode::Oscilloscope,
        library_folders: vec![PathBuf::from(r"C:\music"), PathBuf::from(r"Z:\music")],
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

    assert!(!state.column_browser.visible);
    assert_eq!(state.column_browser.height, 222.0);
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
        accent: Accent::Custom(Color32::from_rgb(0xCA, 0xFE, 0xBA)),
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
