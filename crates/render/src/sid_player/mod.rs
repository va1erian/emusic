#![forbid(unsafe_code)]

//! Pure-Rust SID playback for targets where the cRSID engine cannot build
//! (notably the Android NDK, which is clang-only).
//!
//! Emulates a MOS 6510 (via `mos6502`) with the SID trapped at `$D400`, driven
//! by the `sidera` chip model. Adapted from `sidera`'s `sidplay` example (MIT,
//! © Sébastien Béchet). Scope: PSID tunes (the init/play model); RSID tunes that
//! need real CIA/VIC timing are best-effort.

mod bus;
mod player;
mod psid;

use std::path::Path;
use std::time::Duration;

use sidera::ChipModel;

use crate::error::RenderError;
use crate::pcm::PcmBuffer;
use crate::renderer::{RenderOptions, Renderer, has_extension};

/// Extensions this renderer accepts.
pub const SID_EXTENSIONS: &[&str] = &["sid", "psid", "rsid"];

/// Bump when a change alters SID output, to invalidate cached renditions.
pub const SID_PLAYER_VERSION: u32 = 1;

/// Default render length when no song length is known.
const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(180);

/// Renders PSID/RSID tunes to mono PCM with the pure-Rust engine.
pub struct SidPlayerRenderer {
    sample_rate: u32,
}

impl SidPlayerRenderer {
    /// A renderer at `sample_rate` Hz.
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    /// The rendering sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl Renderer for SidPlayerRenderer {
    fn family(&self) -> &'static str {
        "sid"
    }

    fn version(&self) -> u32 {
        SID_PLAYER_VERSION
    }

    fn supports(&self, path: &Path) -> bool {
        has_extension(path, SID_EXTENSIONS)
    }

    fn render(&self, path: &Path, options: &RenderOptions) -> Result<PcmBuffer, RenderError> {
        let bytes = std::fs::read(path).map_err(|error| RenderError::io(path, error))?;
        let tune = psid::Psid::parse(&bytes).map_err(RenderError::SidPlayer)?;

        // The PSID flags pick the chip model and video standard; PAL/6581 is the
        // fallback when unspecified.
        let model = match tune.prefer_8580 {
            Some(true) => ChipModel::Mos8580,
            _ => ChipModel::Mos6581,
        };
        let refresh = match tune.prefer_ntsc {
            Some(true) => 60,
            _ => 50,
        };

        let song1 = options
            .subtune
            .filter(|subtune| *subtune > 0)
            .unwrap_or(tune.start_song.max(1));
        let song0 = u8::try_from(song1.saturating_sub(1)).unwrap_or(u8::MAX);
        let mut player = player::Player::new(&tune, song0, model, self.sample_rate, refresh);

        let duration = options.max_duration.unwrap_or(DEFAULT_MAX_DURATION);
        let target = (duration.as_secs_f64() * f64::from(self.sample_rate)).max(0.0) as usize;
        let mut samples = Vec::with_capacity(target);
        while samples.len() < target {
            let frame = player.render_frame();
            if frame.is_empty() {
                break;
            }
            samples.extend_from_slice(&frame);
        }
        samples.truncate(target);
        PcmBuffer::mono(self.sample_rate, samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_only_sid_extensions() {
        let renderer = SidPlayerRenderer::new(44_100);
        assert!(renderer.supports(Path::new("tune.sid")));
        assert!(renderer.supports(Path::new("tune.PSID")));
        assert!(renderer.supports(Path::new("tune.rsid")));
        assert!(!renderer.supports(Path::new("song.mod")));
        assert!(!renderer.supports(Path::new("song.mp3")));
    }

    #[test]
    fn rejects_a_non_sid() {
        let renderer = SidPlayerRenderer::new(44_100);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-tune.sid");
        std::fs::write(&path, b"definitely not a PSID header").unwrap();
        assert!(renderer.render(&path, &RenderOptions::default()).is_err());
    }

    /// A minimal PSID: `init` is an `RTS`; `play` sets master volume, a triangle
    /// waveform, ADSR and a frequency, then returns. Enough to prove the
    /// 6510 + SID pipeline produces audio.
    fn minimal_psid() -> Vec<u8> {
        let mut code = vec![0x60];
        code.extend_from_slice(&[
            0xA9, 0x0F, 0x8D, 0x18, 0xD4, // LDA #$0F / STA $D418 (volume)
            0xA9, 0x11, 0x8D, 0x04, 0xD4, // LDA #$11 / STA $D404 (triangle+gate)
            0xA9, 0x00, 0x8D, 0x05, 0xD4, // attack/decay
            0xA9, 0xF0, 0x8D, 0x06, 0xD4, // sustain/release
            0xA9, 0x10, 0x8D, 0x00, 0xD4, // frequency lo
            0xA9, 0x20, 0x8D, 0x01, 0xD4, // frequency hi
            0x60, // RTS
        ]);

        let mut bytes = vec![0u8; 0x7C];
        bytes[0..4].copy_from_slice(b"PSID");
        bytes[4..6].copy_from_slice(&2u16.to_be_bytes());
        bytes[6..8].copy_from_slice(&0x7Cu16.to_be_bytes());
        bytes[8..10].copy_from_slice(&0x1000u16.to_be_bytes());
        bytes[10..12].copy_from_slice(&0x1000u16.to_be_bytes());
        bytes[12..14].copy_from_slice(&0x1001u16.to_be_bytes());
        bytes[14..16].copy_from_slice(&1u16.to_be_bytes());
        bytes[16..18].copy_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&code);
        bytes
    }

    #[test]
    fn renders_a_tone_from_a_minimal_tune() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tone.sid");
        std::fs::write(&path, minimal_psid()).unwrap();

        let renderer = SidPlayerRenderer::new(44_100);
        let options = RenderOptions {
            subtune: None,
            max_duration: Some(Duration::from_secs(1)),
        };
        let pcm = renderer.render(&path, &options).unwrap();
        assert_eq!(pcm.channels(), 1);
        assert!(pcm.frames() >= 44_000, "got {} frames", pcm.frames());
        assert!(pcm.peak() > 0, "expected non-silent SID output");
    }
}
