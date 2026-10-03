//! Background sync of configured remote servers (#391).
//!
//! The worker pulls each server's delta, upserts/deletes the remote rows in
//! the library store from a private connection (never holding the UI's store
//! lock), then rebuilds one snapshot for the UI. It is started by the library
//! backend when the server list changes or a manual sync is requested, and is
//! a no-op while another sync is already running.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use emusic_client::{CredentialStore, RemoteClient, TrackCache, TrackView};
use emusic_core::{ArtSource, Track, TrackId, TrackKind};
use emusic_library::{Folder, RemoteTrack, Store};
use tracing::{debug, info, warn};

use super::source::Snapshot;
use super::star_sync::{StarSyncOutcome, sync_stars};
use super::{Updates, private_store};
use crate::remote::RemoteServer;

/// Remote-sync state owned by [`LibraryBackend`](super::LibraryBackend).
pub(crate) struct RemoteState {
    servers: Vec<RemoteServer>,
    cache: Option<TrackCache>,
    credentials: Option<CredentialStore>,
    running: Arc<AtomicBool>,
    /// The sync requested while one was running, run next by the worker.
    queued: Arc<Mutex<Option<SyncJob>>>,
    status: Option<String>,
}

impl RemoteState {
    /// Builds the state, resolving the cache and credential directories.
    pub(crate) fn new() -> Self {
        Self {
            servers: Vec::new(),
            cache: TrackCache::new().ok(),
            credentials: CredentialStore::new().ok(),
            running: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(None)),
            status: None,
        }
    }

    /// Replaces the configured server list.
    pub(crate) fn set_servers(&mut self, servers: &[RemoteServer]) {
        self.servers = servers.to_vec();
    }

    /// The current server list.
    pub(crate) fn servers(&self) -> &[RemoteServer] {
        &self.servers
    }

    /// The last remote-sync status line, if any.
    pub(crate) fn status(&self) -> Option<String> {
        self.status.clone()
    }

    /// The remote cache root, or `None` when the cache could not be resolved.
    pub(crate) fn cache_root(&self) -> Option<PathBuf> {
        self.cache.as_ref().map(|cache| cache.root().to_path_buf())
    }

    /// Forgets servers that were removed: drops their stored credentials.
    pub(crate) fn forget(&self, server_ids: &[String]) {
        let Some(credentials) = &self.credentials else {
            return;
        };
        for id in server_ids {
            if let Err(error) = credentials.remove(id) {
                warn!(server_id = %id, %error, "could not remove credentials");
            }
        }
    }

    /// Starts a sync on a background thread. While one is already running,
    /// the request is queued instead (replacing any queued one), so a change
    /// made during a sync — such as a star (#516) — is picked up by one more
    /// pass with the latest server list.
    pub(crate) fn spawn_sync(
        &mut self,
        store: Arc<Mutex<Store>>,
        folders: Vec<Folder>,
        updates: Updates,
        only_root: Option<PathBuf>,
    ) {
        if self.servers.is_empty() {
            return;
        }
        let Some(cache) = self.cache.clone() else {
            warn!("remote cache unavailable; skipping sync");
            return;
        };
        let Some(credentials) = self.credentials.clone() else {
            warn!("credential store unavailable; skipping remote sync");
            return;
        };
        *lock(&self.queued) = Some(SyncJob {
            store,
            folders,
            servers: self.servers.clone(),
            cache,
            credentials,
            updates,
            only_root,
        });
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let running = Arc::clone(&self.running);
        let queued = Arc::clone(&self.queued);
        std::thread::spawn(move || drain(&running, &queued));
    }
}

/// Everything one sync pass needs.
struct SyncJob {
    store: Arc<Mutex<Store>>,
    folders: Vec<Folder>,
    servers: Vec<RemoteServer>,
    cache: TrackCache,
    credentials: CredentialStore,
    updates: Updates,
    only_root: Option<PathBuf>,
}

/// The sync worker: runs queued jobs until none is left, then clears
/// `running`. A job queued between the last check and clearing the flag
/// (whose `spawn_sync` saw `running` still set) is caught by the re-check.
fn drain(running: &AtomicBool, queued: &Mutex<Option<SyncJob>>) {
    loop {
        let job = lock(queued).take();
        if let Some(job) = job {
            run_job(&job);
            continue;
        }
        running.store(false, Ordering::SeqCst);
        if lock(queued).is_none() || running.swap(true, Ordering::SeqCst) {
            return;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs one sync pass and reports a failure on the status bar.
fn run_job(job: &SyncJob) {
    let _ = job
        .updates
        .send(super::Update::Status("Syncing remote servers...".into()));
    match run(job) {
        Ok(()) => info!("remote sync finished"),
        Err(error) => {
            warn!(%error, "remote sync failed");
            let _ = job.updates.send(super::Update::Status(format!(
                "Remote sync failed: {error}"
            )));
        }
    }
}

/// Syncs every server, then posts one refreshed snapshot.
fn run(job: &SyncJob) -> Result<()> {
    let SyncJob {
        store,
        folders,
        servers,
        cache,
        credentials,
        updates,
        only_root,
    } = job;
    let only_root = only_root.as_deref();
    let mut total = 0usize;
    let mut failures = 0usize;
    let mut star_failures = 0usize;
    for server in servers {
        match sync_server(store, server, cache, credentials) {
            Ok(synced) => {
                total += synced.tracks;
                match synced.stars {
                    Ok(stars) => debug!(
                        server = %server.name,
                        pushed = stars.pushed,
                        applied = stars.applied,
                        "starred sync done"
                    ),
                    Err(error) => {
                        star_failures += 1;
                        warn!(server = %server.name, error = format!("{error:#}"), "starred sync failed");
                        let _ = updates.send(super::Update::Status(format!(
                            "{}: starred sync failed: {error:#}",
                            server.name
                        )));
                    }
                }
            }
            Err(error) => {
                failures += 1;
                warn!(server = %server.name, %error, "remote server sync failed");
                let _ = updates.send(super::Update::Status(format!("{}: {error}", server.name)));
            }
        }
    }

    // Rebuild one snapshot so the remote rows appear in every view.
    let snapshot = match private_store(store) {
        Some(store) => Snapshot::from_store(&store, folders, only_root)?,
        None => {
            let guard = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Snapshot::from_store(&guard, folders, only_root)?
        }
    };
    let _ = updates.send(super::Update::Snapshot(Box::new(snapshot)));

    let _ = updates.send(super::Update::Status(summary(
        total,
        failures,
        star_failures,
    )));
    Ok(())
}

/// The status line after a sync pass.
fn summary(total: usize, failures: usize, star_failures: usize) -> String {
    let mut message = format!("Remote sync: {total} track(s)");
    if failures > 0 {
        message.push_str(&format!(", {failures} server(s) failed"));
    }
    if star_failures > 0 {
        message.push_str(&format!(
            ", starred sync failed on {star_failures} server(s)"
        ));
    }
    message
}

/// The outcome of syncing one server.
struct ServerSync {
    /// Tracks in the library delta.
    tracks: usize,
    /// The starred sync, which runs after the delta; its failure does not
    /// undo the delta.
    stars: Result<StarSyncOutcome>,
}

/// Syncs one server: its library delta, then its starred tracks (#516).
fn sync_server(
    store: &Arc<Mutex<Store>>,
    server: &RemoteServer,
    cache: &TrackCache,
    credentials: &CredentialStore,
) -> Result<ServerSync> {
    let endpoint = server.endpoint();
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let token = crate::backend::remote_auth::ensure_token(&client, credentials, &server.id)
        .map_err(anyhow::Error::msg)?;
    let sync = |store: &mut Store| -> Result<ServerSync> {
        let tracks = apply_delta(store, &client, &token, server, cache)?;
        let stars = sync_stars(store, &client, &token, &server.id);
        Ok(ServerSync { tracks, stars })
    };

    // Use a private connection for the writes so a slow sync never holds the
    // UI's store lock; fall back to the shared lock when there is no file.
    match private_store(store) {
        Some(mut store) => sync(&mut store),
        None => {
            let mut guard = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            sync(&mut guard)
        }
    }
}

/// Pulls the delta for `server` and applies it to `store`.
pub(super) fn apply_delta(
    store: &mut Store,
    client: &RemoteClient,
    token: &str,
    server: &RemoteServer,
    cache: &TrackCache,
) -> Result<usize> {
    let since = store.remote_since_version(&server.id)?;
    let delta = client
        .sync(token, since)
        .with_context(|| format!("syncing {}", server.name))?;

    let existing = store.remote_track_ids(&server.id)?;
    let mut seen = std::collections::HashSet::new();
    let mut records = Vec::with_capacity(delta.tracks.len());
    for view in &delta.tracks {
        let path = match cache.path_for(&server.id, view) {
            Ok(path) => path,
            Err(error) => {
                warn!(track = %view.id, %error, "skipping remote track with an unsafe id");
                continue;
            }
        };
        seen.insert(view.id.clone());
        records.push(RemoteTrack {
            remote_id: view.id.clone(),
            sync_version: view.sync_version,
            track: track_from_view(view, path),
        });
    }
    store.upsert_remote_tracks(&server.id, &mut records)?;

    // The server sends deletions as tombstones; apply exactly those. Only on a
    // first (full) sync do we also drop rows the server no longer reports,
    // because a delta does not contain the unchanged rows.
    let removed = ids_to_delete(since, existing, &seen, &delta.deleted);
    store.delete_remote_tracks(&server.id, &removed)?;
    store.set_remote_since_version(&server.id, delta.version)?;
    Ok(delta.tracks.len())
}

/// The remote ids to delete after applying a delta.
///
/// Tombstones are always applied. Rows missing from the delta are only
/// deletions on a **full** sync (`since == 0`); on an incremental sync they
/// are simply unchanged and must be kept.
fn ids_to_delete(
    since: i64,
    existing: std::collections::HashSet<String>,
    seen: &std::collections::HashSet<String>,
    tombstones: &[String],
) -> Vec<String> {
    let mut removed: std::collections::HashSet<String> = tombstones.iter().cloned().collect();
    if since == 0 {
        removed.extend(existing.into_iter().filter(|id| !seen.contains(id)));
    }
    removed.into_iter().collect()
}

/// Maps a server [`TrackView`] onto a local [`Track`] at `path`.
pub(crate) fn track_from_view(view: &TrackView, path: PathBuf) -> Track {
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| view.id.clone());
    let duration_ms = view
        .duration_secs
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).min(u32::MAX as f64) as u32)
        .unwrap_or(0);
    Track {
        id: TrackId::UNASSIGNED,
        path,
        dir,
        filename,
        ext: view.format.clone(),
        size: view.file_size,
        mtime: view.added_at,
        kind: if view.kind == "module" {
            TrackKind::Module
        } else {
            TrackKind::Stream
        },
        duration_ms,
        bitrate: None,
        sample_rate: None,
        channels: view
            .channels
            .map(|channels| channels.min(u8::MAX as u32) as u8),
        title: view
            .title
            .clone()
            .or_else(|| view.filename.clone().filter(|name| !name.trim().is_empty())),
        artist: view.artist.clone(),
        album_artist: view.album_artist.clone(),
        album: view.album.clone(),
        genre: view.genre.clone(),
        year: view.year,
        track_no: view.track_no,
        disc_no: view.disc_no,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: view.added_at,
        starred: false,
    }
}

#[cfg(test)]
mod tests;
