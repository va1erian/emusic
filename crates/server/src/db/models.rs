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

/// Track metadata as sent to clients: everything except the internal
/// `root_index` and `relative_path`, which are not disclosed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackView {
    /// Opaque track identifier.
    pub id: String,
    /// Lowercase format label.
    pub format: String,
    /// Either `stream` or `module`.
    pub kind: String,
    /// Whether the client should fetch the whole file and render it natively
    /// (SID, tracker module or MIDI) rather than stream it for seeking.
    pub specialized: bool,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Opaque album identifier.
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
    /// Cheap change fingerprint.
    pub hash: String,
    /// Whether artwork is available.
    pub has_art: bool,
    /// Monotonic version at which this row last changed.
    pub sync_version: i64,
    /// Unix timestamp (seconds) when the track was first seen.
    pub added_at: i64,
}

impl From<&TrackRecord> for TrackView {
    fn from(track: &TrackRecord) -> Self {
        Self {
            id: track.id.clone(),
            format: track.format.clone(),
            kind: track.kind.clone(),
            specialized: is_specialized_format(&track.format),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album_artist: track.album_artist.clone(),
            album: track.album.clone(),
            album_id: track.album_id.clone(),
            genre: track.genre.clone(),
            year: track.year,
            track_no: track.track_no,
            disc_no: track.disc_no,
            duration_secs: track.duration_secs,
            subtunes: track.subtunes,
            channels: track.channels,
            file_size: track.file_size,
            hash: track.hash.clone(),
            has_art: track.has_art,
            sync_version: track.sync_version,
            added_at: track.added_at,
        }
    }
}

/// A sync batch as sent to clients (no internal path fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncDeltaView {
    /// The current library version after this batch.
    pub version: i64,
    /// Added or updated tracks.
    pub tracks: Vec<TrackView>,
    /// Identifiers of tracks deleted since the requested version.
    pub deleted: Vec<String>,
}

impl From<SyncDelta> for SyncDeltaView {
    fn from(delta: SyncDelta) -> Self {
        Self {
            version: delta.version,
            tracks: delta.tracks.iter().map(TrackView::from).collect(),
            deleted: delta.deleted,
        }
    }
}

/// Whether a format label must be fetched whole and rendered by the client.
fn is_specialized_format(format: &str) -> bool {
    matches!(
        format,
        "sid"
            | "psid"
            | "rsid"
            | "mid"
            | "midi"
            | "mod"
            | "xm"
            | "it"
            | "s3m"
            | "mo3"
            | "mtm"
            | "umx"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialized_formats_are_flagged() {
        assert!(is_specialized_format("sid"));
        assert!(is_specialized_format("xm"));
        assert!(is_specialized_format("mid"));
        assert!(!is_specialized_format("flac"));
        assert!(!is_specialized_format("mp3"));
    }
}
