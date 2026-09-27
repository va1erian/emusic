//! Server-side rendering of specialized files and the rendition cache.
//!
//! Android has no native SID/module/MIDI engines, so `emusic-server` renders
//! those formats to FLAC and serves the result. This module owns the runtime
//! [`RenderService`] (renderer, cache and concurrency bound) and the
//! [`RenderCapabilities`] advertised to clients. The HTTP handler lives in
//! [`crate::api::render_routes`]; the heavy lifting is in `emusic-render`.
//!
//! Only the SID renderer exists today. Module and MIDI renderers are planned
//! follow-ups; until they land, those formats are neither `renderable` nor
//! served by `/render` (which returns `404`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use emusic_render::{Codec, Renderer, RenditionCache, RenditionKey, SidRenderer};
use tokio::sync::Semaphore;

use crate::config::{RENDER_CODEC_FLAC, RenderConfig};
use crate::error::{Result, ServerError};
use crate::scanner::formats;

/// A running renderer: its output codec, rendition cache and concurrency bound.
pub struct RenderService {
    codec: Codec,
    cache: RenditionCache,
    sid: SidRenderer,
    permits: Arc<Semaphore>,
    renditions: Vec<String>,
}

impl RenderService {
    /// Builds the service from `config`, or `None` when rendering is disabled.
    ///
    /// `data_dir` anchors the cache at `<data_dir>/renditions`. A configured
    /// SID song-lengths database (`songlengths`) is loaded best-effort: a
    /// missing or malformed file only costs exact tune lengths, never a crash.
    pub fn new(
        config: &RenderConfig,
        data_dir: &Path,
        songlengths: Option<&Path>,
    ) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        let codec = parse_codec(&config.codec)?;
        let cache = RenditionCache::new(data_dir.join("renditions"), config.cache_max_bytes);
        let sid = build_sid_renderer(config.sample_rate, songlengths);
        Ok(Some(Self {
            codec,
            cache,
            sid,
            permits: Arc::new(Semaphore::new(config.max_concurrent as usize)),
            renditions: vec![codec.as_str().to_string()],
        }))
    }

    /// The output codec renditions are encoded to.
    pub fn codec(&self) -> Codec {
        self.codec
    }

    /// The rendition cache.
    pub fn cache(&self) -> &RenditionCache {
        &self.cache
    }

    /// The SID renderer.
    pub fn sid(&self) -> &SidRenderer {
        &self.sid
    }

    /// The concurrency bound shared by every render request.
    pub fn permits(&self) -> &Arc<Semaphore> {
        &self.permits
    }

    /// The capability set this service advertises.
    pub fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            renditions: self.renditions.clone(),
        }
    }
}

/// Which specialized formats the server can render, and to which codecs.
///
/// This is computed from the runtime [`RenderService`], not from a track, so
/// disabling `render` makes every track report `renderable = false`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderCapabilities {
    renditions: Vec<String>,
}

impl RenderCapabilities {
    /// No renderer is enabled.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Whether any renderer is available.
    pub fn is_enabled(&self) -> bool {
        !self.renditions.is_empty()
    }

    /// The output codecs advertised to clients.
    pub fn renditions(&self) -> &[String] {
        &self.renditions
    }

    /// Whether the stored format label `format` can be rendered today.
    ///
    /// Only SID has a renderer; module and MIDI renderers are follow-ups.
    pub fn is_renderable(&self, format: &str) -> bool {
        self.is_enabled() && formats::is_sid(format)
    }
}

/// Parses the configured codec, rejecting anything but FLAC.
fn parse_codec(value: &str) -> Result<Codec> {
    if value.eq_ignore_ascii_case(RENDER_CODEC_FLAC) {
        Ok(Codec::Flac)
    } else {
        Err(ServerError::Config(format!(
            "render.codec must be {RENDER_CODEC_FLAC:?}"
        )))
    }
}

/// Builds a SID renderer, attaching song lengths when they load.
fn build_sid_renderer(sample_rate: u32, songlengths: Option<&Path>) -> SidRenderer {
    let Some(path) = songlengths else {
        return SidRenderer::new(sample_rate);
    };
    match SidRenderer::new(sample_rate).with_song_lengths_database(path) {
        Ok(renderer) => {
            tracing::info!(path = %path.display(), "loaded SID song lengths for rendering");
            renderer
        }
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                "SID song lengths unavailable for rendering; using bounded lengths"
            );
            SidRenderer::new(sample_rate)
        }
    }
}

/// Builds a rendition key for `track_id` at `subtune` (0 meaning the renderer's
/// default subtune).
pub fn rendition_key<'a>(
    service: &'a RenderService,
    track_id: &'a str,
    subtune: u16,
) -> RenditionKey<'a> {
    RenditionKey::new(
        track_id,
        subtune,
        service.codec(),
        service.sid.family(),
        service.sid.version(),
    )
}

/// The cache path for a rendition, for tests and diagnostics.
pub fn rendition_path(
    service: &RenderService,
    track_id: &str,
    subtune: u16,
) -> std::result::Result<PathBuf, emusic_render::RenderError> {
    let key = rendition_key(service, track_id, subtune);
    service.cache.path_for(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_capabilities_render_nothing() {
        let caps = RenderCapabilities::disabled();
        assert!(!caps.is_enabled());
        assert!(!caps.is_renderable("sid"));
        assert!(caps.renditions().is_empty());
    }

    #[test]
    fn enabled_capabilities_render_only_sid() {
        let caps = RenderCapabilities {
            renditions: vec!["flac".to_string()],
        };
        assert!(caps.is_renderable("sid"));
        assert!(caps.is_renderable("psid"));
        assert!(!caps.is_renderable("xm"));
        assert!(!caps.is_renderable("mid"));
        assert!(!caps.is_renderable("flac"));
        assert_eq!(caps.renditions(), ["flac".to_string()]);
    }

    #[test]
    fn codec_parsing_rejects_anything_but_flac() {
        assert_eq!(parse_codec("flac").unwrap(), Codec::Flac);
        assert_eq!(parse_codec("FLAC").unwrap(), Codec::Flac);
        assert!(parse_codec("opus").is_err());
    }

    #[test]
    fn disabled_config_yields_no_service() {
        let config = RenderConfig::default();
        let service = RenderService::new(&config, Path::new("/tmp"), None).unwrap();
        assert!(service.is_none());
    }
}
