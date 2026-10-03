#![forbid(unsafe_code)]

//! The server's starred tracks (#513), shown read-only as a "Starred" playlist.
//!
//! Starring happens on the desktop; the phone only mirrors the set. It is kept
//! in a small local cache (`starred.json`) so the playlist shows offline and the
//! next fetch can be conditional on the cached version (`If-None-Match`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::MobileCore;
use crate::error::MobileError;
use crate::library_store;
use crate::types::Track;

/// The cache file name under the app data directory.
const STARRED_FILE: &str = "starred.json";

/// The cached starred set: the server's version and its ids, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct StarredCache {
    /// The starred-set version this cache is at (its ETag).
    #[serde(default)]
    pub version: i64,
    /// Starred track ids, newest first.
    #[serde(default)]
    pub ids: Vec<String>,
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(STARRED_FILE)
}

/// Loads the cache, or `None` when there is none or it cannot be parsed (a
/// disposable cache: the next fetch rebuilds it unconditionally).
pub(crate) fn load(data_dir: &Path) -> Result<Option<StarredCache>, MobileError> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Writes the cache atomically (temp file then rename).
pub(crate) fn save(data_dir: &Path, cache: &StarredCache) -> Result<(), MobileError> {
    std::fs::create_dir_all(data_dir)?;
    let encoded = serde_json::to_vec(cache).map_err(|error| MobileError::Client {
        message: error.to_string(),
    })?;
    let destination = path(data_dir);
    let temporary = destination.with_extension("json.tmp");
    std::fs::write(&temporary, &encoded)?;
    crate::util::replace_file(&temporary, &destination)?;
    Ok(())
}

/// `ids` (in order) restricted to tracks present in `tracks`, without
/// duplicates: a star on a track the phone has not synced yet is not shown.
pub(crate) fn filter_to_library(ids: &[String], tracks: &[Track]) -> Vec<String> {
    let known: HashSet<&str> = tracks.iter().map(|track| track.id.as_str()).collect();
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| known.contains(id.as_str()) && seen.insert(id.as_str()))
        .cloned()
        .collect()
}

impl MobileCore {
    /// The cached ids filtered to the local library snapshot.
    fn filtered(&self, cache: Option<StarredCache>) -> Result<Vec<String>, MobileError> {
        let Some(cache) = cache else {
            return Ok(Vec::new());
        };
        let snapshot = library_store::load(self.data_dir())?;
        Ok(filter_to_library(&cache.ids, &snapshot.tracks))
    }
}

#[uniffi::export]
impl MobileCore {
    /// The last fetched starred track ids, newest first, limited to tracks in
    /// the local library snapshot. No network; empty before the first fetch.
    pub fn cached_starred(&self) -> Result<Vec<String>, MobileError> {
        self.filtered(load(self.data_dir())?)
    }

    /// Fetches the server's starred set (conditional on the cached version),
    /// caches it, and returns its ids newest first, limited to tracks in the
    /// local library snapshot. Call it after [`MobileCore::refresh_library`] so
    /// newly synced tracks are not filtered out.
    pub fn refresh_starred(&self) -> Result<Vec<String>, MobileError> {
        let cached = load(self.data_dir())?;
        let (_, token) = self.fresh_token()?;
        let etag = cached.as_ref().map(|cache| cache.version);
        let current = match self.client().starred(&token, etag)? {
            // 304: unchanged since the cached version.
            None => cached,
            Some(set) => {
                let cache = StarredCache {
                    version: set.version,
                    ids: set.tracks.into_iter().map(|track| track.id).collect(),
                };
                save(self.data_dir(), &cache)?;
                Some(cache)
            }
        };
        self.filtered(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tests::track;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn filter_keeps_starred_order_and_drops_unknown_and_duplicate_ids() {
        let tracks = vec![track("a", "A"), track("b", "B"), track("c", "C")];
        let filtered = filter_to_library(&ids(&["c", "x", "a", "c"]), &tracks);
        assert_eq!(filtered, ids(&["c", "a"]));
        assert!(filter_to_library(&ids(&["x"]), &tracks).is_empty());
    }

    #[test]
    fn cache_round_trips_and_tolerates_garbage() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
        let cache = StarredCache {
            version: 7,
            ids: ids(&["b", "a"]),
        };
        save(dir.path(), &cache).unwrap();
        assert_eq!(load(dir.path()).unwrap(), Some(cache));
        std::fs::write(path(dir.path()), "not json").unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
    }
}
