//! Plain data records crossing the uniffi boundary.
//!
//! They mirror the subset of the server API the phone app consumes; the raw
//! `emusic_client` types are not exposed directly so their layout can evolve
//! without breaking the generated Kotlin.

use emusic_client::TrackView;

/// A track as synced from the server.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct Track {
    /// Opaque track identifier.
    pub id: String,
    /// File name (final path component), for display fallback.
    pub filename: Option<String>,
    /// The parent directory's relative path, `/`-separated (empty at the root).
    pub directory: String,
    /// Lowercase format label (`flac`, `mp3`, `sid`, `xm`, ...).
    pub format: String,
    /// Either `stream` or `module`.
    pub kind: String,
    /// Whether the raw file must be fetched whole and rendered client-side.
    pub specialized: bool,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Opaque album identifier, for artwork.
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
    pub subtunes: u32,
    /// Channel count, when known.
    pub channels: Option<u32>,
    /// File size in bytes.
    pub file_size: u64,
    /// Whether artwork is available.
    pub has_art: bool,
    /// Library version at which this row last changed.
    pub sync_version: i64,
    /// Unix timestamp (seconds) when the track was first seen.
    pub added_at: i64,
}

impl From<TrackView> for Track {
    fn from(track: TrackView) -> Self {
        Self {
            id: track.id,
            filename: track.filename,
            directory: track.directory,
            format: track.format,
            kind: track.kind,
            specialized: track.specialized,
            title: track.title,
            artist: track.artist,
            album_artist: track.album_artist,
            album: track.album,
            album_id: track.album_id,
            genre: track.genre,
            year: track.year,
            track_no: track.track_no,
            disc_no: track.disc_no,
            duration_secs: track.duration_secs,
            subtunes: track.subtunes,
            channels: track.channels,
            file_size: track.file_size,
            has_art: track.has_art,
            sync_version: track.sync_version,
            added_at: track.added_at,
        }
    }
}

impl Track {
    /// Title to display: the tagged title, else the file name without its
    /// extension, else [`UNTITLED`]. Never the opaque id, which is a 64-digit
    /// hash and meaningless to a person.
    pub fn display_title(&self) -> String {
        if let Some(title) = self.title.as_deref().map(str::trim)
            && !title.is_empty()
        {
            return title.to_string();
        }
        self.filename
            .as_deref()
            .map(file_stem)
            .filter(|stem| !stem.is_empty())
            .unwrap_or(UNTITLED)
            .to_string()
    }
}

/// The title shown for a track with neither a title tag nor a file name.
pub const UNTITLED: &str = "Untitled";

/// `name` without its final extension (`a.b.flac` -> `a.b`), trimmed. A
/// leading dot (`.hidden`) is part of the name, not an extension.
fn file_stem(name: &str) -> &str {
    let name = name.trim();
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].trim_end(),
        _ => name,
    }
}

/// A delta-sync batch from `GET /library/sync`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SyncResult {
    /// The server's library version after this batch.
    pub version: i64,
    /// Added or updated tracks.
    pub tracks: Vec<Track>,
    /// Identifiers deleted since the last sync.
    pub deleted: Vec<String>,
}

/// Local pairing state for a server.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AuthState {
    /// Normalized base URL.
    pub base_url: String,
    /// Whether a device is paired with this server.
    pub paired: bool,
    /// Server-assigned device id, when paired.
    pub device_id: Option<String>,
    /// Device name, when paired.
    pub device_name: Option<String>,
    /// Access-token expiry (Unix seconds), when paired.
    pub expires_at: Option<i64>,
    /// Last synced library version, when paired.
    pub since_version: Option<i64>,
}

/// Response of `GET /health`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Health {
    /// Always `ok` while the server is serving.
    pub status: String,
    /// Server start time (Unix seconds).
    pub started_at: i64,
}

/// A server this app has been configured with.
///
/// Credentials are stored per hashed endpoint id, which is not reversible to a
/// URL; this side record keeps the URL and display name so the app can list and
/// re-open servers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct ServerEntry {
    /// Stable id derived from the URL (`ServerEndpoint::id`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Normalized base URL.
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_title_falls_back_to_the_file_stem_never_the_id() {
        let mut track = Track {
            id: "0e0a2e72050fc119033f89f75829941e".into(),
            filename: Some("AMAZONS - KISS IN THE DARK.mp3".into()),
            directory: String::new(),
            format: "mp3".into(),
            kind: "stream".into(),
            specialized: false,
            title: None,
            artist: None,
            album_artist: None,
            album: None,
            album_id: None,
            genre: None,
            year: None,
            track_no: None,
            disc_no: None,
            duration_secs: None,
            subtunes: 1,
            channels: None,
            file_size: 0,
            has_art: false,
            sync_version: 0,
            added_at: 0,
        };
        assert_eq!(track.display_title(), "AMAZONS - KISS IN THE DARK");
        track.filename = Some("v1.2 mix.flac".into());
        assert_eq!(track.display_title(), "v1.2 mix");
        track.filename = Some(".hidden".into());
        assert_eq!(track.display_title(), ".hidden");
        track.filename = None;
        assert_eq!(track.display_title(), UNTITLED);
        track.title = Some("  ".into());
        assert_eq!(track.display_title(), UNTITLED);
        track.title = Some(" Real ".into());
        assert_eq!(track.display_title(), "Real");
    }
}
