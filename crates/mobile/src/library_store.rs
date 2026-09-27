#![forbid(unsafe_code)]

//! A local snapshot of the synced library.
//!
//! It lets the app show the library instantly and offline, then apply the next
//! delta sync on top. The whole library is a few megabytes of JSON, which is
//! cheap to read and avoids a second schema to migrate on the client.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MobileError;
use crate::types::Track;

/// The snapshot file name under the app data directory.
const LIBRARY_FILE: &str = "library.json";

/// The persisted library: the last synced version and its tracks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// The server library version this snapshot is at.
    #[serde(default)]
    pub version: i64,
    /// The tracks known at that version.
    #[serde(default)]
    pub tracks: Vec<Track>,
}

/// The snapshot file path for a data directory.
fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(LIBRARY_FILE)
}

/// Loads the snapshot, or an empty one when none exists yet.
pub fn load(data_dir: &Path) -> Result<Snapshot, MobileError> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| MobileError::Client {
            message: format!("cannot parse library snapshot: {error}"),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Snapshot::default()),
        Err(error) => Err(error.into()),
    }
}

/// Writes the snapshot atomically (temp file then rename).
pub fn save(data_dir: &Path, snapshot: &Snapshot) -> Result<(), MobileError> {
    std::fs::create_dir_all(data_dir)?;
    let encoded = serde_json::to_vec(snapshot).map_err(|error| MobileError::Client {
        message: error.to_string(),
    })?;
    let destination = path(data_dir);
    let temporary = destination.with_extension("json.tmp");
    std::fs::write(&temporary, &encoded)?;
    if destination.exists() {
        let _ = std::fs::remove_file(&destination);
    }
    std::fs::rename(&temporary, &destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_snapshot_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = load(dir.path()).unwrap();
        assert_eq!(snapshot.version, 0);
        assert!(snapshot.tracks.is_empty());
    }

    #[test]
    fn round_trips_a_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = Snapshot {
            version: 42,
            tracks: Vec::new(),
        };
        save(dir.path(), &snapshot).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.version, 42);
    }
}
