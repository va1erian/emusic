//! Additional cross-cutting tests for the tracker settings module.

use super::settings::{Emulation, EndBehavior, Interpolation, Ramping, Surround, TrackerSettings};
use super::{TrackerConfig, TrackerFormat};
use std::collections::HashMap;
use std::path::Path;

#[test]
fn default_settings_match_bass_defaults() {
    let s = TrackerSettings::default();
    assert_eq!(s.interpolation, Interpolation::Linear);
    assert_eq!(s.ramping, Ramping::Off);
    assert_eq!(s.stereo_separation, 100);
    assert_eq!(s.amplify, 50);
    assert_eq!(s.surround, Surround::Off);
    assert_eq!(s.emulation, Emulation::Auto);
    assert!(!s.ft2_pan);
    assert_eq!(s.end, EndBehavior::StopAtEnd);
    assert_eq!(s.resampling_quality, 2);
}

#[test]
fn sanitize_clamps_numeric_fields() {
    let mut s = TrackerSettings {
        stereo_separation: 200,
        amplify: 255,
        resampling_quality: 9,
        ..TrackerSettings::default()
    };
    s.sanitize();
    assert_eq!(s.stereo_separation, 100);
    assert_eq!(s.amplify, 100);
    assert_eq!(s.resampling_quality, 4);
}

#[test]
fn presets_are_distinct() {
    assert_ne!(
        TrackerSettings::default(),
        TrackerSettings::amiga_authentic()
    );
    assert_ne!(TrackerSettings::default(), TrackerSettings::smooth());
    assert_ne!(
        TrackerSettings::amiga_authentic(),
        TrackerSettings::smooth()
    );
}

#[test]
fn config_serialises_round_trip() {
    let mut config = TrackerConfig::new(TrackerSettings::default());
    config
        .per_format
        .insert(TrackerFormat::It, TrackerSettings::smooth());
    config.per_file.insert(
        "/music/demo.xm".to_string(),
        TrackerSettings::amiga_authentic(),
    );

    let toml = toml::to_string(&config).expect("serialise");
    let back: TrackerConfig = toml::from_str(&toml).expect("deserialise");

    assert_eq!(back.global, config.global);
    assert_eq!(back.per_format, config.per_format);
    assert_eq!(back.per_file, config.per_file);
}

#[test]
fn unknown_extension_falls_back_to_global_even_with_format_overrides() {
    let mut config = TrackerConfig::new(TrackerSettings::amiga_authentic());
    config.per_format = HashMap::from([
        (TrackerFormat::Mod, TrackerSettings::smooth()),
        (TrackerFormat::Xm, TrackerSettings::smooth()),
    ]);

    assert_eq!(
        config.resolve(Path::new("unknown.xyz")),
        TrackerSettings::amiga_authentic()
    );
}
