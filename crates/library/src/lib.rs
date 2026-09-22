//! SQLite-backed library store: tracks, folders, play history and stats.

#![forbid(unsafe_code)]

mod error;
pub mod index;
pub mod scanner;
pub mod stats;
mod store;
pub mod watch;

pub use error::{LibraryError, Result};
pub use store::{Folder, MostPlayedEntry, PlayHistoryEntry, Store, TrackStats, default_db_path};
