//! Real library backend: SQLite store + in-memory index + scanner + watcher
//! + stats, all behind the app's [`LibraryDataSource`] trait.
//!
//! All file I/O and database work happens on background threads (opening the
//! store is a small local-disk operation done once at startup); the UI thread
//! only swaps in completed snapshots and applies in-memory play records. This
//! keeps the app responsive even when the library lives on a mapped network
//! drive, where directory walks and tag reads are slow.

pub(crate) mod folders;
pub(crate) mod loader;
pub(crate) mod scan;
pub(crate) mod source;
pub(crate) mod stats;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use emusic_library::stats::StatsRecorder;
use emusic_library::watch::{WatchEvent, Watcher};
use emusic_library::{Folder, Store, TrackId};
use tracing::{info, warn};

use crate::backend::PlayMessage;
use crate::library_api::{
    AlbumInfo, ArtistInfo, DirNodeInfo, FolderInfo, GenreInfo, HistoryEntry, LibraryDataSource,
    StatsWindow, TrackInfo,
};

use scan::ScanHandle;
use source::Snapshot;

/// Messages sent from background threads to the UI-owning backend.
pub(crate) enum Update {
    /// A freshly built, UI-ready snapshot to swap in. Boxed to keep this
    /// small, frequently-created channel message compact.
    Snapshot(Box<Snapshot>),
    /// A progress line for the status bar; empty clears it.
    Status(String),
    /// A scan with this id finished; clears the scanning state if it is still
    /// the current one (an older scan's completion is ignored).
    ScanFinished(u64),
}

/// [`LibraryDataSource`] implementation backed by `emusic-library`.
pub struct LibraryBackend {
    /// Shared with the scanner writer thread and the stats recorder.
    store: Arc<Mutex<Store>>,
    snapshot: Snapshot,
    folders: Vec<Folder>,
    /// Enabled roots the last scan covered, so adding a folder later triggers
    /// a scan without rescanning the whole library on every settings tick.
    scanned_roots: Vec<PathBuf>,
    updates: Receiver<Update>,
    update_tx: Sender<Update>,
    watch_events: Receiver<WatchEvent>,
    play_messages: Receiver<PlayMessage>,
    play_message_tx: Sender<PlayMessage>,
    stats_recorder: StatsRecorder,
    watcher: Option<Watcher>,
    status: Option<String>,
    loader_started: bool,
    /// Id handed to the next scan; makes stale `ScanFinished` messages
    /// detectable.
    next_scan_id: u64,
    /// The scan currently running (cancel handle + id), if any.
    active_scan: Option<ScanHandle>,
    /// Shared with the player backend; used to read tracker module tags
    /// during scans. `None` when BASS failed to initialize, in which case
    /// modules are counted but skipped (see [`emusic_library::scanner`]).
    bass: Option<Arc<bass::Bass>>,
}

impl Default for LibraryBackend {
    fn default() -> Self {
        Self::new(None)
    }
}

impl LibraryBackend {
    /// Opens the default store (`%LOCALAPPDATA%\emusic\library.db`), falling
    /// back to an in-memory store if the default path is unavailable.
    ///
    /// `bass` is the instance shared with the player, used to read tracker
    /// module tags during scans; pass `None` when BASS is unavailable.
    pub fn new(bass: Option<Arc<bass::Bass>>) -> Self {
        let store = match Store::open_default() {
            Ok(store) => {
                info!("opened library store");
                store
            }
            Err(err) => {
                warn!(%err, "could not open library store; using an in-memory store");
                Store::open_in_memory().expect("in-memory store always opens")
            }
        };
        Self::with_store(store, bass)
    }

    /// Creates a backend around an existing store. Useful in tests.
    pub fn with_store(store: Store, bass: Option<Arc<bass::Bass>>) -> Self {
        let store = Arc::new(Mutex::new(store));
        let stats_recorder = StatsRecorder::spawn(store.clone());

        let (update_tx, updates) = std::sync::mpsc::channel();
        let (watch_tx, watch_events) = std::sync::mpsc::channel();
        let (play_message_tx, play_messages) = std::sync::mpsc::channel();

        let watcher = match Watcher::new(Default::default(), watch_tx) {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                warn!(%err, "folder watcher failed to start; file changes will not be detected");
                None
            }
        };

        Self {
            store,
            snapshot: Snapshot::default(),
            folders: Vec::new(),
            scanned_roots: Vec::new(),
            updates,
            update_tx,
            watch_events,
            play_messages,
            play_message_tx,
            stats_recorder,
            watcher,
            status: None,
            loader_started: false,
            next_scan_id: 0,
            active_scan: None,
            bass,
        }
    }

    /// Sender the player adapter uses to report track starts and finishes.
    pub(crate) fn play_message_tx(&self) -> Sender<PlayMessage> {
        self.play_message_tx.clone()
    }

    /// Writes a just-started play's history row and mirrors it into the
    /// snapshot, so the History view shows the track immediately.
    ///
    /// The store write is synchronous (an indexed lookup plus one insert,
    /// like the other user-action store calls) because the snapshot needs
    /// the assigned row id right away for "remove entry".
    fn begin_play(&mut self, path: &Path, started_at: i64) {
        let mut store = self
            .store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let track_id = match store.resolve_track_id(path) {
            Ok(Some(track_id)) => track_id,
            Ok(None) => {
                warn!(path = %path.display(), "no library track for started path, dropping record");
                return;
            }
            Err(err) => {
                warn!(%err, "failed to resolve started path");
                return;
            }
        };
        match store.begin_play(track_id, started_at) {
            Ok(play_id) => {
                self.snapshot
                    .record_play_started(track_id.0 as u64, play_id, started_at);
            }
            Err(err) => warn!(%err, "failed to record started play"),
        }
    }
}

impl LibraryDataSource for LibraryBackend {
    fn tracks(&self) -> &[TrackInfo] {
        &self.snapshot.tracks
    }

    fn albums(&self) -> &[AlbumInfo] {
        &self.snapshot.albums
    }

    fn artists(&self) -> &[ArtistInfo] {
        &self.snapshot.artists
    }

    fn genres(&self) -> &[GenreInfo] {
        &self.snapshot.genres
    }

    fn folders(&self) -> &[FolderInfo] {
        &self.snapshot.folders
    }

    fn dir_tree(&self) -> &[DirNodeInfo] {
        &self.snapshot.dirs
    }

    fn history(&self) -> &[HistoryEntry] {
        &self.snapshot.history
    }

    fn most_played(&self, window: StatsWindow) -> &[TrackInfo] {
        self.snapshot.most_played(window)
    }

    fn remove_history_entry(&mut self, id: i64) {
        let store = self
            .store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(err) = store.delete_play(id) {
            warn!(%err, "failed to remove history entry");
        }
        self.snapshot.remove_history(id);
    }

    fn clear_history(&mut self) {
        let store = self
            .store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(err) = store.clear_plays() {
            warn!(%err, "failed to clear history");
        }
        self.snapshot.clear_history();
    }

    fn set_starred(&mut self, id: u64, starred: bool) {
        {
            let store = self
                .store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(err) = store.set_starred(TrackId(id as i64), starred) {
                warn!(%err, "failed to update starred state");
            }
        }
        self.snapshot.set_starred(id, starred);
    }

    fn tick(&mut self) {
        while let Ok(update) = self.updates.try_recv() {
            match update {
                Update::Snapshot(snapshot) => self.snapshot = *snapshot,
                Update::Status(text) => {
                    self.status = if text.is_empty() { None } else { Some(text) };
                }
                Update::ScanFinished(id) => {
                    if self.active_scan.as_ref().is_some_and(|scan| scan.id == id) {
                        self.active_scan = None;
                    }
                }
            }
        }

        while let Ok(event) = self.watch_events.try_recv() {
            match event {
                WatchEvent::ScanRequested { paths } => {
                    info!(count = paths.len(), "library watch triggered rescan");
                    let handle = self.begin_scan();
                    scan::spawn(
                        self.store.clone(),
                        self.folders.clone(),
                        paths,
                        self.update_tx.clone(),
                        handle,
                        Vec::new(),
                        self.bass.clone(),
                    );
                }
            }
        }

        while let Ok(message) = self.play_messages.try_recv() {
            match message {
                PlayMessage::Started { path, started_at } => {
                    self.begin_play(&path, started_at);
                }
                PlayMessage::Finished(record) => {
                    self.stats_recorder.record(record.clone());
                    self.snapshot.record_play_finished(&record);
                }
            }
        }

        while let Some(path) = crate::settings::folder_picker::try_recv() {
            info!(path = %path.display(), "folder chosen via picker");
            let mut folders: Vec<PathBuf> = self
                .folders
                .iter()
                .map(|folder| folder.path.clone())
                .collect();
            if !folders.contains(&path) {
                folders.push(path);
            }
            self.apply_folders(&folders);
        }
    }

    fn set_folders(&mut self, folders: &[PathBuf]) {
        self.apply_folders(folders);
    }

    fn rescan(&mut self) {
        let roots = enabled_roots(&self.folders);
        if roots.is_empty() {
            info!("rescan skipped: no enabled library folders");
            return;
        }
        info!(folders = roots.len(), "on-demand library rescan requested");
        let handle = self.begin_scan();
        scan::spawn(
            self.store.clone(),
            self.folders.clone(),
            roots,
            self.update_tx.clone(),
            handle,
            Vec::new(),
            self.bass.clone(),
        );
    }

    fn cancel_scan(&mut self) {
        if let Some(handle) = &self.active_scan {
            info!("library scan cancellation requested");
            handle.cancel.cancel();
        }
    }

    fn is_scanning(&self) -> bool {
        self.active_scan.is_some()
    }

    fn status_text(&self) -> Option<String> {
        self.status.clone()
    }
}

impl Drop for LibraryBackend {
    fn drop(&mut self) {
        if let Some(watcher) = self.watcher.take()
            && let Err(err) = watcher.shutdown()
        {
            warn!(%err, "folder watcher did not shut down cleanly");
        }
        // Ensure queued play records are written before the recorder's
        // writer thread is joined by dropping the field.
        self.stats_recorder.flush();
    }
}

/// Roots that should be scanned and watched: enabled folders.
///
/// The settings UI (#19) adds and removes whole folders; per-folder
/// enable/watch toggles are not exposed yet, so every enabled folder is
/// watched and changes on a mapped drive are picked up, including by the
/// watcher's remote-root polling.
pub(crate) fn enabled_roots(folders: &[Folder]) -> Vec<PathBuf> {
    folders
        .iter()
        .filter(|folder| folder.enabled)
        .map(|folder| folder.path.clone())
        .collect()
}

/// Scanner options reused for startup and watch-driven scans.
///
/// `bass` is the instance shared with the player backend
/// ([`emusic_player::BassBackend`] holds a clone too); when `None` (BASS
/// failed to initialize) module files are still counted but skipped during
/// scanning, while streamed formats keep getting their tags read by lofty.
pub(crate) fn scan_options(bass: Option<Arc<bass::Bass>>) -> emusic_library::scanner::ScanOptions {
    emusic_library::scanner::ScanOptions {
        bass,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
