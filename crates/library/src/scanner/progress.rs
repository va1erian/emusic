//! Progress reporting and cancellation for the scanner.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A live progress update from a running scan.
///
/// Delivered over the channel passed to
/// [`scan`](super::scan); send failures are ignored, so dropping the
/// receiver simply disables progress reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    /// The running count of supported audio files discovered while walking
    /// the library roots.
    FilesFound { count: u64 },

    /// A candidate file was handled (its tags were read, or it was skipped
    /// as unreadable): `processed` of `total` files are done, and `path` is
    /// the file just handled.
    FileProcessed {
        processed: u64,
        total: u64,
        path: PathBuf,
    },

    /// A library root could not be accessed (offline share, removed drive,
    /// ...). Its rows are left untouched and the scan is marked partial.
    RootUnreachable { root: PathBuf },
}

/// Cooperative cancellation flag shared between a scan and its caller.
///
/// Create one with [`CancelToken::default`], pass a reference to
/// [`scan`](super::scan) (usually from a background thread) and call
/// [`CancelToken::cancel`] from any thread to stop the scan at its next
/// checkpoint. A cancelled scan stops early, commits nothing destructive
/// (no row deletions) and reports [`ScanSummary::cancelled`].
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// Requests cancellation; the scan stops at its next checkpoint.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_uncancelled_and_can_be_cancelled() {
        let token = CancelToken::default();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn clones_share_the_same_flag() {
        let token = CancelToken::default();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
    }
}
