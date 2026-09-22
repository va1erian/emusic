//! The library scanner: incremental, parallel tag reading.
//!
//! [`scan`] walks the given library roots with `walkdir`, comparing each
//! file's (size, mtime) against the store; only new or changed files have
//! their tags read — streamed audio via lofty, tracker modules via a
//! caller-provided BASS instance — on a bounded thread pool. Results are
//! streamed in batches to a writer thread that owns the store, detects
//! moved/renamed files (same size, duration and tags at a new path) and
//! re-points their rows instead of delete + insert, preserving play
//! history. Files that vanished under roots that were walked completely are
//! deleted; rows under unreachable roots (offline shares) are never
//! touched.
//!
//! Paths are compared through case-insensitive, separator-normalised keys
//! (see the `paths` submodule), never `std::fs::canonicalize`, so mapped
//! drive letters and UNC shares behave consistently.
//!
//! ```no_run
//! # use std::sync::mpsc;
//! # use std::path::PathBuf;
//! # use emusic_library::scanner::{scan, CancelToken, ScanOptions};
//! # use emusic_library::Store;
//! # fn main() -> Result<(), emusic_library::LibraryError> {
//! let mut store = Store::open_default()?;
//! let roots = vec![PathBuf::from(r"Z:\music")];
//! let (progress_tx, _progress_rx) = mpsc::channel();
//! let cancel = CancelToken::default();
//! let summary = scan(&mut store, &roots, &ScanOptions::default(), &progress_tx, &cancel)?;
//! # let _ = summary;
//! # Ok(())
//! # }
//! ```

mod module_tags;
mod paths;
mod progress;
mod tags;
mod walk;
mod writer;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use tracing::warn;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use crate::error::Result;
use crate::store::Store;

pub use progress::{CancelToken, ScanEvent};

use walk::FoundFile;
use writer::{ScannedTrack, WriterMessage, WriterStats};

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

/// A candidate file whose tags must be (re)read, plus the stored row it
/// replaces, if any.
struct PendingRead {
    file: FoundFile,
    /// The stored path of the row whose normalised key matches this file;
    /// `None` for files new to the library.
    prior_path: Option<PathBuf>,
}

/// What happened to one candidate file during tag reading.
enum ScanOutcome {
    Stored(Box<ScannedTrack>),
    SkippedUnreadable,
    SkippedNoBass,
}

/// Runs one incremental scan of `roots`, blocking until it completes or is
/// cancelled.
///
/// `roots` are the folders to walk (callers usually pass the enabled
/// folders from [`Store::list_folders`]); paths are accepted as mapped
/// drives or UNC shares. `progress` receives [`ScanEvent`]s for the walk
/// and tag-reading phases — a dropped receiver is fine, it only disables
/// reporting. `cancel` stops the scan at the next checkpoint; a cancelled
/// scan commits the batches it already produced but deletes nothing.
///
/// Only files whose (size, mtime) differ from the stored row are re-read;
/// unchanged files cost a directory stat. Rows are only deleted for files
/// that vanished under roots that were walked completely and successfully;
/// unreachable roots and unreadable subdirectories never lose their rows.
///
/// # Errors
///
/// Returns a [`LibraryError`](crate::LibraryError) if the database writer
/// fails; individual unreadable files are logged and skipped instead.
pub fn scan(
    store: &mut Store,
    roots: &[PathBuf],
    options: &ScanOptions,
    progress: &Sender<ScanEvent>,
    cancel: &CancelToken,
) -> Result<ScanSummary> {
    let started = Instant::now();
    let now = unix_now();

    let existing = store.size_mtime_map()?;
    let outcome = walk::walk_roots(roots, cancel, progress);

    let vanished = vanished_rows(&existing, &outcome, cancel);
    let pending = classify_files(outcome.files, &existing);

    let mut files_skipped = 0u64;
    let mut modules_skipped = 0u64;
    let stats = thread::scope(|scope| -> Result<WriterStats> {
        let (outbox, inbox) = mpsc::channel();
        let writer = scope.spawn(|| writer::run(store, inbox));

        if !cancel.is_cancelled() {
            if !vanished.is_empty() {
                let _ = outbox.send(WriterMessage::Vanished(vanished));
            }
            read_and_stream(
                &pending,
                options,
                now,
                progress,
                cancel,
                &outbox,
                &mut files_skipped,
                &mut modules_skipped,
            );
        }
        // Deletions only when the whole scan ran to completion: a cancelled
        // walk has an incomplete "seen" set, and cancelled reads may leave a
        // move partner unread.
        let _ = outbox.send(WriterMessage::Finish {
            delete_remaining: !cancel.is_cancelled(),
        });

        match writer.join() {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })?;

    Ok(ScanSummary {
        files_found: outcome.seen_keys.len() as u64,
        tracks_added: stats.added,
        tracks_updated: stats.updated,
        tracks_moved: stats.moved,
        tracks_deleted: stats.deleted,
        files_skipped,
        modules_skipped,
        unreachable_roots: outcome.unreachable_roots,
        cancelled: cancel.is_cancelled(),
        partial: cancel.is_cancelled() || outcome.partial,
        elapsed: started.elapsed(),
    })
}

/// Splits walked files into unchanged (dropped) and new/changed (to read),
/// tagging each candidate with the stored row it replaces, matched by
/// normalised path key.
fn classify_files(
    files: Vec<FoundFile>,
    existing: &HashMap<PathBuf, (u64, i64)>,
) -> Vec<PendingRead> {
    let mut prior_by_key: HashMap<String, (PathBuf, u64, i64)> =
        HashMap::with_capacity(existing.len());
    for (path, &(size, mtime)) in existing {
        prior_by_key
            .entry(paths::normalize_key(path))
            .or_insert((path.clone(), size, mtime));
    }

    let mut pending = Vec::new();
    for file in files {
        let key = paths::normalize_key(&file.path);
        match prior_by_key.get(&key) {
            Some((prior, size, mtime)) if *size == file.size && *mtime == file.mtime => {}
            Some((prior, ..)) => pending.push(PendingRead {
                file,
                prior_path: Some(prior.clone()),
            }),
            None => pending.push(PendingRead {
                file,
                prior_path: None,
            }),
        }
    }
    pending
}

/// Stored rows whose files were not seen on disk — but only those safe to
/// act on: under a root that was walked completely, and not under a
/// directory that failed to list. Empty when the walk was cancelled (the
/// "seen" set would be incomplete).
fn vanished_rows(
    existing: &HashMap<PathBuf, (u64, i64)>,
    outcome: &walk::WalkOutcome,
    cancel: &CancelToken,
) -> Vec<PathBuf> {
    if cancel.is_cancelled() {
        return Vec::new();
    }
    let scanned_root_keys: Vec<String> = outcome
        .scanned_roots
        .iter()
        .map(|root| paths::normalize_key(root))
        .collect();
    let failed_keys: Vec<String> = outcome
        .failed_subtrees
        .iter()
        .map(|dir| paths::normalize_key(dir))
        .collect();

    let mut vanished = Vec::new();
    for path in existing.keys() {
        let key = paths::normalize_key(path);
        if outcome.seen_keys.contains(&key) {
            continue;
        }
        if !scanned_root_keys
            .iter()
            .any(|root| paths::key_is_under(&key, root))
        {
            continue;
        }
        if failed_keys
            .iter()
            .any(|failed| paths::key_is_under(&key, failed))
        {
            continue;
        }
        vanished.push(path.clone());
    }
    vanished.sort();
    vanished
}

/// Reads tags for every pending file on a bounded pool, streaming batches to
/// the writer until done, cancelled, or the writer dies.
#[allow(clippy::too_many_arguments)]
fn read_and_stream(
    pending: &[PendingRead],
    options: &ScanOptions,
    now: i64,
    progress: &Sender<ScanEvent>,
    cancel: &CancelToken,
    outbox: &Sender<WriterMessage>,
    files_skipped: &mut u64,
    modules_skipped: &mut u64,
) {
    // The pool builder can only fail for zero threads or an unusable thread
    // name; both are excluded by clamping the thread count at one.
    let pool = ThreadPoolBuilder::new()
        .num_threads(options.tag_threads.max(1))
        .build()
        .expect("a rayon pool with at least one thread cannot fail to build");
    let batch_size = options.batch_size.max(1);
    let total = pending.len() as u64;
    let processed = AtomicU64::new(0);

    for chunk in pending.chunks(batch_size) {
        if cancel.is_cancelled() {
            break;
        }
        let outcomes: Vec<ScanOutcome> = pool.install(|| {
            chunk
                .par_iter()
                .map(|item| {
                    scan_file(
                        item,
                        options.bass.as_deref(),
                        now,
                        &processed,
                        total,
                        progress,
                    )
                })
                .collect()
        });
        let mut batch = Vec::with_capacity(outcomes.len());
        for outcome in outcomes {
            match outcome {
                ScanOutcome::Stored(scanned) => batch.push(*scanned),
                ScanOutcome::SkippedUnreadable => *files_skipped += 1,
                ScanOutcome::SkippedNoBass => *modules_skipped += 1,
            }
        }
        if outbox.send(WriterMessage::Tracks(batch)).is_err() {
            break;
        }
    }
}

/// Reads tags for one pending file and reports its progress event.
fn scan_file(
    item: &PendingRead,
    bass: Option<&bass::Bass>,
    now: i64,
    processed: &AtomicU64,
    total: u64,
    progress: &Sender<ScanEvent>,
) -> ScanOutcome {
    let outcome = match item.file.kind {
        TrackKind::Stream => match tags::read_stream(&item.file, now) {
            Ok(track) => ScanOutcome::Stored(Box::new(ScannedTrack {
                track,
                prior_path: item.prior_path.clone(),
            })),
            Err(error) => {
                warn!(path = %item.file.path.display(), %error, "skipping unreadable file");
                ScanOutcome::SkippedUnreadable
            }
        },
        TrackKind::Module => match bass {
            Some(bass) => match module_tags::read_module(bass, &item.file, now) {
                Ok(track) => ScanOutcome::Stored(Box::new(ScannedTrack {
                    track,
                    prior_path: item.prior_path.clone(),
                })),
                Err(error) => {
                    warn!(path = %item.file.path.display(), %error, "skipping unreadable module");
                    ScanOutcome::SkippedUnreadable
                }
            },
            None => ScanOutcome::SkippedNoBass,
        },
    };
    let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = progress.send(ScanEvent::FileProcessed {
        processed: done,
        total,
        path: item.file.path.clone(),
    });
    outcome
}

/// The file-facts skeleton of a track: identity fields filled in from the
/// walk, every tag field empty, awaiting a tag reader.
pub(super) fn untagged_track(file: &FoundFile, now: i64) -> Track {
    Track {
        id: TrackId::UNASSIGNED,
        path: file.path.clone(),
        dir: file
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        filename: file
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ext: paths::extension(&file.path).unwrap_or_default(),
        size: file.size,
        mtime: file.mtime,
        kind: file.kind,
        duration_ms: 0,
        bitrate: None,
        sample_rate: None,
        channels: None,
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: now,
    }
}

/// The current time as a Unix timestamp (seconds, UTC).
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
