//! Persisted row models shared between the database and the REST API.

use serde::{Deserialize, Serialize};

/// A paired client device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Server-assigned opaque identifier (UUID v4).
    pub id: String,
    /// Human-readable device name supplied at pairing time.
    pub name: String,
    /// PASERK-encoded Ed25519 public key proving possession at refresh.
    pub public_key: String,
    /// Unix timestamp (seconds) when the device paired.
    pub paired_at: i64,
    /// Unix timestamp (seconds) of the device's last authenticated request.
    pub last_seen: Option<i64>,
    /// Whether administrator revocation has invalidated the device.
    pub is_revoked: bool,
}

/// One scanned track as stored in the `tracks` table.
///
/// The identifier is the SHA-256 of the file's canonical absolute path, which
/// keeps it stable across scans and opaque to clients (the absolute host path
/// is never disclosed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackRecord {
    /// Opaque track identifier.
    pub id: String,
    /// Index into the configured library roots.
    pub root_index: i64,
    /// Path relative to the root, using forward slashes.
    pub relative_path: String,
    /// Lowercase format label (`flac`, `mp3`, `sid`, `xm`, `mid`, ...).
    pub format: String,
    /// Either `stream` or `module`.
    pub kind: String,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Opaque album identifier derived from album artist and album name.
    pub album_id: Option<String>,
    /// Tagged genre.
    pub genre: Option<String>,
    /// Tagged year.
    pub year: Option<i32>,
    /// Tagged track number.
    pub track_no: Option<u32>,
    /// Tagged disc number.
    pub disc_no: Option<u32>,
    /// Duration in seconds, when known.
    pub duration_secs: Option<f64>,
    /// Number of subtunes (SID files; `1` otherwise).
    pub subtunes: u32,
    /// Channel count, when known.
    pub channels: Option<u32>,
    /// File size in bytes.
    pub file_size: u64,
    /// File modification time as a Unix timestamp (seconds).
    pub mtime: i64,
    /// Cheap change fingerprint (SHA-256 over size and mtime).
    pub hash: String,
    /// Whether artwork is available for this track.
    pub has_art: bool,
    /// Monotonic version at which this row last changed.
    pub sync_version: i64,
    /// Unix timestamp (seconds) when the track was first seen.
    pub added_at: i64,
}

/// A track as produced by a scan, ready to be inserted or updated.
///
/// The `sync_version` and `added_at` bookkeeping is owned by the store, not by
/// the scanner.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTrack {
    /// Opaque identifier (SHA-256 of the canonical absolute path).
    pub id: String,
    /// Index into the configured library roots.
    pub root_index: i64,
    /// Path relative to the root, using forward slashes.
    pub relative_path: String,
    /// Lowercase format label.
    pub format: String,
    /// Either `stream` or `module`.
    pub kind: String,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Opaque album identifier derived from album artist and album name.
    pub album_id: Option<String>,
    /// Tagged genre.
    pub genre: Option<String>,
    /// Tagged year.
    pub year: Option<i32>,
    /// Tagged track number.
    pub track_no: Option<u32>,
    /// Tagged disc number.
    pub disc_no: Option<u32>,
    /// Duration in seconds, when known.
    pub duration_secs: Option<f64>,
    /// Number of subtunes.
    pub subtunes: u32,
    /// Channel count, when known.
    pub channels: Option<u32>,
    /// File size in bytes.
    pub file_size: u64,
    /// File modification time (Unix seconds).
    pub mtime: i64,
    /// Cheap change fingerprint.
    pub hash: String,
    /// Whether artwork is available.
    pub has_art: bool,
    /// Unix timestamp (seconds) when the track was first seen.
    pub added_at: i64,
}

/// A batch of changes since a client's known version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncDelta {
    /// The current library version after this batch.
    pub version: i64,
    /// Added or updated tracks.
    pub tracks: Vec<TrackRecord>,
    /// Identifiers of tracks deleted since the requested version.
    pub deleted: Vec<String>,
}
