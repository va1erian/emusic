#![forbid(unsafe_code)]

//! Errors returned while rendering or caching.

use std::path::PathBuf;

use thiserror::Error;

/// Something went wrong rendering or caching a specialized file.
#[derive(Debug, Error)]
pub enum RenderError {
    /// A file could not be read or written.
    #[error("i/o error on {path}: {source}")]
    Io {
        /// The file that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// No renderer handles this file's format.
    #[error("no renderer supports {0}")]
    Unsupported(String),
    /// The SID engine rejected or could not render the tune.
    #[error("SID rendering failed: {0}")]
    Sid(#[from] emusic_sid::SidError),
    /// The HVSC Songlengths database could not be read.
    #[error("songlengths database error: {0}")]
    SongLengths(#[from] emusic_sid::SongLengthsError),
    /// A [`crate::PcmBuffer`] was constructed with inconsistent shape.
    #[error("invalid PCM buffer: {0}")]
    InvalidPcm(String),
    /// The FLAC encoder failed.
    #[error("FLAC encoding failed: {0}")]
    Flac(String),
    /// The rendition cache could not satisfy the request safely.
    #[error("rendition cache error: {0}")]
    Cache(String),
}

impl RenderError {
    /// Convenience for wrapping an I/O error with the failing path.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
