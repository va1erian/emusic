//! Server response types, mirroring `emusic-server`'s API.
//!
//! Only the fields clients consume are declared; unknown fields are ignored so
//! a newer server can add data without breaking older clients.

use serde::{Deserialize, Serialize};

/// A track as sent by `GET /api/v1/library/sync` and `GET /tracks/{id}/meta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackView {
    /// Opaque track identifier.
    pub id: String,
    /// The file's name (final path component), for display fallback.
    #[serde(default)]
    pub filename: Option<String>,
    /// The parent directory's relative path, `/`-separated with no leading or
    /// trailing slash (empty at the library root). Absent from older servers.
    #[serde(default)]
    pub directory: String,
    /// Lowercase format label (`flac`, `mp3`, `sid`, `xm`, `mid`, ...).
    pub format: String,
    /// Either `stream` or `module`.
    pub kind: String,
    /// Whether the client must fetch the whole file and render it natively.
    #[serde(default)]
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
    /// Number of subtunes (SID files).
    #[serde(default = "one")]
    pub subtunes: u32,
    /// Channel count, when known.
    pub channels: Option<u32>,
    /// File size in bytes.
    pub file_size: u64,
    /// Cheap change fingerprint.
    #[serde(default)]
    pub hash: String,
    /// Whether artwork is available.
    #[serde(default)]
    pub has_art: bool,
    /// Version at which this row last changed.
    pub sync_version: i64,
    /// Unix timestamp (seconds) when the track was first seen.
    pub added_at: i64,
}

fn one() -> u32 {
    1
}

impl TrackView {
    /// Title to display: the tagged title, else the file name, else the id.
    pub fn display_title(&self) -> String {
        self.title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| {
                self.filename
                    .as_deref()
                    .filter(|name| !name.trim().is_empty())
            })
            .unwrap_or(&self.id)
            .to_string()
    }
}

/// A delta-sync batch from `GET /api/v1/library/sync`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncDelta {
    /// The server's library version after this batch.
    pub version: i64,
    /// Added or updated tracks.
    #[serde(default)]
    pub tracks: Vec<TrackView>,
    /// Identifiers deleted since the requested version.
    #[serde(default)]
    pub deleted: Vec<String>,
}

/// Response of `POST /api/v1/auth/pair`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairResponse {
    /// Server-assigned device id.
    pub device_id: String,
    /// Device name as registered.
    pub device_name: String,
    /// First access token.
    pub auth_token: String,
    /// Token expiry (Unix seconds).
    pub expires_at: i64,
}

/// Response of `POST /api/v1/auth/refresh`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenResponse {
    /// The renewed access token.
    pub auth_token: String,
    /// Expiry (Unix seconds).
    pub expires_at: i64,
}

/// Response of `GET /api/v1/health`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    /// Always `ok` when the server is serving.
    pub status: String,
    /// Server start time (Unix seconds).
    #[serde(default)]
    pub started_at: i64,
}

/// The server-wide starred set from `GET /api/v1/starred`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarredSet {
    /// Starred-set version; pass it back as the ETag to skip unchanged sets.
    pub version: i64,
    /// Starred tracks that currently exist on the server, newest first.
    #[serde(default)]
    pub tracks: Vec<StarredTrack>,
}

/// One starred track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarredTrack {
    /// Server track id.
    pub id: String,
    /// Unix timestamp (seconds) when it was starred.
    pub starred_at: i64,
}

/// Response of `PUT`/`DELETE /api/v1/starred/{track_id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarredVersion {
    /// Starred-set version after the request.
    pub version: i64,
}

/// Response of `POST /api/v1/starred/batch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarredBatchResult {
    /// Starred-set version after the batch.
    pub version: i64,
    /// Ids asked to be starred that the server does not know (skipped).
    #[serde(default)]
    pub unknown: Vec<String>,
}

/// An event pushed on the server's WebSocket (`GET /api/v1/ws`).
///
/// Only the events clients act on are modelled; anything else (scan progress,
/// status snapshots, future events) decodes as [`ServerEvent::Other`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// The library version advanced: run a delta sync.
    LibraryChanged {
        /// New library version.
        version: i64,
    },
    /// The starred set changed: re-fetch `GET /api/v1/starred`.
    StarredChanged {
        /// New starred-set version.
        version: i64,
    },
    /// Any other event.
    #[serde(other)]
    Other,
}

/// The server's JSON error body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Human-readable error.
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starred_types_match_the_server_wire_format() {
        let set: StarredSet = serde_json::from_str(
            r#"{"version":4,"tracks":[{"id":"b","starred_at":20},{"id":"a","starred_at":10}]}"#,
        )
        .unwrap();
        assert_eq!(set.version, 4);
        assert_eq!(set.tracks[0].id, "b");
        assert_eq!(set.tracks[1].starred_at, 10);
        let batch: StarredBatchResult =
            serde_json::from_str(r#"{"version":5,"unknown":["x"]}"#).unwrap();
        assert_eq!(batch.unknown, vec!["x"]);
    }

    #[test]
    fn server_events_decode_known_and_unknown_types() {
        let event: ServerEvent =
            serde_json::from_str(r#"{"type":"starred_changed","version":3}"#).unwrap();
        assert_eq!(event, ServerEvent::StarredChanged { version: 3 });
        let event: ServerEvent =
            serde_json::from_str(r#"{"type":"library_changed","version":9}"#).unwrap();
        assert_eq!(event, ServerEvent::LibraryChanged { version: 9 });
        let event: ServerEvent =
            serde_json::from_str(r#"{"type":"scan_progress","root_index":0,"files_found":1}"#)
                .unwrap();
        assert_eq!(event, ServerEvent::Other);
    }
}
