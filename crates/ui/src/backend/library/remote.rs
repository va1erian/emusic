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
use tracing::{info, warn};

use super::source::Snapshot;
use super::{Updates, private_store};
use crate::remote::RemoteServer;

/// Remote-sync state owned by [`LibraryBackend`](super::LibraryBackend).
pub(crate) struct RemoteState {
    servers: Vec<RemoteServer>,
    cache: Option<TrackCache>,
    credentials: Option<CredentialStore>,
    running: Arc<AtomicBool>,
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

    /// Starts a sync on a background thread unless one is already running.
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
        if self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(cache) = self.cache.clone() else {
            warn!("remote cache unavailable; skipping sync");
            self.running.store(false, Ordering::Release);
            return;
        };
        let Some(credentials) = self.credentials.clone() else {
            warn!("credential store unavailable; skipping remote sync");
            self.running.store(false, Ordering::Release);
            return;
        };
        let servers = self.servers.clone();
        let running = Arc::clone(&self.running);
        std::thread::spawn(move || {
            let _ = updates.send(super::Update::Status("Syncing remote servers...".into()));
            match run(
                &store,
                &folders,
                &servers,
                &cache,
                &credentials,
                &updates,
                only_root.as_deref(),
            ) {
                Ok(()) => info!("remote sync finished"),
                Err(error) => {
                    warn!(%error, "remote sync failed");
                    let _ = updates.send(super::Update::Status(format!(
                        "Remote sync failed: {error}"
                    )));
                }
            }
            running.store(false, Ordering::Release);
        });
    }
}

/// Syncs every server, then posts one refreshed snapshot.
fn run(
    store: &Arc<Mutex<Store>>,
    folders: &[Folder],
    servers: &[RemoteServer],
    cache: &TrackCache,
    credentials: &CredentialStore,
    updates: &Updates,
    only_root: Option<&Path>,
) -> Result<()> {
    let mut total = 0usize;
    let mut failures = 0usize;
    for server in servers {
        match sync_server(store, server, cache, credentials) {
            Ok(count) => total += count,
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

    let message = if failures == 0 {
        format!("Remote sync: {total} track(s)")
    } else {
        format!("Remote sync: {total} track(s), {failures} server(s) failed")
    };
    let _ = updates.send(super::Update::Status(message));
    Ok(())
}

/// Syncs one server, returning the number of tracks in its delta.
fn sync_server(
    store: &Arc<Mutex<Store>>,
    server: &RemoteServer,
    cache: &TrackCache,
    credentials: &CredentialStore,
) -> Result<usize> {
    let endpoint = server.endpoint();
    let client = RemoteClient::from_endpoint(&endpoint)?;
    let token = crate::backend::remote_auth::ensure_token(&client, credentials, &server.id)
        .map_err(anyhow::Error::msg)?;

    // Use a private connection for the writes so a slow sync never holds the
    // UI's store lock; fall back to the shared lock when there is no file.
    match private_store(store) {
        Some(mut store) => apply_delta(&mut store, &client, &token, server, cache),
        None => {
            let mut guard = store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            apply_delta(&mut guard, &client, &token, server, cache)
        }
    }
}

/// Pulls the delta for `server` and applies it to `store`.
fn apply_delta(
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
mod tests {
    use super::*;

    fn view() -> TrackView {
        TrackView {
            id: "abc".into(),
            filename: Some("song.xm".into()),
            format: "xm".into(),
            kind: "module".into(),
            specialized: true,
            title: Some("Song".into()),
            artist: Some("Artist".into()),
            album_artist: None,
            album: Some("Album".into()),
            album_id: None,
            genre: None,
            year: Some(1994),
            track_no: Some(3),
            disc_no: None,
            duration_secs: Some(125.5),
            subtunes: 1,
            channels: Some(8),
            file_size: 4096,
            hash: "h".into(),
            has_art: false,
            sync_version: 7,
            added_at: 1_600_000_000,
        }
    }

    #[test]
    fn maps_a_view_to_a_track() {
        let path = PathBuf::from(r"C:\cache\srv\abc.xm");
        let track = track_from_view(&view(), path.clone());
        assert_eq!(track.path, path);
        assert_eq!(track.filename, "abc.xm");
        assert_eq!(track.ext, "xm");
        assert_eq!(track.kind, TrackKind::Module);
        assert_eq!(track.duration_ms, 125_500);
        assert_eq!(track.channels, Some(8));
        assert_eq!(track.size, 4096);
        assert_eq!(track.title.as_deref(), Some("Song"));
    }

    #[test]
    fn falls_back_to_the_file_name_without_a_title() {
        let mut view = view();
        view.title = None;
        let track = track_from_view(&view, PathBuf::from(r"C:\cache\srv\abc.xm"));
        assert_eq!(track.title.as_deref(), Some("song.xm"));
    }

    fn set(items: &[&str]) -> std::collections::HashSet<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn incremental_sync_keeps_unchanged_rows() {
        // Existing {a, b}, delta contains only the changed a, no tombstones:
        // b is unchanged and must be kept, not deleted.
        let removed = ids_to_delete(5, set(&["a", "b"]), &set(&["a"]), &[]);
        assert!(removed.is_empty(), "unchanged rows must survive a delta");
    }

    #[test]
    fn incremental_sync_applies_tombstones() {
        let removed = ids_to_delete(5, set(&["a", "b"]), &set(&["a"]), &["b".to_string()]);
        assert_eq!(removed, vec!["b".to_string()]);
    }

    #[test]
    fn full_sync_drops_absent_rows() {
        let removed = ids_to_delete(0, set(&["a", "b", "c"]), &set(&["a"]), &[]);
        let mut removed = removed;
        removed.sort();
        assert_eq!(removed, vec!["b".to_string(), "c".to_string()]);
    }
}
