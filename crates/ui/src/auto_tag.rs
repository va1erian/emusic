//! Offline types for online auto-tag lookups (#208).
//!
//! These are the shapes the shell, the library backend and the tag editor
//! exchange; the actual metadata database lives in `emusic-metadata`, reached
//! through its [`Provider`](emusic_metadata::Provider) trait. Nothing here does
//! I/O, so the types are cheap to clone and safe to move across threads.

use std::path::PathBuf;

pub use emusic_metadata::{Candidate, TrackQuery};

/// A request to look up online metadata for one track.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoTagRequest {
    /// The file the lookup is for; outcomes carry it back.
    pub path: PathBuf,
    /// The local metadata the lookup is seeded from.
    pub query: TrackQuery,
}

/// Why an auto-tag lookup did not produce candidates.
#[derive(Debug, thiserror::Error)]
pub enum AutoTagError {
    /// The lookup was cancelled before it produced a result.
    #[error("the lookup was cancelled")]
    Cancelled,
    /// The metadata provider failed (offline, rate-limited, parse error, ...).
    #[error(transparent)]
    Provider(#[from] emusic_metadata::MetadataError),
}

/// The result of one auto-tag lookup.
#[derive(Debug)]
pub struct AutoTagOutcome {
    /// The file the lookup targeted.
    pub path: PathBuf,
    /// The ranked candidates, or the error that stopped the lookup.
    pub result: Result<Vec<Candidate>, AutoTagError>,
}

/// An in-flight lookup's progress, shown by the status bar (#210).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoTagStatus {
    /// The file being looked up.
    pub path: PathBuf,
    /// A human-readable progress line.
    pub text: String,
    /// How many lookups have finished in this request.
    pub done: usize,
    /// How many lookups the request covers.
    pub total: usize,
}
