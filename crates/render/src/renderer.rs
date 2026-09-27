#![forbid(unsafe_code)]

//! The [`Renderer`] abstraction: one implementation per format family.

use std::path::Path;
use std::time::Duration;

use crate::error::RenderError;
use crate::pcm::PcmBuffer;

/// Options for a single render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderOptions {
    /// 1-based subtune to render.
    ///
    /// `None` selects the file's default subtune. Renderers that have no
    /// subtunes ignore it.
    pub subtune: Option<u16>,
    /// A cap on the rendered length, overriding the renderer's bounded default.
    ///
    /// A known song length (for example an HVSC `Songlengths` entry) still
    /// wins; this only bounds tunes whose end is unknown.
    pub max_duration: Option<Duration>,
}

/// Renders one format family to PCM.
///
/// The trait takes a path (not bytes) so a renderer can stream large files and
/// read side data (soundfonts, songlengths) itself. [`Renderer::family`] and
/// [`Renderer::version`] together identify the rendering algorithm in a cache
/// key; bump [`Renderer::version`] whenever a change alters the output.
pub trait Renderer {
    /// Stable family name, for example `"sid"`.
    fn family(&self) -> &'static str;

    /// Output-changing implementation version.
    fn version(&self) -> u32;

    /// Whether this renderer handles `path`. Implementations match on the
    /// file extension and do not touch the filesystem.
    fn supports(&self, path: &Path) -> bool;

    /// Renders `path` to PCM, honouring `options`.
    fn render(&self, path: &Path, options: &RenderOptions) -> Result<PcmBuffer, RenderError>;
}

/// Whether `path` has one of `extensions`, compared ASCII case-insensitively.
pub(crate) fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert!(has_extension(Path::new("tune.sid"), &["sid", "psid"]));
        assert!(has_extension(Path::new("TUNE.SID"), &["sid"]));
        assert!(!has_extension(Path::new("tune.flac"), &["sid"]));
        assert!(!has_extension(Path::new("sid"), &["sid"]));
    }
}
