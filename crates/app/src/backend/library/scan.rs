//! On-demand and watch-triggered scans for the library backend.
//!
//! Each scan runs on its own thread, writes results to the store, then builds
//! a fresh snapshot and sends it to the UI thread. This keeps all file I/O
//! off the UI thread.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use emusic_library::scanner::{CancelToken, scan};
use emusic_library::{Folder, Store};
use tracing::{info, warn};

use super::source::Snapshot;
use super::{Update, scan_options};

/// Spawns a scan of `roots` on a background thread.
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    roots: Vec<PathBuf>,
    updates: Sender<Update>,
) {
    thread::spawn(move || {
        if let Err(err) = run(&store, &folders, &roots, &updates) {
            warn!(%err, "library scan failed");
        }
    });
}

fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    roots: &[PathBuf],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    // Watch-driven scans don't surface per-file progress (the startup scan
    // already covers that); drop the receiver so `scan`'s progress sends are
    // cheap and never queue up for a large library.
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();
    drop(progress_rx);
    let cancel = CancelToken::default();

    let summary = {
        let mut store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        scan(&mut store, roots, &scan_options(), &progress_tx, &cancel)?
    };

    info!(
        added = summary.tracks_added,
        updated = summary.tracks_updated,
        deleted = summary.tracks_deleted,
        "watch scan complete"
    );

    let store = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = Snapshot::from_store(&store, folders)?;
    let _ = updates.send(Update::Snapshot(snapshot));
    Ok(())
}
