//! Remote library provider and synchronization client.

use emusic_core::{ArtSource, Track, TrackId, TrackKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{LibraryError, Result};
use crate::store::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteTrackMeta {
    pub id: String,
    pub relative_path: String,
    pub format: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub subtunes: u32,
    pub file_size: u64,
    pub mtime: i64,
    pub hash: String,
}

pub struct RemoteLibraryProvider {
    pub server_url: String,
    pub auth_token: String,
}

impl RemoteLibraryProvider {
    pub fn new(server_url: impl Into<String>, auth_token: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim_end_matches('/').to_string(),
            auth_token: auth_token.into(),
        }
    }

    /// Fetches delta tracks from `GET /api/v1/library/sync?since_version=since` and upserts them into `store`.
    pub fn sync_into_store(&self, store: &mut Store, since_version: i64) -> Result<usize> {
        let url = format!(
            "{}/api/v1/library/sync?since_version={since_version}",
            self.server_url
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| LibraryError::RemoteSync(e.to_string()))?;

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .map_err(|e| LibraryError::RemoteSync(e.to_string()))?;

        if !response.status().is_success() {
            return Err(LibraryError::RemoteSync(format!(
                "Server HTTP {}",
                response.status()
            )));
        }

        let remote_tracks: Vec<RemoteTrackMeta> = response
            .json()
            .map_err(|e| LibraryError::RemoteSync(e.to_string()))?;

        let count = remote_tracks.len();
        let mut domain_tracks = Vec::with_capacity(count);

        for meta in remote_tracks {
            let stream_url = format!("{}/api/v1/tracks/{}/stream", self.server_url, meta.id);
            let filename = std::path::Path::new(&meta.relative_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| meta.id.clone());

            let is_module = matches!(
                meta.format.to_lowercase().as_str(),
                "sid" | "psid" | "rsid" | "mod" | "s3m" | "xm" | "it" | "mo3" | "mid" | "midi"
            );

            let kind = if is_module {
                TrackKind::Module
            } else {
                TrackKind::Stream
            };

            let duration_ms = meta.duration_secs.map(|s| (s * 1000.0) as u32).unwrap_or(0);

            let track = Track {
                id: TrackId::UNASSIGNED,
                path: PathBuf::from(&stream_url),
                dir: PathBuf::from(&self.server_url),
                filename,
                ext: meta.format,
                size: meta.file_size,
                mtime: meta.mtime,
                kind,
                duration_ms,
                bitrate: None,
                sample_rate: None,
                channels: None,
                title: meta.title,
                artist: meta.artist,
                album_artist: None,
                album: meta.album,
                genre: None,
                year: None,
                track_no: None,
                disc_no: None,
                composer: None,
                comment: None,
                art_source: ArtSource::None,
                added_at: meta.mtime,
                starred: false,
            };
            domain_tracks.push(track);
        }

        if !domain_tracks.is_empty() {
            store.upsert_tracks(&mut domain_tracks)?;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_provider_construction() {
        let provider = RemoteLibraryProvider::new("http://localhost:8080/", "token123");
        assert_eq!(provider.server_url, "http://localhost:8080");
        assert_eq!(provider.auth_token, "token123");
    }
}
