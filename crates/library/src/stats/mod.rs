//! Play/skip recording and the read-side stats queries built on top of it.
//!
//! [`StatsRecorder`] takes [`PlayRecord`]s off the UI/player thread and
//! writes them to the store on a background thread, so a slow disk never
//! stalls playback. Everything else in `queries` is plain read access
//! against [`crate::Store`], meant for history and stats views.
//!
//! This module deliberately does not depend on `emusic-player`: `PlayRecord`
//! is a small, player-shaped input type defined here instead, so the app
//! crate (wiring #11) maps `emusic_player::PlayerEvent::PlayFinished` onto
//! it rather than this crate taking on a dependency on the player.

mod queries;
mod recorder;

pub use queries::{StatsWindow, history_page, most_played};
pub use recorder::{PlayRecord, StatsRecorder};
