//! SQLite-backed library store: tracks, folders, play history and stats.

#![forbid(unsafe_code)]

mod error;
pub mod index;
pub mod remote;
pub mod scanner;
pub mod stats;
mod store;
pub mod tags;
pub mod watch;

pub use error::{LibraryError, Result};
pub use store::{Folder, MostPlayedEntry, PlayHistoryEntry, Store, TrackStats, default_db_path};

// Re-export domain types used in the public store/scanner/index API so
// consumers don't have to depend on emusic-core separately for common types.
pub use emusic_core::{ArtSource, Track, TrackId, TrackKind};
