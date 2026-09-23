//! Error type returned by fallible [`crate::Player`] operations.

use thiserror::Error;

/// Everything that can go wrong driving playback.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PlayerError {
    /// The underlying BASS call failed.
    #[error(transparent)]
    Bass(#[from] bass::BassError),
    /// The queue index passed to a queue operation is out of bounds.
    #[error("queue index {0} out of bounds")]
    IndexOutOfBounds(usize),
    /// There is no track currently loaded to act on.
    #[error("no track is currently loaded")]
    NoCurrentTrack,
    /// Couldn't spawn the worker thread used to open files off the UI
    /// thread (see the network-drive note on the tracking issue).
    #[error("failed to spawn player worker thread: {0}")]
    SpawnFailed(String),
    /// A track's file couldn't be read from disk.
    #[error("failed to read file: {0}")]
    ReadFailed(String),
    /// The SID engine failed to load or run a tune.
    #[error(transparent)]
    Sid(#[from] emusic_sid::SidError),
}
