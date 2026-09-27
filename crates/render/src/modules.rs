#![forbid(unsafe_code)]

//! Tracker-module rendering through the pure-Rust [`xmrsplayer`].
//!
//! Handles Amiga MOD, FastTracker XM, ScreamTracker S3M and Impulse Tracker IT
//! by importing them into the `xmrs` module model and driving the sample/OPL/SID
//! synthesis engine. It is 100% safe Rust with no C toolchain, so it runs on
//! the server and on Android (the NDK would reject the cRSID SID engine).

use std::path::Path;

use xmrs::prelude::Module;
use xmrsplayer::prelude::XmrsPlayer;

use crate::error::RenderError;
use crate::pcm::PcmBuffer;
use crate::renderer::{RenderOptions, Renderer, has_extension};

/// Extensions this renderer accepts.
///
/// `xmrsplayer` supports MOD, XM, S3M, IT and David Whittaker `.dw`; MO3 and
/// MTM/UMX are not covered (MO3 has no pure-Rust decoder).
pub const MODULE_EXTENSIONS: &[&str] = &["mod", "xm", "s3m", "it"];

/// Bump when a change alters module output, to invalidate cached renditions.
pub const MODULE_RENDERER_VERSION: u32 = 1;

/// Safety cap so a pathological module cannot render forever or allocate an
/// unbounded sample buffer. A normal tune stops after one loop via
/// [`XmrsPlayer::set_max_loop_count`]; this only bounds the pathological case.
const MAX_RENDER_SECONDS: u64 = 10 * 60;

/// Renders tracker modules to interleaved stereo PCM.
pub struct ModuleRenderer {
    sample_rate: u32,
}

impl ModuleRenderer {
    /// A renderer at `sample_rate` Hz.
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    /// The rendering sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl Renderer for ModuleRenderer {
    fn family(&self) -> &'static str {
        "module"
    }

    fn version(&self) -> u32 {
        MODULE_RENDERER_VERSION
    }

    fn supports(&self, path: &Path) -> bool {
        has_extension(path, MODULE_EXTENSIONS)
    }

    fn render(&self, path: &Path, options: &RenderOptions) -> Result<PcmBuffer, RenderError> {
        let bytes = std::fs::read(path).map_err(|error| RenderError::io(path, error))?;
        let module =
            Module::load(&bytes).map_err(|error| RenderError::Module(format!("{error:?}")))?;

        // Subtune is exposed as a 1-based sub-song; the engine indexes from 0.
        let song = options
            .subtune
            .map(|subtune| usize::from(subtune.saturating_sub(1)))
            .unwrap_or(0);
        let mut player = XmrsPlayer::new(&module, self.sample_rate, song);
        player.set_max_loop_count(1);

        let rate = u64::from(self.sample_rate);
        let cap_seconds = options
            .max_duration
            .map(|duration| duration.as_secs())
            .unwrap_or(MAX_RENDER_SECONDS)
            .min(MAX_RENDER_SECONDS);
        let capacity = (cap_seconds * rate) as usize * 2;

        let mut samples = Vec::new();
        for sample in player.by_ref() {
            samples.push(sample);
            if samples.len() >= capacity {
                break;
            }
        }
        PcmBuffer::new(self.sample_rate, 2, samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_only_the_tracker_extensions() {
        let renderer = ModuleRenderer::new(48_000);
        assert!(renderer.supports(Path::new("song.mod")));
        assert!(renderer.supports(Path::new("song.IT")));
        assert!(renderer.supports(Path::new("song.s3m")));
        assert!(renderer.supports(Path::new("song.xm")));
        assert!(!renderer.supports(Path::new("tune.sid")));
        assert!(!renderer.supports(Path::new("song.mp3")));
    }

    #[test]
    fn rejects_a_non_module() {
        let renderer = ModuleRenderer::new(48_000);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-module.mod");
        std::fs::write(&path, b"this is not a tracker module").unwrap();
        assert!(renderer.render(&path, &RenderOptions::default()).is_err());
    }
}
