//! Background loading and startup scanning for the library backend.
//!
//! The initial load runs on its own thread so the UI thread is never blocked
//! by SQLite queries or network file stats. It emits an initial snapshot as
//! soon as the existing store is read, then runs the incremental scanner and
//! emits a refreshed snapshot when the scan finishes.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use emusic_library::scanner::{CancelToken, ScanEvent, scan};
use emusic_library::{Folder, Store};
use tracing::{info, warn};

use super::source::Snapshot;
use super::{Update, enabled_roots, scan_options};

/// Starts the initial load + startup scan on a background thread.
pub(crate) fn spawn(store: Arc<Mutex<Store>>, folders: Vec<Folder>, updates: Sender<Update>) {
    thread::spawn(move || {
        if let Err(err) = run(&store, &folders, &updates) {
            warn!(%err, "library startup load/scan failed");
        }
    });
}

fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    send_initial_snapshot(store, folders, updates)?;

    let roots = enabled_roots(folders);
    if roots.is_empty() {
        info!("no enabled library folders; skipping startup scan");
        return Ok(());
    }

    let cancel = CancelToken::default();
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();

    // Run the scan on another thread so this thread can forward progress
    // events without blocking on the store lock.
    let scan_store = store.clone();
    let scan_handle = thread::spawn(move || {
        let mut store = scan_store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        scan(&mut store, &roots, &scan_options(), &progress_tx, &cancel)
    });

    // Report roughly twice a second' worth of files rather than one message
    // per file, so a fast local scan never floods the UI-thread channel.
    const STATUS_EVERY: u64 = 50;
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
                processed,
                total,
                path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default()
            )));
        }
    }

    match scan_handle.join() {
        Ok(Ok(summary)) => {
            info!(
                added = summary.tracks_added,
                updated = summary.tracks_updated,
                deleted = summary.tracks_deleted,
                "startup scan complete"
            );
        }
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => return Err(anyhow::anyhow!("scanner thread panicked")),
    }

    send_initial_snapshot(store, folders, updates)?;
    let _ = updates.send(Update::Status(String::new()));
    Ok(())
}

fn send_initial_snapshot(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    let store = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = Snapshot::from_store(&store, folders)?;
    let _ = updates.send(Update::Snapshot(snapshot));
    Ok(())
}
