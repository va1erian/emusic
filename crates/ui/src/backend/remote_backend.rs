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
use emusic_player::PlayerError;
use emusic_player::backend::{AudioBackend, BackendChannel};
use tracing::warn;

use crate::backend::library::{Update, Updates};
use crate::remote::RemoteRegistry;

/// Wraps another [`AudioBackend`], fetching remote cache paths first.
pub(crate) struct RemoteAudioBackend {
    inner: Arc<dyn AudioBackend>,
    cache: TrackCache,
    credentials: CredentialStore,
    registry: RemoteRegistry,
    /// Posts the network-indicator status line while a track is fetched.
    updates: Updates,
}

impl RemoteAudioBackend {
    /// Wraps `inner`, using `cache` for downloaded tracks and `registry` to
    /// resolve a cache path's server id to its URL.
    pub(crate) fn new(
        inner: Arc<dyn AudioBackend>,
        cache: TrackCache,
        credentials: CredentialStore,
        registry: RemoteRegistry,
        updates: Updates,
    ) -> Self {
        Self {
            inner,
            cache,
            credentials,
            registry,
            updates,
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
        // Network indicator: shown in the status bar while the file downloads,
        // then left as the current track's source until the next track opens.
        let _ = self.updates.send(Update::Network(Some(format!(
            "⇅ Streaming \"{}\" from {}…",
            view.display_title(),
            server.name
        ))));
        let result = self
            .cache
            .ensure(&client, &token, &server_id, &view)
            .map_err(|error| error.to_string());
        match result {
            Ok(destination) => {
                let _ = self.updates.send(Update::Network(Some(format!(
                    "⇅ \"{}\" from {}",
                    view.display_title(),
                    server.name
                ))));
                Ok(Some(destination))
            }
            Err(message) => {
                let _ = self.updates.send(Update::Network(None));
                Err(message)
            }
        }
    }
}

impl AudioBackend for RemoteAudioBackend {
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError> {
        // Clear any previous track's source indicator before this one.
        let _ = self.updates.send(Update::Network(None));
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
    let server = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let file = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    if components.next().is_some() {
        return None;
    }
    let track = Path::new(&file).file_stem()?.to_string_lossy().into_owned();
    Some((server, track))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    // The literals below use Windows separators, which `Path` only splits on
    // Windows; the parser itself is portable (it walks `components`).
    #[cfg(windows)]
    #[test]
    fn parses_remote_cache_paths() {
        let root = Path::new(r"C:\cache\remote");
        assert_eq!(
            parse_remote_path(Path::new(r"C:\cache\remote\srv1\abc.flac"), root),
            Some(("srv1".to_string(), "abc".to_string()))
        );
        assert_eq!(
            parse_remote_path(Path::new(r"C:\music\song.flac"), root),
            None
        );
        // Nested paths (unexpected) are rejected.
        assert_eq!(
            parse_remote_path(Path::new(r"C:\cache\remote\srv1\sub\abc.flac"), root),
            None
        );
    }
}
