//! Playback backend decorator that materialises remote tracks (#391).
//!
//! Remote tracks are stored with a synthetic path under the cache root. When
//! the player opens such a path, this decorator downloads the file through
//! `emusic_client` and delegates to the real BASS/SID backend at the local
//! path; local paths pass straight through. This keeps the whole player
//! pipeline untouched.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use emusic_client::{CredentialStore, RemoteClient, TrackCache};
use emusic_player::backend::{AudioBackend, BackendChannel};
use emusic_player::PlayerError;
use tracing::warn;

use crate::remote::RemoteRegistry;

/// Wraps another [`AudioBackend`], fetching remote cache paths first.
pub(crate) struct RemoteAudioBackend {
    inner: Arc<dyn AudioBackend>,
    cache: TrackCache,
    credentials: CredentialStore,
    registry: RemoteRegistry,
}

impl RemoteAudioBackend {
    /// Wraps `inner`, using `cache` for downloaded tracks and `registry` to
    /// resolve a cache path's server id to its URL.
    pub(crate) fn new(
        inner: Arc<dyn AudioBackend>,
        cache: TrackCache,
        credentials: CredentialStore,
        registry: RemoteRegistry,
    ) -> Self {
        Self {
            inner,
            cache,
            credentials,
            registry,
        }
    }

    /// If `path` is a remote cache path, ensures the file is downloaded and
    /// returns its local path; `None` for ordinary local paths.
    fn materialise(&self, path: &Path) -> Result<Option<PathBuf>, String> {
        let Some((server_id, track_id)) = parse_remote_path(path, self.cache.root()) else {
            return Ok(None);
        };
        let server = self
            .registry
            .get(&server_id)
            .ok_or_else(|| format!("unknown remote server {server_id}"))?;
        let client =
            RemoteClient::from_endpoint(&server.endpoint()).map_err(|error| error.to_string())?;
        let token = super::remote_auth::ensure_token(&client, &self.credentials, &server_id)?;
        let view = client
            .track_meta(&token, &track_id)
            .map_err(|error| error.to_string())?;
        let destination = self
            .cache
            .ensure(&client, &token, &server_id, &view)
            .map_err(|error| error.to_string())?;
        Ok(Some(destination))
    }
}

impl AudioBackend for RemoteAudioBackend {
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError> {
        match self.materialise(path) {
            Ok(Some(local)) => self.inner.open(&local),
            Ok(None) => self.inner.open(path),
            Err(message) => {
                warn!(path = %path.display(), %message, "remote track fetch failed");
                Err(PlayerError::ReadFailed(message))
            }
        }
    }

    fn set_tracker_resampling_quality(&self, quality: u8) -> Result<(), PlayerError> {
        self.inner.set_tracker_resampling_quality(quality)
    }

    fn set_midi_soundfont(&self, configured: Option<&Path>) -> Result<(), PlayerError> {
        self.inner.set_midi_soundfont(configured)
    }

    fn set_songlengths_path(&self, path: Option<&Path>) {
        self.inner.set_songlengths_path(path);
    }

    fn set_sid_fallback_length(&self, length: Duration) {
        self.inner.set_sid_fallback_length(length);
    }
}

/// Splits `<root>/<server-id>/<track-id>.<ext>` into `(server-id, track-id)`.
fn parse_remote_path(path: &Path, root: &Path) -> Option<(String, String)> {
    let rest = path.strip_prefix(root).ok()?;
    let mut components = rest.components();
    let server = components.next()?.as_os_str().to_string_lossy().into_owned();
    let file = components.next()?.as_os_str().to_string_lossy().into_owned();
    if components.next().is_some() {
        return None;
    }
    let track = Path::new(&file).file_stem()?.to_string_lossy().into_owned();
    Some((server, track))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remote_cache_paths() {
        let root = Path::new(r"C:\cache\remote");
        assert_eq!(
            parse_remote_path(Path::new(r"C:\cache\remote\srv1\abc.flac"), root),
            Some(("srv1".to_string(), "abc".to_string()))
        );
        assert_eq!(parse_remote_path(Path::new(r"C:\music\song.flac"), root), None);
        // Nested paths (unexpected) are rejected.
        assert_eq!(
            parse_remote_path(Path::new(r"C:\cache\remote\srv1\sub\abc.flac"), root),
            None
        );
    }
}
