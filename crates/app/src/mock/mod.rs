//! Deterministic mock data mode (#32): fake library + fake player, so the
//! shell (and `emusic-shot`) can run and be screenshotted with no BASS, no
//! database, and fully reproducible output.

mod data;
mod generators;
mod library;
mod player;

pub use data::generate;
pub use library::MockLibrary;
pub use player::MockPlayer;
