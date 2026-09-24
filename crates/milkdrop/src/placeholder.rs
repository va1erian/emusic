#![forbid(unsafe_code)]

use crate::engine::{Frame, MilkdropEngine};

/// Hue cycling speed at rest, in cycles/second; scales up with loudness.
const HUE_SPEED: f32 = 0.05;
/// Swirl rotation speed at rest, in radians/second; scales up with loudness.
const SWIRL_SPEED: f32 = 0.6;
/// Per-second ease rate the loudness pulse chases the latest RMS at, so it
/// reads as a smooth pulse rather than jittering every block.
const PULSE_EASE_PER_SECOND: f32 = 8.0;

/// Stand-in engine: no MilkDrop presets, just a smooth audio-reactive
/// plasma (hue/pulse/swirl) computed from PCM loudness. Exercises the same
/// [`MilkdropEngine`] seam a real projectM binding would implement.
#[derive(Debug, Default)]
pub struct PlaceholderEngine {
    rms: f32,
    pulse: f32,
    hue_phase: f32,
    swirl_phase: f32,
}

impl PlaceholderEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MilkdropEngine for PlaceholderEngine {
    fn feed_pcm(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        self.rms = (sum_sq / samples.len() as f32).sqrt();
    }

    fn tick(&mut self, dt: f32) -> Frame {
        let ease = (PULSE_EASE_PER_SECOND * dt).min(1.0);
        self.pulse += (self.rms - self.pulse) * ease;
        let drive = 1.0 + self.pulse * 2.0;
        self.hue_phase = (self.hue_phase + HUE_SPEED * dt * drive).fract();
        self.swirl_phase += SWIRL_SPEED * dt * drive;

        Frame {
            hue: self.hue_phase,
            pulse: self.pulse.clamp(0.0, 1.0),
            swirl: self.swirl_phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_decays_pulse_to_zero() {
        let mut engine = PlaceholderEngine::new();
        engine.feed_pcm(&[1.0, -1.0, 1.0, -1.0]);
        engine.tick(0.1);
        engine.feed_pcm(&[0.0; 4]);
        for _ in 0..1000 {
            engine.tick(0.1);
        }
        assert!(engine.tick(0.1).pulse < 0.01);
    }

    #[test]
    fn loud_signal_raises_pulse() {
        let mut engine = PlaceholderEngine::new();
        engine.feed_pcm(&[1.0, -1.0, 1.0, -1.0]);
        let frame = engine.tick(1.0);
        assert!(frame.pulse > 0.5);
    }

    #[test]
    fn hue_and_swirl_advance_over_time() {
        let mut engine = PlaceholderEngine::new();
        let first = engine.tick(1.0);
        let second = engine.tick(1.0);
        assert!(second.swirl > first.swirl);
        assert_ne!(second.hue, first.hue);
    }

    #[test]
    fn empty_pcm_block_is_ignored() {
        let mut engine = PlaceholderEngine::new();
        engine.feed_pcm(&[1.0, -1.0]);
        let before = engine.tick(0.0);
        engine.feed_pcm(&[]);
        let after = engine.tick(0.0);
        assert_eq!(before.pulse, after.pulse);
    }
}
