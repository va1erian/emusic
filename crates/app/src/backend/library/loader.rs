//! Background startup loading for the library backend.
//!
//! The initial load runs on its own thread so the UI thread is never blocked
//! by SQLite queries or network file stats. It emits a snapshot as soon as
//! the existing store is read, then (when there are enabled roots) runs the
//! startup incremental scan via [`super::scan`].

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use emusic_library::{Folder, Store};
use tracing::{info, warn};

use super::scan::{self, ScanHandle};
use super::source::Snapshot;
use super::{Update, enabled_roots};

/// Starts the initial load on a background thread, then the startup scan if
/// `scan` is given (there are enabled roots to scan).
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    updates: Sender<Update>,
    scan: Option<ScanHandle>,
) {
    thread::spawn(move || {
        if let Err(err) = run(&store, &folders, &updates, scan) {
            warn!(%err, "library startup load failed");
        }
    });
}

fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Sender<Update>,
    scan: Option<ScanHandle>,
) -> anyhow::Result<()> {
    send_initial_snapshot(store, folders, updates)?;

    let Some(handle) = scan else {
        info!("no enabled library folders; skipping startup scan");
        return Ok(());
    };
    let roots = enabled_roots(folders);
    scan::run(store, folders, &roots, updates, &handle, &[])
}

fn send_initial_snapshot(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    let snapshot = {
        let store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot::from_store(&store, folders)?
    };
    let _ = updates.send(Update::Snapshot(snapshot));
    Ok(())
}
