#![forbid(unsafe_code)]

//! SID rendering through the in-repo [`emusic_sid`] (cRSID) engine.

use std::path::Path;
use std::time::Duration;

use emusic_sid::{SidConfig, SidPlayer, SongLengths, resolve_database_path};

use crate::error::RenderError;
use crate::pcm::PcmBuffer;
use crate::renderer::{RenderOptions, Renderer, has_extension};

/// Extensions this renderer accepts.
pub const SID_EXTENSIONS: &[&str] = &["sid", "psid", "rsid"];

/// Bump when a change alters SID output, to invalidate cached renditions.
pub const SID_RENDERER_VERSION: u32 = 1;

/// Upper bound for a tune whose length is unknown, so a non-terminating play
/// routine still finishes. Ten minutes is far longer than a typical SID tune.
pub const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(600);

/// Samples rendered per engine call. Keeps single allocations small while the
/// per-tune buffer still grows to the full length.
const RENDER_CHUNK: usize = 4_096;

/// Renders SID tunes to mono signed 16-bit PCM.
///
/// When an HVSC [`SongLengths`] database is configured, a tune's per-subtune
/// length decides how much to render. Otherwise the tune is clamped to a
/// bounded default (or [`RenderOptions::max_duration`]).
pub struct SidRenderer {
    sample_rate: u32,
    song_lengths: Option<SongLengths>,
    default_max_duration: Duration,
}

impl SidRenderer {
    /// A renderer at `sample_rate` Hz without song-length data.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            song_lengths: None,
            default_max_duration: DEFAULT_MAX_DURATION,
        }
    }

    /// Attaches already-loaded song lengths.
    pub fn with_song_lengths(mut self, database: SongLengths) -> Self {
        self.song_lengths = Some(database);
        self
    }

    /// Loads a song-lengths database from a file or HVSC root and attaches it.
    pub fn with_song_lengths_database(mut self, path: &Path) -> Result<Self, RenderError> {
        let resolved = resolve_database_path(path).unwrap_or_else(|| path.to_path_buf());
        self.song_lengths = Some(SongLengths::load(&resolved)?);
        Ok(self)
    }

    /// Overrides the bounded length used when no song length is known.
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.default_max_duration = duration;
        self
    }

    /// The rendering sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The duration to render for `subtune`, in priority order: the configured
    /// song length, an explicit option, then the bounded default.
    fn duration_for(&self, data: &[u8], subtune: u16, options: &RenderOptions) -> Duration {
        self.song_lengths
            .as_ref()
            .and_then(|database| database.subtune(data, subtune))
            .or(options.max_duration)
            .unwrap_or(self.default_max_duration)
    }
}

impl Renderer for SidRenderer {
    fn family(&self) -> &'static str {
        "sid"
    }

    fn version(&self) -> u32 {
        SID_RENDERER_VERSION
    }

    fn supports(&self, path: &Path) -> bool {
        has_extension(path, SID_EXTENSIONS)
    }

    fn render(&self, path: &Path, options: &RenderOptions) -> Result<PcmBuffer, RenderError> {
        let data = std::fs::read(path).map_err(|error| RenderError::io(path, error))?;
        let mut player =
            SidPlayer::from_bytes(data.clone(), self.sample_rate, SidConfig::default())?;
        let requested = options.subtune.unwrap_or(player.default_subtune());
        player.select_subtune(requested)?;

        let duration = self.duration_for(&data, player.current_subtune(), options);
        let samples = render_to_end(&mut player, self.sample_rate, duration);
        PcmBuffer::mono(self.sample_rate, samples)
    }
}

/// Renders exactly `duration` worth of samples, chunk by chunk.
fn render_to_end(player: &mut SidPlayer, sample_rate: u32, duration: Duration) -> Vec<i16> {
    let target = samples_for(duration, sample_rate);
    let mut samples = Vec::with_capacity(target);
    let mut chunk = vec![0i16; RENDER_CHUNK];
    while samples.len() < target {
        let count = (target - samples.len()).min(RENDER_CHUNK);
        player.render(&mut chunk[..count]);
        samples.extend_from_slice(&chunk[..count]);
    }
    samples
}

/// Converts a duration to a whole number of mono samples.
fn samples_for(duration: Duration, sample_rate: u32) -> usize {
    let seconds = duration.as_secs_f64();
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * f64::from(sample_rate)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Environment variable holding a real `.sid` file to render.
    const TUNE_ENV: &str = "EMUSIC_SID_TUNE";

    /// Renders a real SID file when `EMUSIC_SID_TUNE` is set.
    ///
    /// No `.sid` fixture is committed to the repository (and this test refuses
    /// to hand-craft a 6502 program): the `emusic-sid` crate already exercises
    /// the engine with an in-test synthetic tune. Point `EMUSIC_SID_TUNE` at
    /// any real `.sid` to run this end-to-end; otherwise it skips.
    #[test]
    fn renders_a_real_tune_when_configured() {
        let Ok(path) = std::env::var(TUNE_ENV) else {
            return;
        };
        let path = Path::new(&path);
        let renderer = SidRenderer::new(44_100).with_max_duration(Duration::from_millis(500));
        assert!(renderer.supports(path));
        let pcm = renderer
            .render(path, &RenderOptions::default())
            .expect("render real tune");
        assert_eq!(pcm.channels(), 1);
        assert_eq!(pcm.sample_rate(), 44_100);
        assert!(!pcm.is_empty());
        assert!(pcm.peak() > 0, "expected non-silent PCM from {path:?}");
    }

    #[test]
    fn reports_whether_it_supports_a_path() {
        let renderer = SidRenderer::new(44_100);
        assert!(renderer.supports(Path::new("tune.sid")));
        assert!(renderer.supports(Path::new("tune.psid")));
        assert!(renderer.supports(Path::new("tune.RSID")));
        assert!(!renderer.supports(Path::new("tune.mod")));
    }

    #[test]
    fn unknown_length_falls_back_to_the_bounded_default() {
        let renderer = SidRenderer::new(8_000);
        assert_eq!(
            renderer.duration_for(b"not a tune", 1, &RenderOptions::default()),
            DEFAULT_MAX_DURATION
        );
        let options = RenderOptions {
            subtune: None,
            max_duration: Some(Duration::from_secs(2)),
        };
        assert_eq!(
            renderer.duration_for(b"x", 1, &options),
            Duration::from_secs(2)
        );
    }

    #[test]
    fn converts_duration_to_whole_samples() {
        assert_eq!(samples_for(Duration::from_secs(1), 44_100), 44_100);
        assert_eq!(samples_for(Duration::from_millis(500), 8_000), 4_000);
        assert_eq!(samples_for(Duration::ZERO, 44_100), 0);
    }
}
