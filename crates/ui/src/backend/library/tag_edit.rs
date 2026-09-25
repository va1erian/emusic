//! Background tag-edit worker.
//!
//! Applying a batch rewrites each target file's tags — slow I/O, possibly on
//! a mapped network drive — and then syncs the store rows, so it runs on its
//! own thread. Like the scanner (#69) it uses a private store connection when
//! the store is file-backed, so the UI's own reads never wait behind the
//! edit. Outcomes and, when at least one edit succeeded, a refreshed snapshot
//! go back through the backend's update channel.

use std::sync::{Arc, Mutex};

use emusic_library::tags::{EditOutcome, EditRequest, edit_tags};
use emusic_library::{Folder, Store};
use tracing::warn;

use super::source::Snapshot;
use super::{Update, Updates, private_store};

/// Spawns a worker that applies `requests` to their files and the store,
/// reporting the outcomes and (when any edit succeeded) a fresh snapshot.
pub(crate) fn spawn(
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    requests: Vec<EditRequest>,
    updates: Updates,
) {
    std::thread::spawn(move || {
        let outcomes = run(&store, &folders, &requests, &updates);
        let _ = updates.send(Update::TagEdits(outcomes));
    });
}

/// Applies the batch and sends a refreshed snapshot when something changed.
///
/// The store rows are synced inside [`edit_tags`]; the snapshot is rebuilt
/// from the store afterwards so the UI shows the new tags without a rescan.
/// A snapshot failure is logged and never discards the outcomes.
fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    requests: &[EditRequest],
    updates: &Updates,
) -> Vec<EditOutcome> {
    let outcomes = match private_store(store) {
        Some(mut private) => edit_tags(&mut private, requests),
        None => {
            let mut shared = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            edit_tags(&mut shared, requests)
        }
    };

    if outcomes.iter().any(|outcome| outcome.result.is_ok())
        && let Err(err) = send_snapshot(store, folders, updates)
    {
        warn!(%err, "failed to refresh the snapshot after tag edits");
    }
    outcomes
}

/// Rebuilds a snapshot from the shared store and sends it to the UI thread.
fn send_snapshot(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    updates: &Updates,
) -> anyhow::Result<()> {
    let snapshot = {
        let store = store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot::from_store(&store, folders)?
    };
    let _ = updates.send(Update::Snapshot(Box::new(snapshot)));
    Ok(())
}

#[cfg(test)]
mod tests;
