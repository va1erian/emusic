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

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use emusic_library::stats::{PlayRecord, StatsRecorder};
use emusic_library::watch::{WatchEvent, Watcher};
use emusic_library::{Folder, Store};
use tracing::{info, warn};

use crate::library_api::{
    AlbumInfo, ArtistInfo, FolderInfo, HistoryEntry, LibraryDataSource, TrackInfo,
};

use scan::ScanHandle;
use source::Snapshot;

/// Messages sent from background threads to the UI-owning backend.
pub(crate) enum Update {
    /// A freshly built, UI-ready snapshot to swap in.
    Snapshot(Snapshot),
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
    play_records: Receiver<PlayRecord>,
    play_record_tx: Sender<PlayRecord>,
    stats_recorder: StatsRecorder,
    watcher: Option<Watcher>,
    status: Option<String>,
    loader_started: bool,
    /// Id handed to the next scan; makes stale `ScanFinished` messages
    /// detectable.
    next_scan_id: u64,
    /// The scan currently running (cancel handle + id), if any.
    active_scan: Option<ScanHandle>,
}

impl Default for LibraryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryBackend {
    /// Opens the default store (`%LOCALAPPDATA%\emusic\library.db`), falling
    /// back to an in-memory store if the default path is unavailable.
    pub fn new() -> Self {
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
        Self::with_store(store)
    }

    /// Creates a backend around an existing store. Useful in tests.
    pub fn with_store(store: Store) -> Self {
        let store = Arc::new(Mutex::new(store));
        let stats_recorder = StatsRecorder::spawn(store.clone());

        let (update_tx, updates) = std::sync::mpsc::channel();
        let (watch_tx, watch_events) = std::sync::mpsc::channel();
        let (play_record_tx, play_records) = std::sync::mpsc::channel();

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
            play_records,
            play_record_tx,
            stats_recorder,
            watcher,
            status: None,
            loader_started: false,
            next_scan_id: 0,
            active_scan: None,
        }
    }

    /// Sender the player adapter uses to report completed plays.
    pub fn play_record_tx(&self) -> Sender<PlayRecord> {
        self.play_record_tx.clone()
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

    fn genres(&self) -> &[String] {
        &self.snapshot.genres
    }

    fn folders(&self) -> &[FolderInfo] {
        &self.snapshot.folders
    }

    fn history(&self) -> &[HistoryEntry] {
        &self.snapshot.history
    }

    fn most_played(&self) -> &[TrackInfo] {
        &self.snapshot.most_played
    }

    fn tick(&mut self) {
        while let Ok(update) = self.updates.try_recv() {
            match update {
                Update::Snapshot(snapshot) => self.snapshot = snapshot,
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
                    );
                }
            }
        }

        while let Ok(record) = self.play_records.try_recv() {
            self.stats_recorder.record(record.clone());
            self.snapshot.record_play(&record);
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
/// BASS is owned by the player backend ([`emusic_player::BassBackend`] takes
/// the handle), so module files are counted but skipped during scanning;
/// streamed formats still get their tags read by lofty.
pub(crate) fn scan_options() -> emusic_library::scanner::ScanOptions {
    emusic_library::scanner::ScanOptions::default()
}

#[cfg(test)]
mod tests;
