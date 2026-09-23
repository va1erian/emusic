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

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use emusic_library::scanner::{CancelToken, ScanEvent, scan};
use emusic_library::{Folder, Store};
use tracing::{info, warn};

use super::source::Snapshot;
use super::{Update, scan_options};

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

/// Spawns a scan of `roots` on a background thread, first purging tracks
/// under any `purge` prefixes (folders removed from the library).
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    roots: Vec<PathBuf>,
    updates: Sender<Update>,
    handle: ScanHandle,
    purge: Vec<PathBuf>,
    bass: Option<Arc<bass::Bass>>,
) {
    std::thread::spawn(move || {
        if let Err(err) = run(&store, &folders, &roots, &updates, &handle, &purge, bass) {
            warn!(%err, "library scan failed");
        }
    });
}

/// Runs one scan to completion, reporting progress, a final snapshot and the
/// [`Update::ScanFinished`] that clears the scanning state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Sender<Update>,
    handle: &ScanHandle,
    purge: &[PathBuf],
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<()> {
    let result = run_inner(store, folders, roots, updates, handle, purge, bass);
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
    updates: &Sender<Update>,
    handle: &ScanHandle,
    purge: &[PathBuf],
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<()> {
    // Real deployments scan through a private connection, so the shared lock
    // stays free for the UI. In-memory stores (unit tests) cannot be
    // reopened; for those, fall back to the shared connection and hold its
    // lock for the run, matching the pre-#69 behaviour.
    match private_scan_store(store) {
        Some(mut scan_store) => {
            purge_removed_roots(&mut scan_store, purge)?;
            if !roots.is_empty() {
                scan_roots(&mut scan_store, roots, updates, &handle.cancel, bass)?;
            }
        }
        None => {
            let mut shared = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            purge_removed_roots(&mut shared, purge)?;
            if !roots.is_empty() {
                scan_roots(&mut shared, roots, updates, &handle.cancel, bass)?;
            }
        }
    }

    let snapshot = {
        let store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot::from_store(&store, folders)?
    };
    let _ = updates.send(Update::Snapshot(Box::new(snapshot)));
    let _ = updates.send(Update::Status(String::new()));
    Ok(())
}

/// Opens the scan's private connection to the same file as the shared store,
/// if there is one.
///
/// The shared lock is held only for the `open_second` call itself, never
/// while a scan or purge runs, so a UI read can never wait on this. Returns
/// `None` for an in-memory store (there is no file to reopen), which callers
/// handle by falling back to the shared connection.
fn private_scan_store(store: &Arc<Mutex<Store>>) -> Option<Store> {
    let shared = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    shared.open_second().ok()
}

/// Scans `roots` on a worker thread while this thread forwards coarse
/// progress updates to the UI.
fn scan_roots(
    store: &mut Store,
    roots: &[PathBuf],
    updates: &Sender<Update>,
    cancel: &CancelToken,
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<()> {
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
                deleted = summary.tracks_deleted,
                cancelled = summary.cancelled,
                "library scan complete"
            );
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// Deletes every stored track under a removed folder. Folder rows are
/// independent of tracks (see `Store::remove_folder`), so without this a
/// removed folder's music would linger in the library.
fn purge_removed_roots(store: &mut Store, roots: &[PathBuf]) -> anyhow::Result<()> {
    if roots.is_empty() {
        return Ok(());
    }
    let paths: Vec<PathBuf> = store
        .load_all_tracks()?
        .into_iter()
        .filter(|track| roots.iter().any(|root| track.path.starts_with(root)))
        .map(|track| track.path)
        .collect();
    if !paths.is_empty() {
        let deleted = store.delete_tracks_by_paths(&paths)?;
        info!(deleted, "purged tracks under removed library folders");
    }
    Ok(())
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
mod tests {
    use super::format_count;

    #[test]
    fn format_count_groups_thousands() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(12_345), "12,345");
        assert_eq!(format_count(1_234_567), "1,234,567");
    }
}
