//! Local cache of remote track files.
//!
//! Remote tracks are materialised as ordinary files so the existing audio
//! backend can open them unchanged. Paths are deterministic from the server id
//! and track id, so the playback layer can recognise a remote path from the
//! path alone (no in-memory map).
//!
//! Track ids and formats come from the server and are treated as untrusted:
//! they are validated to a safe filename alphabet before any path is built, so
//! a malicious or buggy server cannot escape the cache root.

use std::path::{Component, Path, PathBuf};

use crate::client::RemoteClient;
use crate::error::{ClientError, Result};
use crate::types::TrackView;

/// Environment override for the cache root (used by tests and the CLI).
pub const CACHE_ENV: &str = "EMUSIC_REMOTE_CACHE";

/// Longest accepted format label.
const MAX_FORMAT_LEN: usize = 16;

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
    ///
    /// Fails for a server id, track id or format that could escape the cache
    /// root (`..`, path separators, absolute/UNC paths, NUL).
    pub fn path_for(&self, server_id: &str, track: &TrackView) -> Result<PathBuf> {
        let server = safe_id(server_id)
            .ok_or_else(|| ClientError::Protocol(format!("unsafe server id {server_id:?}")))?;
        let id = safe_id(&track.id)
            .ok_or_else(|| ClientError::Protocol(format!("unsafe track id {:?}", track.id)))?;
        let extension = safe_extension(&track.format);
        let path = self.root.join(server).join(format!("{id}.{extension}"));
        ensure_inside(&self.root, &path)
    }

    /// Downloads `track` if not already cached, returning its local path.
    ///
    /// The download is written to a unique temporary file and renamed into
    /// place, so a crashed or interrupted fetch never leaves a partial file
    /// the audio backend might open, and concurrent fetches of the same track
    /// do not corrupt each other.
    pub fn ensure(
        &self,
        client: &RemoteClient,
        token: &str,
        server_id: &str,
        track: &TrackView,
    ) -> Result<PathBuf> {
        let destination = self.path_for(server_id, track)?;
        if is_complete(&destination, track) {
            return Ok(destination);
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = unique_temp(&destination);

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
        replace_file(&temporary, &destination)?;
        Ok(destination)
    }
}

/// Whether `path` already holds the complete track.
///
/// A zero `file_size` gives no way to tell a complete file from a truncated
/// one, so such tracks are always re-fetched.
fn is_complete(path: &Path, track: &TrackView) -> bool {
    track.file_size > 0
        && std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == track.file_size)
}

/// A safe single path component made from `track.id`/`track.format`, for
/// callers that build their own destination (the CLI).
pub fn safe_file_name(track: &TrackView) -> Result<String> {
    let id = safe_id(&track.id)
        .ok_or_else(|| ClientError::Protocol(format!("unsafe track id {:?}", track.id)))?;
    Ok(format!("{id}.{}", safe_extension(&track.format)))
}

/// Accepts a component made only of `[A-Za-z0-9._-]` that is not a dot-path.
pub fn safe_id(value: &str) -> Option<&str> {
    if value.is_empty() || value == "." || value == ".." || value.len() > 128 {
        return None;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        .then_some(value)
}

/// Maps a format label to a safe extension, falling back to `bin`.
fn safe_extension(format: &str) -> String {
    if !format.is_empty()
        && format.len() <= MAX_FORMAT_LEN
        && format.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        format.to_ascii_lowercase()
    } else {
        "bin".to_string()
    }
}

/// A unique sibling path for an in-progress download.
fn unique_temp(destination: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "track".to_string());
    destination.with_file_name(format!("{name}.part.{}.{sequence}", std::process::id()))
}

/// Replaces `destination` with `source`, tolerating an existing file on
/// Windows.
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    std::fs::rename(source, destination)?;
    Ok(())
}

/// Rejects a path that is not lexically inside `root`.
fn ensure_inside(root: &Path, path: &Path) -> Result<PathBuf> {
    let inside = path.starts_with(root)
        && path.components().all(|component| {
            matches!(
                component,
                Component::Normal(_) | Component::Prefix(_) | Component::RootDir
            )
        })
        && path.strip_prefix(root).is_ok_and(|rest| {
            rest.components()
                .all(|component| matches!(component, Component::Normal(_)))
        });
    if inside {
        Ok(path.to_path_buf())
    } else {
        Err(ClientError::Protocol(format!(
            "refusing path outside the cache: {}",
            path.display()
        )))
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
        let a = cache.path_for("srv1", &track("abc", "flac")).unwrap();
        assert_eq!(a, cache.path_for("srv1", &track("abc", "flac")).unwrap());
        assert_ne!(a, cache.path_for("srv2", &track("abc", "flac")).unwrap());
        assert!(a.to_string_lossy().ends_with("abc.flac"));
    }

    #[test]
    fn missing_extension_falls_back_to_bin() {
        let cache = TrackCache::with_root(PathBuf::from("/cache"));
        assert!(
            cache
                .path_for("srv", &track("x", ""))
                .unwrap()
                .to_string_lossy()
                .ends_with("x.bin")
        );
    }

    #[test]
    fn rejects_path_traversal_in_id_or_format() {
        let cache = TrackCache::with_root(PathBuf::from("/cache"));
        for evil in [
            "../evil",
            "..",
            ".",
            "/etc/passwd",
            "a/b",
            "a\\b",
            "a\0b",
            "C:\\Windows\\x",
        ] {
            assert!(
                cache.path_for("srv", &track(evil, "flac")).is_err(),
                "id {evil:?} must be rejected"
            );
        }
        // A hostile format cannot escape either; it degrades to .bin.
        let path = cache
            .path_for("srv", &track("ok", "../../etc/cron.d/x"))
            .unwrap();
        assert!(path.starts_with("/cache/srv"));
        assert!(path.to_string_lossy().ends_with("ok.bin"));
        assert!(cache.path_for("../srv", &track("ok", "flac")).is_err());
    }
}
