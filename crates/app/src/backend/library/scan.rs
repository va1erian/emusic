//! Background library scans: on-demand rescans, watch-triggered updates and
//! removal purges.
//!
//! Each scan runs on its own thread, writes results to the store, then builds
//! a fresh snapshot and sends it to the UI thread, forwarding coarse progress
//! updates for the status bar. All file I/O stays off the UI thread.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

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
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    roots: Vec<PathBuf>,
    updates: Sender<Update>,
    handle: ScanHandle,
    purge: Vec<PathBuf>,
) {
    thread::spawn(move || {
        if let Err(err) = run(&store, &folders, &roots, &updates, &handle, &purge) {
            warn!(%err, "library scan failed");
        }
    });
}

/// Runs one scan to completion, reporting progress, a final snapshot and the
/// [`Update::ScanFinished`] that clears the scanning state.
pub(crate) fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Sender<Update>,
    handle: &ScanHandle,
    purge: &[PathBuf],
) -> anyhow::Result<()> {
    let result = run_inner(store, folders, roots, updates, handle, purge);
    if result.is_err() {
        let _ = updates.send(Update::Status(String::new()));
    }
    let _ = updates.send(Update::ScanFinished(handle.id));
    result
}

fn run_inner(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Sender<Update>,
    handle: &ScanHandle,
    purge: &[PathBuf],
) -> anyhow::Result<()> {
    purge_removed_roots(store, purge)?;
    if !roots.is_empty() {
        scan_roots(store, roots, updates, &handle.cancel)?;
    }

    let snapshot = {
        let store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot::from_store(&store, folders)?
    };
    let _ = updates.send(Update::Snapshot(snapshot));
    let _ = updates.send(Update::Status(String::new()));
    Ok(())
}

/// Scans `roots` on a worker thread while this thread forwards coarse
/// progress updates to the UI.
fn scan_roots(
    store: &Arc<Mutex<Store>>,
    roots: &[PathBuf],
    updates: &Sender<Update>,
    cancel: &CancelToken,
) -> anyhow::Result<()> {
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();
    let scan_store = store.clone();
    let roots = roots.to_vec();
    let cancel = cancel.clone();
    let scan_handle = thread::spawn(move || {
        let mut store = scan_store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        scan(&mut store, &roots, &scan_options(), &progress_tx, &cancel)
    });

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
        Ok(Ok(summary)) => {
            info!(
                added = summary.tracks_added,
                updated = summary.tracks_updated,
                deleted = summary.tracks_deleted,
                cancelled = summary.cancelled,
                "library scan complete"
            );
            Ok(())
        }
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(anyhow::anyhow!("scanner thread panicked")),
    }
}

/// Deletes every stored track under a removed folder. Folder rows are
/// independent of tracks (see `Store::remove_folder`), so without this a
/// removed folder's music would linger in the library.
fn purge_removed_roots(store: &Arc<Mutex<Store>>, roots: &[PathBuf]) -> anyhow::Result<()> {
    if roots.is_empty() {
        return Ok(());
    }
    let mut store = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
