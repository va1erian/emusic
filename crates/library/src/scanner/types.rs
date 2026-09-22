//! Public configuration and result types for the scanner.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Tuning knobs and external resources for one scan run.
#[derive(Clone)]
pub struct ScanOptions {
    /// Number of threads reading tags. Tag reading is I/O-bound —
    /// especially over SMB shares — so this is a small bounded pool
    /// (sensible range 8–16) rather than the machine's CPU count.
    pub tag_threads: usize,
    /// How many scanned tracks are committed per database transaction.
    pub batch_size: usize,
    /// An initialised BASS instance used to read tracker module tags.
    /// When `None`, module files are counted but skipped (see
    /// [`ScanSummary::modules_skipped`]).
    pub bass: Option<Arc<bass::Bass>>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            tag_threads: 8,
            batch_size: 500,
            bass: None,
        }
    }
}

/// The outcome of a completed (or cancelled) scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    /// Supported audio files seen during the walk.
    pub files_found: u64,
    /// Rows inserted for files new to the library.
    pub tracks_added: u64,
    /// Rows of changed files updated in place.
    pub tracks_updated: u64,
    /// Rows re-pointed to a new path after move/rename detection.
    pub tracks_moved: u64,
    /// Rows deleted for files that vanished under fully-scanned roots.
    pub tracks_deleted: u64,
    /// Files that could not be parsed (logged and skipped).
    pub files_skipped: u64,
    /// Module files skipped because no BASS instance was provided.
    pub modules_skipped: u64,
    /// Roots that could not be accessed at all.
    pub unreachable_roots: Vec<PathBuf>,
    /// Whether the scan stopped early after
    /// [`CancelToken::cancel`] was requested.
    pub cancelled: bool,
    /// Whether the scan could not see the whole library: unreachable
    /// roots, unreadable directories, or cancellation. Nothing was deleted
    /// that might still exist.
    pub partial: bool,
    /// Total wall-clock time of the scan.
    pub elapsed: Duration,
}
