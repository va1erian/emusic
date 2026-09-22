//! Folder reconciliation and scan lifecycle for [`LibraryBackend`].
//!
//! Kept apart from the threading/trait plumbing in `mod.rs`: this is where the
//! config's folder list is turned into store rows, a watcher root set and a
//! scan (with purging for removed folders).

use std::path::PathBuf;

use emusic_library::scanner::CancelToken;
use tracing::warn;

use super::scan::ScanHandle;
use super::{LibraryBackend, enabled_roots, loader, scan};

impl LibraryBackend {
    /// Persists `folders`, updates the watcher, and starts the background
    /// loader on the first call. The initial load emits a snapshot as soon as
    /// the existing store is read, then rescans if any enabled roots exist.
    ///
    /// Subsequent calls trigger an incremental scan when the enabled roots
    /// changed or a folder was removed (whose tracks are then purged).
    pub(super) fn apply_folders(&mut self, folders: &[PathBuf]) {
        let removed = self.sync_folders(folders);
        let roots = enabled_roots(&self.folders);

        if let Some(watcher) = &self.watcher
            && let Err(err) = watcher.set_roots(roots.clone())
        {
            warn!(%err, "could not update watched folders");
        }

        if !self.loader_started {
            self.loader_started = true;
            let scan = (!roots.is_empty()).then(|| self.begin_scan());
            loader::spawn(
                self.store.clone(),
                self.folders.clone(),
                self.update_tx.clone(),
                scan,
            );
        } else if roots != self.scanned_roots || !removed.is_empty() {
            let handle = self.begin_scan();
            scan::spawn(
                self.store.clone(),
                self.folders.clone(),
                roots.clone(),
                self.update_tx.clone(),
                handle,
                removed,
            );
        }
        self.scanned_roots = roots;
    }

    /// Registers a fresh scan (cancellation handle + id) as the active one.
    pub(super) fn begin_scan(&mut self) -> ScanHandle {
        self.next_scan_id += 1;
        let handle = ScanHandle {
            cancel: CancelToken::default(),
            id: self.next_scan_id,
        };
        self.active_scan = Some(handle.clone());
        handle
    }

    /// Reconciles the store's folder rows with `folders` (the config's list),
    /// adding missing paths and removing rows that are no longer configured.
    /// Returns the paths that were removed so the caller can purge their
    /// tracks.
    fn sync_folders(&mut self, folders: &[PathBuf]) -> Vec<PathBuf> {
        let store = self
            .store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let removed = match store.list_folders() {
            Ok(existing) => {
                let mut removed = Vec::new();
                for folder in existing {
                    if folders.contains(&folder.path) {
                        continue;
                    }
                    match store.remove_folder(folder.id) {
                        Ok(()) => removed.push(folder.path),
                        Err(err) => {
                            warn!(path = %folder.path.display(), %err, "could not remove library folder")
                        }
                    }
                }
                removed
            }
            Err(err) => {
                warn!(%err, "could not list library folders");
                Vec::new()
            }
        };

        for path in folders {
            if let Err(err) = store.add_folder(path) {
                warn!(path = %path.display(), %err, "could not add library folder");
            }
        }
        match store.list_folders() {
            Ok(list) => self.folders = list,
            Err(err) => warn!(%err, "could not list library folders"),
        }
        removed
    }
}
