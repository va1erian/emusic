//! Local cache of remote track files.
//!
//! Remote tracks are materialised as ordinary files so the existing audio
//! backend can open them unchanged. Paths are deterministic from the server id
//! and track id, so the playback layer can recognise a remote path from the
//! path alone (no in-memory map).

use std::path::{Path, PathBuf};

use crate::client::RemoteClient;
use crate::error::{ClientError, Result};
use crate::types::TrackView;

/// Environment override for the cache root (used by tests and the CLI).
pub const CACHE_ENV: &str = "EMUSIC_REMOTE_CACHE";

/// A directory holding downloaded remote tracks.
#[derive(Debug, Clone)]
pub struct TrackCache {
    root: PathBuf,
}

impl TrackCache {
    /// The default cache root: `EMUSIC_REMOTE_CACHE`, else the user's cache
    /// directory under `emusic/remote`.
    pub fn new() -> Result<Self> {
        if let Some(dir) = std::env::var_os(CACHE_ENV) {
            return Ok(Self::with_root(PathBuf::from(dir)));
        }
        let base = dirs::cache_dir()
            .or_else(dirs::data_local_dir)
            .ok_or_else(|| ClientError::Store("no user cache directory available".into()))?;
        Ok(Self::with_root(base.join("emusic").join("remote")))
    }

    /// A cache rooted at an explicit directory.
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// The cache root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The deterministic local path for a remote track.
    pub fn path_for(&self, server_id: &str, track: &TrackView) -> PathBuf {
        let ext = if track.format.trim().is_empty() {
            "bin"
        } else {
            track.format.as_str()
        };
        self.root
            .join(server_id)
            .join(format!("{}.{ext}", track.id))
    }

    /// Downloads `track` if not already cached, returning its local path.
    ///
    /// The download is written to a temporary file and renamed into place, so
    /// a crashed or interrupted fetch never leaves a partial file the audio
    /// backend might open.
    pub fn ensure(
        &self,
        client: &RemoteClient,
        token: &str,
        server_id: &str,
        track: &TrackView,
    ) -> Result<PathBuf> {
        let destination = self.path_for(server_id, track);
        if self.is_cached(&destination, track) {
            return Ok(destination);
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = destination.with_extension("part");

        let result = (|| -> Result<u64> {
            let mut file = std::fs::File::create(&temporary)?;
            let copied = client.download_to(token, &track.id, &mut file)?;
            file.sync_all()?;
            Ok(copied)
        })();
        let copied = match result {
            Ok(copied) => copied,
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(error);
            }
        };
        if track.file_size > 0 && copied != track.file_size {
            let _ = std::fs::remove_file(&temporary);
            return Err(ClientError::Protocol(format!(
                "download size mismatch for {}: expected {}, got {copied}",
                track.id, track.file_size
            )));
        }
        if destination.exists() {
            std::fs::remove_file(&destination)?;
        }
        std::fs::rename(&temporary, &destination)?;
        Ok(destination)
    }

    /// Whether `path` already holds the complete track.
    fn is_cached(&self, path: &Path, track: &TrackView) -> bool {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                metadata.is_file() && (track.file_size == 0 || metadata.len() == track.file_size)
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, format: &str) -> TrackView {
        TrackView {
            id: id.into(),
            format: format.into(),
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
            hash: String::new(),
            has_art: false,
            sync_version: 0,
            added_at: 0,
        }
    }

    #[test]
    fn paths_are_deterministic_and_namespaced() {
        let cache = TrackCache::with_root(PathBuf::from("/cache"));
        let a = cache.path_for("srv1", &track("abc", "flac"));
        assert_eq!(a, cache.path_for("srv1", &track("abc", "flac")));
        assert_ne!(a, cache.path_for("srv2", &track("abc", "flac")));
        assert!(a.to_string_lossy().ends_with("abc.flac"));
    }

    #[test]
    fn missing_extension_falls_back_to_bin() {
        let cache = TrackCache::with_root(PathBuf::from("/cache"));
        assert!(
            cache
                .path_for("srv", &track("x", ""))
                .to_string_lossy()
                .ends_with("x.bin")
        );
    }
}
