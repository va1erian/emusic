#![forbid(unsafe_code)]

//! A side registry of the servers this app is configured with.
//!
//! `emusic-client` keys credentials by a hash of the endpoint URL, which cannot
//! be reversed back to a URL. The app still needs the URLs to re-open servers,
//! so the mobile crate keeps this small [`ServerEntry`] list alongside them.

use std::path::{Path, PathBuf};

use crate::error::MobileError;
use crate::types::ServerEntry;

/// Registry file name under the app data directory.
const REGISTRY_FILE: &str = "servers.json";

/// The registry file path for a data directory.
fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(REGISTRY_FILE)
}

/// Loads the registry, or an empty list when none exists yet.
pub fn load(data_dir: &Path) -> Result<Vec<ServerEntry>, MobileError> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| MobileError::Client {
            message: format!("cannot parse server registry: {error}"),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// Inserts or updates an entry, matched by [`ServerEntry::id`].
pub fn upsert(data_dir: &Path, entry: &ServerEntry) -> Result<(), MobileError> {
    let mut entries = load(data_dir)?;
    match entries.iter_mut().find(|existing| existing.id == entry.id) {
        Some(existing) => existing.clone_from(entry),
        None => entries.push(entry.clone()),
    }
    save(data_dir, &entries)
}

/// Removes an entry by id; a missing entry is not an error.
pub fn remove(data_dir: &Path, id: &str) -> Result<(), MobileError> {
    let mut entries = load(data_dir)?;
    let before = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == before {
        return Ok(());
    }
    save(data_dir, &entries)
}

/// Writes the registry atomically (temp file then rename).
fn save(data_dir: &Path, entries: &[ServerEntry]) -> Result<(), MobileError> {
    std::fs::create_dir_all(data_dir)?;
    let encoded = serde_json::to_vec_pretty(entries).map_err(|error| MobileError::Client {
        message: error.to_string(),
    })?;
    let destination = path(data_dir);
    let temporary = destination.with_extension("json.tmp");
    std::fs::write(&temporary, &encoded)?;
    if destination.exists() {
        std::fs::remove_file(&destination)?;
    }
    std::fs::rename(&temporary, &destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, url: &str) -> ServerEntry {
        ServerEntry {
            id: id.to_string(),
            name: url.to_string(),
            url: url.to_string(),
        }
    }

    #[test]
    fn missing_registry_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn upsert_updates_in_place_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        upsert(dir.path(), &entry("a", "https://a.example")).unwrap();
        upsert(dir.path(), &entry("b", "https://b.example")).unwrap();
        upsert(dir.path(), &entry("a", "https://a.example")).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "a");
        assert_eq!(loaded[1].id, "b");
    }

    #[test]
    fn remove_ignores_missing_entries() {
        let dir = tempfile::tempdir().unwrap();
        upsert(dir.path(), &entry("a", "https://a.example")).unwrap();
        remove(dir.path(), "b").unwrap();
        assert_eq!(load(dir.path()).unwrap().len(), 1);
        remove(dir.path(), "a").unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }
}
