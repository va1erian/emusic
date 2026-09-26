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
    /// Title to display, falling back to the file name derived from the id.
    pub fn display_title(&self) -> String {
        self.title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
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

/// The server's JSON error body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Human-readable error.
    pub error: String,
}
