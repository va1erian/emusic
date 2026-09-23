//! Unit tests for the oscilloscope's sample decimation.

use super::*;

#[test]
fn short_input_is_passed_through_unchanged() {
    let samples = vec![0.1, -0.2, 0.3];
    assert_eq!(decimate(&samples, 256), samples);
}

#[test]
fn long_input_is_reduced_to_at_most_max_points() {
    let samples: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
    let reduced = decimate(&samples, 256);
    assert!(reduced.len() <= 256, "got {} points", reduced.len());
    assert!(!reduced.is_empty());
}

#[test]
fn decimation_preserves_the_average() {
    let samples = vec![1.0_f32; 1000];
    let reduced = decimate(&samples, 100);
    assert_eq!(reduced.len(), 100);
    for value in reduced {
        assert!((value - 1.0).abs() < 1e-6);
    }
}

#[test]
fn decimation_of_symmetric_wave_stays_centred() {
    // A sine spanning positive and negative should average to roughly zero
    // over full periods, so the decimated trace should not drift.
    let samples: Vec<f32> = (0..4096)
        .map(|i| (i as f32 / 4096.0 * std::f32::consts::TAU * 64.0).sin())
        .collect();
    let reduced = decimate(&samples, 128);
    let mean = reduced.iter().sum::<f32>() / reduced.len() as f32;
    assert!(mean.abs() < 0.05, "trace drifted to {mean}");
}
