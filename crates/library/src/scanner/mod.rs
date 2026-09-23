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

mod midi_tags;
mod module_tags;
mod moves;
pub(crate) mod paths;
mod progress;
mod tags;
mod types;
mod walk;
mod writer;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use tracing::warn;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};

use crate::error::Result;
use crate::store::Store;

pub use progress::{CancelToken, ScanEvent};
pub use types::{ScanOptions, ScanSummary};

use moves::{PendingRead, classify_files, vanished_rows};
use walk::FoundFile;
use writer::{ScannedTrack, WriterMessage, WriterStats};

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
        TrackKind::Stream if paths::is_midi(&item.file.path) => match bass {
            Some(bass) => match midi_tags::read_midi(bass, &item.file, now) {
                Ok(track) => ScanOutcome::Stored(Box::new(ScannedTrack {
                    track,
                    prior_path: item.prior_path.clone(),
                })),
                Err(error) => {
                    warn!(path = %item.file.path.display(), %error, "skipping unreadable MIDI file");
                    ScanOutcome::SkippedUnreadable
                }
            },
            None => ScanOutcome::SkippedNoBass,
        },
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
        starred: false,
    }
}

/// The current time as a Unix timestamp (seconds, UTC).
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
