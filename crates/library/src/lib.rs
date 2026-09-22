//! SQLite-backed library store: tracks, folders, play history and stats.

#![forbid(unsafe_code)]

mod error;
mod store;

pub use error::{LibraryError, Result};
pub use store::{Folder, Store, TrackStats, default_db_path};
