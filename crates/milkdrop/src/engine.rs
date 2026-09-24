#![forbid(unsafe_code)]

/// One rendered frame's worth of visual state. Deliberately small and
/// toolkit-agnostic: cheap enough to paint with a plain 2D painter (as
/// [`crate::PlaceholderEngine`] does) while still being roughly what a
/// GPU-backed engine would need to expose (a colour, an intensity, a phase)
/// if a future implementation exposed a texture instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Hue, cycling 0.0..1.0.
    pub hue: f32,
    /// Current loudness, eased and clamped to 0.0..=1.0.
    pub pulse: f32,
    /// Cumulative rotation phase, in radians, unbounded.
    pub swirl: f32,
}

/// A frame-driven visualization engine: fed PCM audio, advanced by `dt`,
/// and read back as a [`Frame`] each tick.
///
/// This is the seam a real MilkDrop/projectM binding would implement
/// alongside the [`crate::PlaceholderEngine`] shipped in this crate (see
/// the crate docs).
pub trait MilkdropEngine {
    /// Feed one block of interleaved PCM samples.
    fn feed_pcm(&mut self, samples: &[f32]);

    /// Advance the engine by `dt` seconds and return the frame to paint.
    fn tick(&mut self, dt: f32) -> Frame;
}
