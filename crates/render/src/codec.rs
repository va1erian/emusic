#![forbid(unsafe_code)]

//! Output codecs a [`crate::Renderer`]'s PCM can be encoded to.

/// A container/codec for a rendered rendition.
///
/// Only FLAC exists today. Future codecs (for example Opus) extend this enum
/// without changing the cache-key format, because [`Codec::as_str`] is stable
/// and is what the key is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Codec {
    /// Free Lossless Audio Codec.
    Flac,
}

impl Codec {
    /// The stable token used in cache keys and content negotiation.
    pub fn as_str(self) -> &'static str {
        match self {
            Codec::Flac => "flac",
        }
    }

    /// The file extension for a cached rendition.
    pub fn extension(self) -> &'static str {
        self.as_str()
    }
}

impl std::fmt::Display for Codec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
