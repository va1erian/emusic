//! Playlists for the real library backend (#473): a cache of the store's
//! playlists that views read every frame, and the write-through helpers.

use std::collections::HashMap;

use emusic_library::{PlaylistId, Store, TrackId};
use tracing::warn;

use super::LibraryBackend;
use crate::library_api::{PlaylistInfo, PlaylistItem};

/// In-memory mirror of the `playlists` and `playlist_tracks` tables.
#[derive(Default)]
pub(crate) struct PlaylistCache {
    infos: Vec<PlaylistInfo>,
    items: HashMap<u64, Vec<PlaylistItem>>,
}

impl PlaylistCache {
    /// Reads every playlist from `store`; an unreadable store yields an
    /// empty cache (logged).
    pub(crate) fn load(store: &Store) -> Self {
        match Self::try_load(store) {
            Ok(cache) => cache,
            Err(err) => {
                warn!(%err, "failed to load playlists");
                Self::default()
            }
        }
    }

    fn try_load(store: &Store) -> emusic_library::Result<Self> {
        let mut cache = Self::default();
        for playlist in store.playlists()? {
            let id = playlist.id.0 as u64;
            let items: Vec<PlaylistItem> = store
                .playlist_entries(playlist.id)?
                .into_iter()
                .map(|entry| PlaylistItem {
                    entry_id: entry.id,
                    track_id: entry.track_id.0 as u64,
                })
                .collect();
            cache.infos.push(PlaylistInfo {
                id,
                name: playlist.name,
                track_count: items.len(),
            });
            cache.items.insert(id, items);
        }
        Ok(cache)
    }

    pub(crate) fn infos(&self) -> &[PlaylistInfo] {
        &self.infos
    }

    pub(crate) fn items(&self, id: u64) -> &[PlaylistItem] {
        self.items.get(&id).map_or(&[], Vec::as_slice)
    }
}

impl LibraryBackend {
    /// Runs a playlist write against the store, then reloads the cache and
    /// bumps the revision. A failure is logged and leaves the cache in sync
    /// with whatever the store now holds.
    pub(super) fn write_playlists<T>(
        &mut self,
        what: &str,
        write: impl FnOnce(&Store) -> emusic_library::Result<T>,
    ) -> Option<T> {
        let (result, cache) = {
            let store = self
                .store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let result = write(&store);
            (result, PlaylistCache::load(&store))
        };
        self.playlists = cache;
        self.mark_changed();
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                warn!(%err, "failed to {what}");
                None
            }
        }
    }

    /// Reloads the playlist cache, e.g. after a scan removed tracks (their
    /// entries cascade away in the store).
    pub(super) fn reload_playlists(&mut self) {
        let cache = {
            let store = self
                .store
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            PlaylistCache::load(&store)
        };
        self.playlists = cache;
    }
}

pub(super) fn playlist_id(id: u64) -> PlaylistId {
    PlaylistId(id as i64)
}

pub(super) fn track_ids(ids: &[u64]) -> Vec<TrackId> {
    ids.iter().map(|&id| TrackId(id as i64)).collect()
}

#[cfg(test)]
mod tests;
