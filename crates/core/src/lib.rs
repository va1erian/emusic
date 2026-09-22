//! Domain types shared across the emusic workspace.
//!
//! This crate is intentionally dependency-light: it is used by the app,
//! the library store, the player and a mock-data generator, so it must
//! not pull in heavy or platform-specific dependencies.

#![forbid(unsafe_code)]

mod events;
mod ids;
mod track;

pub use events::PlayEvent;
pub use ids::TrackId;
pub use track::{ArtSource, Track, TrackKind};
