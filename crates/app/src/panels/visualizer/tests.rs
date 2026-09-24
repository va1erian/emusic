//! Unit tests for the visualizer's mode cycling and repaint gating.

use super::*;

#[test]
fn mode_cycles_through_every_variant_and_wraps() {
    assert_eq!(
        VisualizerMode::Spectrum.next(),
        VisualizerMode::Oscilloscope
    );
    assert_eq!(
        VisualizerMode::Oscilloscope.next(),
        VisualizerMode::Milkdrop
    );
    assert_eq!(VisualizerMode::Milkdrop.next(), VisualizerMode::Off);
    assert_eq!(VisualizerMode::Off.next(), VisualizerMode::Spectrum);
}

#[test]
fn mode_slugs_round_trip() {
    for mode in VisualizerMode::ALL {
        assert_eq!(VisualizerMode::from_slug(mode.slug()), Some(mode));
    }
    assert_eq!(VisualizerMode::from_slug("nope"), None);
}

#[test]
fn wishes_repaint_only_while_playing_and_not_off() {
    assert!(VisualizerState::wishes_repaint(
        VisualizerMode::Spectrum,
        true
    ));
    assert!(VisualizerState::wishes_repaint(
        VisualizerMode::Oscilloscope,
        true
    ));
    // Off never animates, even while playing.
    assert!(!VisualizerState::wishes_repaint(VisualizerMode::Off, true));
    // Paused/stopped never animates, whatever the mode.
    assert!(!VisualizerState::wishes_repaint(
        VisualizerMode::Spectrum,
        false
    ));
    assert!(!VisualizerState::wishes_repaint(
        VisualizerMode::Oscilloscope,
        false
    ));
}
