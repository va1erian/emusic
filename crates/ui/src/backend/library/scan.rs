//! Background library scans: on-demand rescans, watch-triggered updates and
//! removal purges.
//!
//! Each scan runs on its own thread and opens its **own** [`Store`]
//! connection from the same database file (`Store::open_second`), so it never
//! holds the shared store mutex: UI reads and folder edits stay responsive
//! for the whole scan, even on a slow network drive (#69). Results are
//! written through that connection, then a fresh snapshot built via the
//! shared one and sent to the UI thread, forwarding coarse progress updates
//! for the status bar. All file I/O stays off the UI thread.
//!
//! A rescan that changed nothing skips the snapshot (see [`Refresh`]):
//! applying one rebuilds every library-derived view on the UI thread, a
//! visible stall on a large library for no visible change.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use emusic_library::scanner::{CancelToken, ScanEvent, ScanSummary, scan};
use emusic_library::{Folder, Store};
use tracing::{info, warn};

use super::source::Snapshot;
use super::{Update, Updates, private_store, scan_options};

/// How many files to process between status-bar progress updates, so a fast
/// local scan never floods the UI-thread channel.
const STATUS_EVERY: u64 = 50;

/// A running scan's cancellation token plus its id. The id lets the backend
/// ignore a stale scan's completion after a newer one has started.
#[derive(Clone)]
pub(crate) struct ScanHandle {
    pub cancel: CancelToken,
    pub id: u64,
}

/// When a finished scan sends the UI a fresh [`Snapshot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refresh {
    /// Always send one: the folder list changed, so the snapshot's folders
    /// (and per-folder counts) are stale even if no track row did.
    Always,
    /// Send one only when the scan or purge changed the store. For rescans
    /// whose UI already holds a snapshot of the same store (the startup scan
    /// after the initial load, on-demand and watch-triggered rescans).
    IfChanged,
}

impl Refresh {
    /// Whether a scan that purged `purged` tracks and finished with
    /// `summary` (`None` when no roots were scanned) needs a new snapshot.
    fn needed(self, purged: usize, summary: Option<&ScanSummary>) -> bool {
        self == Refresh::Always || purged > 0 || summary.is_some_and(store_changed)
    }
}

/// Whether a scan wrote to the store, or stopped early (cancelled) so its
/// counts may not tell the whole story.
fn store_changed(summary: &ScanSummary) -> bool {
    summary.tracks_added > 0
        || summary.tracks_updated > 0
        || summary.tracks_moved > 0
        || summary.tracks_deleted > 0
        || summary.cancelled
}

/// Spawns a scan of `roots` on a background thread, first purging tracks
/// under any `purge` prefixes (folders removed from the library).
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    roots: Vec<PathBuf>,
    updates: Updates,
    handle: ScanHandle,
    purge: Vec<PathBuf>,
    bass: Option<Arc<bass::Bass>>,
    only_root: Option<PathBuf>,
    refresh: Refresh,
) {
    std::thread::spawn(move || {
        if let Err(err) = run(
            &store, &folders, &roots, &updates, &handle, &purge, bass, only_root, refresh,
        ) {
            warn!(%err, "library scan failed");
        }
    });
}

/// Runs one scan to completion, reporting progress, a final snapshot (when
/// `refresh` calls for one) and the [`Update::ScanFinished`] that clears the
/// scanning state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Updates,
    handle: &ScanHandle,
    purge: &[PathBuf],
    bass: Option<Arc<bass::Bass>>,
    only_root: Option<PathBuf>,
    refresh: Refresh,
) -> anyhow::Result<()> {
    let result = run_inner(
        store, folders, roots, updates, handle, purge, bass, only_root, refresh,
    );
    if result.is_err() {
        let _ = updates.send(Update::Status(String::new()));
    }
    let _ = updates.send(Update::ScanFinished(handle.id));
    result
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Updates,
    handle: &ScanHandle,
    purge: &[PathBuf],
    bass: Option<Arc<bass::Bass>>,
    only_root: Option<PathBuf>,
    refresh: Refresh,
) -> anyhow::Result<()> {
    // Real deployments scan through a private connection, so the shared lock
    // stays free for the UI. In-memory stores (unit tests) cannot be
    // reopened; for those, fall back to the shared connection and hold its
    // lock for the run, matching the pre-#69 behaviour.
    let (purged, summary) = match private_store(store) {
        Some(mut scan_store) => {
            purge_and_scan(&mut scan_store, roots, updates, handle, purge, bass)?
        }
        None => {
            let mut shared = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            purge_and_scan(&mut shared, roots, updates, handle, purge, bass)?
        }
    };

    if !refresh.needed(purged, summary.as_ref()) {
        info!("library unchanged; keeping the current snapshot");
        let _ = updates.send(Update::Status(String::new()));
        return Ok(());
    }

    let snapshot = {
        let store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot::from_store(&store, folders, only_root.as_deref())?
    };
    let _ = updates.send(Update::Snapshot(Box::new(snapshot)));
    let _ = updates.send(Update::Status(String::new()));
    Ok(())
}

/// Purges `purge`, then scans `roots` (if any). Returns how many tracks were
/// purged and the scan's summary (`None` when there was nothing to scan).
fn purge_and_scan(
    store: &mut Store,
    roots: &[PathBuf],
    updates: &Updates,
    handle: &ScanHandle,
    purge: &[PathBuf],
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<(usize, Option<ScanSummary>)> {
    let purged = purge_removed_roots(store, purge)?;
    let summary = if roots.is_empty() {
        None
    } else {
        Some(scan_roots(store, roots, updates, &handle.cancel, bass)?)
    };
    Ok((purged, summary))
}

/// Scans `roots` on a worker thread while this thread forwards coarse
/// progress updates to the UI.
fn scan_roots(
    store: &mut Store,
    roots: &[PathBuf],
    updates: &Updates,
    cancel: &CancelToken,
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<ScanSummary> {
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();
    let roots = roots.to_vec();
    let cancel = cancel.clone();
    let scan_result = std::thread::scope(|scope| {
        // `move` so `progress_tx` is owned by the scan thread and dropped
        // when it finishes, which ends the `progress_rx` loop below.
        let scan_handle =
            scope.spawn(move || scan(store, &roots, &scan_options(bass), &progress_tx, &cancel));

        while let Ok(event) = progress_rx.recv() {
            if let ScanEvent::FileProcessed {
                processed,
                total,
                path,
            } = event
                && (processed % STATUS_EVERY == 0 || processed == total)
            {
                let _ = updates.send(Update::Status(format!(
                    "Scanning {} / {} - {}",
                    format_count(processed),
                    format_count(total),
                    file_name(&path)
                )));
            }
        }

        match scan_handle.join() {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    });

    match scan_result {
        Ok(summary) => {
            info!(
                added = summary.tracks_added,
                updated = summary.tracks_updated,
                moved = summary.tracks_moved,
                deleted = summary.tracks_deleted,
                cancelled = summary.cancelled,
                "library scan complete"
            );
            Ok(summary)
        }
        Err(err) => Err(err.into()),
    }
}

/// Deletes every stored track under a removed folder. Folder rows are
/// independent of tracks (see `Store::remove_folder`), so without this a
/// removed folder's music would linger in the library. Returns how many
/// tracks were deleted.
fn purge_removed_roots(store: &mut Store, roots: &[PathBuf]) -> anyhow::Result<usize> {
    if roots.is_empty() {
        return Ok(0);
    }
    let paths: Vec<PathBuf> = store
        .load_all_tracks()?
        .into_iter()
        .filter(|track| roots.iter().any(|root| track.path.starts_with(root)))
        .map(|track| track.path)
        .collect();
    if paths.is_empty() {
        return Ok(0);
    }
    let deleted = store.delete_tracks_by_paths(&paths)?;
    info!(deleted, "purged tracks under removed library folders");
    Ok(deleted)
}

/// Groups digits in threes, e.g. `1234` -> `"1,234"`, matching the status
/// bar's "Scanning 1,234 / 5,678" wording.
fn format_count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.char_indices() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
