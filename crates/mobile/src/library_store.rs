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

/// The snapshot layout version. Bump it whenever a cached [`Track`] could be
/// missing data a fresh sync would fill in: delta sync only resends changed
/// rows, so without a bump such rows would stay incomplete forever.
///
/// - 1 (implicit, unversioned): rows could lack `filename`, so the app showed
///   the opaque 64-digit track id as the title.
/// - 2: every row comes from a sync that carries `filename` and `directory`.
pub const SCHEMA: u32 = 2;

/// The persisted library: the last synced version and its tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// The layout version this snapshot was written with ([`SCHEMA`]).
    #[serde(default)]
    pub schema: u32,
    /// The server library version this snapshot is at.
    #[serde(default)]
    pub version: i64,
    /// The tracks known at that version.
    #[serde(default)]
    pub tracks: Vec<Track>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            version: 0,
            tracks: Vec::new(),
        }
    }
}

/// The snapshot file path for a data directory.
fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(LIBRARY_FILE)
}

/// Loads the snapshot, or an empty one when none exists, it cannot be parsed or
/// it was written with an older [`SCHEMA`].
///
/// The snapshot is a disposable cache, so a corrupt or outdated file is treated
/// as "no snapshot" (the next refresh rebuilds it from version 0) rather than
/// an error that would block offline loading.
pub fn load(data_dir: &Path) -> Result<Snapshot, MobileError> {
    match std::fs::read_to_string(path(data_dir)) {
        Ok(text) => Ok(serde_json::from_str::<Snapshot>(&text)
            .ok()
            .filter(|snapshot| snapshot.schema == SCHEMA)
            .unwrap_or_default()),
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
    crate::util::replace_file(&temporary, &destination)?;
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
            ..Snapshot::default()
        };
        save(dir.path(), &snapshot).unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.version, 42);
    }

    #[test]
    fn an_unversioned_snapshot_is_discarded_for_a_full_resync() {
        let dir = tempfile::tempdir().unwrap();
        // The pre-schema layout: rows synced before `filename` was sent.
        std::fs::write(
            path(dir.path()),
            r#"{"version":7,"tracks":[{"id":"t","filename":null,"directory":"",
            "format":"mp3","kind":"stream","specialized":false,"title":null,
            "artist":null,"album_artist":null,"album":null,"album_id":null,
            "genre":null,"year":null,"track_no":null,"disc_no":null,
            "duration_secs":null,"subtunes":1,"channels":null,"file_size":0,
            "has_art":false,"sync_version":7,"added_at":0}]}"#,
        )
        .unwrap();
        let loaded = load(dir.path()).unwrap();
        assert_eq!(loaded.version, 0);
        assert!(loaded.tracks.is_empty());
        assert_eq!(loaded.schema, SCHEMA);
    }
}
