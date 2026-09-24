//! Background startup loading for the library backend.
//!
//! The initial load runs on its own thread so the UI thread is never blocked
//! by SQLite queries or network file stats. It opens a private store
//! connection ([`Store::open_second`]) so it never holds the shared store
//! mutex while reading, emits a snapshot as soon as the existing store is
//! read, then (when there are enabled roots) runs the startup incremental
//! scan via [`super::scan`]. Opening the private connection happens first,
//! so the first snapshot also never contends with the UI's own reads (#69).

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
    bass: Option<Arc<bass::Bass>>,
) {
    thread::spawn(move || {
        let private = {
            let shared = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match shared.open_second() {
                Ok(second) => Some(second),
                Err(err) => {
                    warn!(%err, "no second library connection for startup load");
                    None
                }
            }
        };
        if let Err(err) = run(&store, private, &folders, &updates, scan, bass) {
            warn!(%err, "library startup load failed");
        }
    });
}

fn run(
    store: &Arc<Mutex<Store>>,
    private: Option<Store>,
    folders: &[Folder],
    updates: &Sender<Update>,
    scan: Option<ScanHandle>,
    bass: Option<Arc<bass::Bass>>,
) -> anyhow::Result<()> {
    match private {
        Some(private) => send_initial_snapshot(&private, folders, updates)?,
        // In-memory stores cannot be reopened (unit tests); fall back to the
        // shared connection for the startup snapshot only.
        None => send_shared_snapshot(store, folders, updates)?,
    }

    let Some(handle) = scan else {
        info!("no enabled library folders; skipping startup scan");
        return Ok(());
    };
    let roots = enabled_roots(folders);
    scan::run(store, folders, &roots, updates, &handle, &[], bass)
}

fn send_initial_snapshot(
    store: &Store,
    folders: &[Folder],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    let snapshot = Snapshot::from_store(store, folders)?;
    let _ = updates.send(Update::Snapshot(Box::new(snapshot)));
    Ok(())
}

fn send_shared_snapshot(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    let store = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    send_initial_snapshot(&store, folders, updates)
}
