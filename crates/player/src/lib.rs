//! Playback engine: queue, transport, repeat/shuffle and listen accounting,
//! built on top of [`bass`] (see `crates/bass`).

#![forbid(unsafe_code)]

pub mod backend;
pub mod cache;
pub mod error;
pub mod events;
mod listen;
pub mod midi;
mod player;
pub mod queue;
pub mod sid;
pub mod tracker;
pub mod volume;

pub use backend::{AudioBackend, BackendChannel, BassBackend};
pub use cache::StreamCacheManager;
pub use error::PlayerError;
pub use events::{PlaybackState, PlayerEvent};
pub use player::Player;
pub use queue::{
    ExplicitQueueSnapshot, Queue, QueueSnapshot, QueueSource, RepeatMode, ShuffleSnapshot,
    ShuffleSource,
};
pub use sid::{SidChannel, SidDecoder};
