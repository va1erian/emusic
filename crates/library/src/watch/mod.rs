//! File-system watching for library folders.
//!
//! A [`Watcher`] maintains `notify` watchers on local library roots and a
//! periodic polling schedule for remote roots (UNC or mapped network
//! drives). Changed paths are debounced and emitted as
//! [`WatchEvent::ScanRequested`] events over a caller-provided channel.
//!
//! Callers (typically the app) receive these events and run the incremental
//! scanner on the requested paths.
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use std::sync::mpsc;
//! # use std::time::Duration;
//! # use emusic_library::watch::{Watcher, WatchOptions, WatchEvent};
//! # fn run() -> Result<(), emusic_library::watch::WatchError> {
//! let (tx, rx) = mpsc::channel();
//! let watcher = Watcher::new(WatchOptions::default(), tx)?;
//! watcher.set_roots(vec![PathBuf::from(r"C:\music")])?;
//! // Later, on the receiver thread:
//! // let event = rx.recv()?;
//! # watcher.shutdown()?;
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

mod debounce;
mod remote;
mod worker;

pub use debounce::Debounce;
pub use remote::is_remote_root;
pub use worker::Watcher;

/// Errors returned by the folder watcher.
#[derive(Debug, Error)]
pub enum WatchError {
    /// The underlying `notify` watcher failed to start or update.
    #[error("notify watcher error: {0}")]
    Notify(#[from] notify::Error),

    /// The watcher background thread has shut down.
    #[error("watcher thread disconnected")]
    ThreadGone,

    /// The watcher background thread panicked.
    #[error("watcher thread panicked")]
    ThreadPanic,
}

/// A live event from the folder watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// One or more paths changed and should be rescanned.
    ScanRequested { paths: Vec<PathBuf> },
}

/// Tuning knobs for the watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    /// How long to wait after the last notify event before emitting a scan
    /// request. Default: 2 seconds.
    pub debounce: Duration,

    /// How often to rescan remote roots. Default: 30 minutes.
    pub remote_poll_interval: Duration,

    /// How often the background thread wakes to check channels and timers.
    /// Default: 100 milliseconds.
    pub tick_interval: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_secs(2),
            remote_poll_interval: Duration::from_secs(30 * 60),
            tick_interval: Duration::from_millis(100),
        }
    }
}

#[cfg(test)]
mod tests;
