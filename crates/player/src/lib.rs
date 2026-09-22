//! Playback engine: queue, transport, repeat/shuffle and listen accounting,
//! built on top of [`bass`] (see `crates/bass`).
//!
//! # Layout
//! - [`Player`] — the crate's main type: owns the queue and the currently
//!   loaded backend channel, exposes transport methods (`play_pause`,
//!   `next`, `seek`, ...) and emits [`PlayerEvent`]s.
//! - [`queue::Queue`] — track list, shuffle order and repeat mode. Pure
//!   logic, no BASS involved, so it's unit-tested directly.
//! - [`backend::AudioBackend`] / [`backend::BackendChannel`] — the seam
//!   between [`Player`] and BASS. [`backend::BassBackend`] is the real
//!   implementation; tests substitute a mock so `Player`'s queue/transport
//!   logic is exercised without any audio device.
//! - [`events::PlayerEvent`] — what a UI (or the stats module, #23) reads
//!   back to know what happened.
//!
//! # Wiring into the app (#11)
//! This crate deliberately doesn't depend on `emusic` (the app crate) or
//! implement its `PlayerApi` trait — that's out of scope here. But the two
//! are shaped to make that adapter close to mechanical:
//! - [`events::PlaybackState`] mirrors `PlayerApi`'s `PlaybackStatus`.
//! - [`queue::RepeatMode`] has the same three variants (`Off`/`All`/`One`)
//!   as the app's own `RepeatMode`.
//! - Every `PlayerApi` getter/command (`status`, `position`, `duration`,
//!   `volume`, `repeat_mode`, `shuffle`, `play_pause`, `stop`, `next`,
//!   `previous`, `seek`, `set_volume`, `set_repeat_mode`, `set_shuffle`) has
//!   a same-named, same-signature-shape method on [`Player`]. An adapter
//!   only needs to call `Player::tick` from `PlayerApi::tick`, map
//!   `queue_paths()` to `QueueEntry`s (using whatever metadata lookup the
//!   app has), and drain `Player::events()` to react to track changes.

#![forbid(unsafe_code)]

pub mod backend;
pub mod error;
pub mod events;
mod listen;
mod player;
pub mod queue;
pub mod tracker;
pub mod volume;

pub use backend::{AudioBackend, BackendChannel, BassBackend};
pub use error::PlayerError;
pub use events::{PlaybackState, PlayerEvent};
pub use player::Player;
pub use queue::{Queue, QueueSource, RepeatMode, ShuffleSource};
