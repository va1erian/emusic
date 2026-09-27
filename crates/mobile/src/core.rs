//! The [`MobileCore`] object exported to Kotlin.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use emusic_client::auth::{
    generate_keypair, issue_refresh_proof, public_key_paserk, secret_key_paserk, token_fingerprint,
};
use emusic_client::cache::safe_id;
use emusic_client::{CredentialStore, Credentials, RemoteClient, ServerEndpoint, unix_now};
use emusic_render::{MODULE_EXTENSIONS, RenderOptions, SID_EXTENSIONS, flac};

use crate::error::MobileError;
use crate::library_store;
use crate::registry;
use crate::types::{AuthState, Health, ServerEntry, SyncResult, Track};
use crate::util::{encode_segment, renderer_for, safe_extension};

/// Renew the access token when it expires within this margin.
const REFRESH_MARGIN_SECS: i64 = 24 * 3600;

/// Sample rate mobile renderers produce.
const RENDER_SAMPLE_RATE: u32 = 44_100;

/// Bump when the on-device cache layout or renderer output changes, to
/// invalidate previously rendered rendition files.
const RENDITION_VERSION: u32 = 1;

/// A client bound to one `emusic-server`.
///
/// Constructed once per server from Kotlin with the app's private data
/// directory; holds the device credentials and the blocking HTTP client.
#[derive(uniffi::Object)]
pub struct MobileCore {
    endpoint: ServerEndpoint,
    store: CredentialStore,
    client: RemoteClient,
    data_dir: PathBuf,
}

impl MobileCore {
    /// Loads the stored credentials for this server, refreshing the token when
    /// it is close to expiry.
    fn fresh_token(&self) -> Result<(Credentials, String), MobileError> {
        let mut credentials =
            self.store
                .load(&self.endpoint.id)?
                .ok_or_else(|| MobileError::Client {
                    message: "not paired with this server".into(),
                })?;
        if credentials.expiring_within(REFRESH_MARGIN_SECS, unix_now()) {
            let secret = credentials.secret_key()?;
            let fingerprint = token_fingerprint(&credentials.token);
            let proof = issue_refresh_proof(
                &secret,
                &fingerprint,
                Duration::from_secs(120),
                Duration::from_secs(30),
            )?;
            let response = self.client.refresh(&credentials.token, &proof)?;
            credentials.token = response.auth_token;
            credentials.expires_at = response.expires_at;
            self.store.save(&self.endpoint.id, &credentials)?;
        }
        let token = credentials.token.clone();
        Ok((credentials, token))
    }

    fn auth_state_from(&self, credentials: Option<&Credentials>) -> AuthState {
        AuthState {
            base_url: self.endpoint.url.clone(),
            paired: credentials.is_some(),
            device_id: credentials.map(|stored| stored.device_id.clone()),
            device_name: credentials.map(|stored| stored.device_name.clone()),
            expires_at: credentials.map(|stored| stored.expires_at),
            since_version: credentials.map(|stored| stored.since_version),
        }
    }
}

/// The registry entry describing an endpoint.
fn entry_for(endpoint: &ServerEndpoint) -> ServerEntry {
    ServerEntry {
        id: endpoint.id.clone(),
        name: endpoint.name.clone(),
        url: endpoint.url.clone(),
    }
}

/// Maps a server sync delta to the mobile record.
fn map_delta(delta: emusic_client::SyncDelta) -> SyncResult {
    SyncResult {
        version: delta.version,
        tracks: delta.tracks.into_iter().map(Track::from).collect(),
        deleted: delta.deleted,
    }
}

/// Lists the servers this app has been configured with.
#[uniffi::export]
pub fn list_servers(data_dir: String) -> Result<Vec<ServerEntry>, MobileError> {
    registry::load(&PathBuf::from(data_dir))
}

#[uniffi::export]
impl MobileCore {
    /// Builds a core for `base_url`, storing credentials under `data_dir`.
    #[uniffi::constructor]
    pub fn new(base_url: String, data_dir: String) -> Result<Arc<Self>, MobileError> {
        let endpoint = ServerEndpoint::new("server", &base_url)?;
        let client = RemoteClient::from_endpoint(&endpoint)?;
        let data_dir = PathBuf::from(data_dir);
        let store = CredentialStore::with_dir(data_dir.join("servers"));
        Ok(Arc::new(Self {
            endpoint,
            store,
            client,
            data_dir,
        }))
    }

    /// The registry entry for this server.
    pub fn entry(&self) -> ServerEntry {
        entry_for(&self.endpoint)
    }

    /// The normalized base URL this core is bound to.
    pub fn base_url(&self) -> String {
        self.endpoint.url.clone()
    }

    /// `GET /health`: liveness and server start time.
    pub fn health(&self) -> Result<Health, MobileError> {
        let health = self.client.health()?;
        Ok(Health {
            status: health.status,
            started_at: health.started_at,
        })
    }

    /// Pairs the device with a one-time code and persists the credentials.
    pub fn pair(&self, code: String, device_name: String) -> Result<AuthState, MobileError> {
        let (secret, public) = generate_keypair()?;
        let response = self
            .client
            .pair(&code, &device_name, &public_key_paserk(&public)?)?;
        let credentials = Credentials {
            device_id: response.device_id,
            device_name: response.device_name,
            secret: secret_key_paserk(&secret)?,
            token: response.auth_token,
            expires_at: response.expires_at,
            since_version: 0,
        };
        self.store.save(&self.endpoint.id, &credentials)?;
        registry::upsert(&self.data_dir, &entry_for(&self.endpoint))?;
        Ok(self.auth_state_from(Some(&credentials)))
    }

    /// The local pairing state; no network call is made.
    pub fn auth_state(&self) -> Result<AuthState, MobileError> {
        let credentials = self.store.load(&self.endpoint.id)?;
        Ok(self.auth_state_from(credentials.as_ref()))
    }

    /// Forgets this device's credentials.
    pub fn disconnect(&self) -> Result<(), MobileError> {
        self.store.remove(&self.endpoint.id)?;
        Ok(())
    }

    /// Revokes this device on the server, then forgets its credentials.
    pub fn revoke(&self) -> Result<(), MobileError> {
        let (credentials, token) = self.fresh_token()?;
        self.client.revoke_device(&token, &credentials.device_id)?;
        self.store.remove(&self.endpoint.id)?;
        Ok(())
    }

    /// Forgets the credentials and removes the server from the registry.
    pub fn remove_server(&self) -> Result<(), MobileError> {
        self.store.remove(&self.endpoint.id)?;
        registry::remove(&self.data_dir, &self.endpoint.id)?;
        Ok(())
    }

    /// The current bearer token, refreshed if needed.
    ///
    /// Used by the Media3 data source to authenticate stream requests.
    pub fn bearer_token(&self) -> Result<String, MobileError> {
        let (_, token) = self.fresh_token()?;
        Ok(token)
    }

    /// Deltas since the last synced version and advances the stored version.
    pub fn sync(&self) -> Result<SyncResult, MobileError> {
        let (mut credentials, token) = self.fresh_token()?;
        let delta = self.client.sync(&token, credentials.since_version)?;
        credentials.since_version = delta.version;
        self.store.save(&self.endpoint.id, &credentials)?;
        Ok(map_delta(delta))
    }

    /// The full library as currently indexed by the server (sync from version 0).
    ///
    /// Unlike [`Self::sync`], this does not move the stored cursor, so an
    /// incremental sync can still follow it.
    pub fn library(&self) -> Result<SyncResult, MobileError> {
        let (_, token) = self.fresh_token()?;
        let delta = self.client.sync(&token, 0)?;
        Ok(map_delta(delta))
    }

    /// The last synced library, read from the local snapshot (no network).
    ///
    /// Empty when the app has not completed a sync yet.
    pub fn cached_library(&self) -> Result<SyncResult, MobileError> {
        let snapshot = library_store::load(&self.data_dir)?;
        Ok(SyncResult {
            version: snapshot.version,
            tracks: snapshot.tracks,
            deleted: Vec::new(),
        })
    }

    /// Applies the server's delta to the local snapshot and returns the full,
    /// updated library, persisting it for the next launch.
    pub fn refresh_library(&self) -> Result<SyncResult, MobileError> {
        let mut snapshot = library_store::load(&self.data_dir)?;
        let (_, token) = self.fresh_token()?;
        let delta = self.client.sync(&token, snapshot.version)?;

        let updates: Vec<Track> = delta.tracks.into_iter().map(Track::from).collect();
        let deleted = delta.deleted;
        let version = delta.version;
        merge_tracks(&mut snapshot.tracks, updates, &deleted);
        snapshot.version = version;
        library_store::save(&self.data_dir, &snapshot)?;

        Ok(SyncResult {
            version,
            tracks: snapshot.tracks,
            deleted,
        })
    }

    /// The authenticated `/stream` URL for a track.
    pub fn stream_url(&self, track_id: String) -> String {
        format!(
            "{}/api/v1/tracks/{}/stream",
            self.endpoint.url,
            encode_segment(&track_id)
        )
    }

    /// The `/render` URL for a specialized track (subtune + FLAC).
    ///
    /// Used for formats this device cannot render locally (currently SID). The
    /// server must have `[render] enabled = true`, otherwise it returns `404`.
    /// `subtune == 0` means the tune's default and is omitted (the server
    /// rejects an explicit `subtune=0`).
    pub fn render_url(&self, track_id: String, subtune: u32) -> String {
        let mut url = format!(
            "{}/api/v1/tracks/{}/render?codec=flac",
            self.endpoint.url,
            encode_segment(&track_id)
        );
        if subtune > 0 {
            url.push_str(&format!("&subtune={subtune}"));
        }
        url
    }

    /// Downloads a track to `out_path`, creating parent directories.
    pub fn download(&self, track_id: String, out_path: String) -> Result<u64, MobileError> {
        let (_, token) = self.fresh_token()?;
        let path = PathBuf::from(out_path);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&path)?;
        let bytes = self.client.download_to(&token, &track_id, &mut file)?;
        Ok(bytes)
    }

    /// Whether `format` can be rendered to FLAC on this device.
    ///
    /// Pure-Rust tracker modules (MOD/XM/S3M/IT) and SID (PSID/RSID) are
    /// supported locally; other specialized formats are not.
    pub fn can_render(&self, format: String) -> bool {
        MODULE_EXTENSIONS
            .iter()
            .chain(SID_EXTENSIONS)
            .any(|candidate| format.eq_ignore_ascii_case(candidate))
    }

    /// Downloads a specialized track and renders it to a cached local FLAC file.
    ///
    /// Returns the absolute path of the FLAC. A track is rendered at most once;
    /// later calls reuse the cached file. The raw download is removed afterwards.
    pub fn render_to_file(&self, track_id: String, format: String) -> Result<String, MobileError> {
        let id = safe_id(&track_id).ok_or_else(|| MobileError::Client {
            message: format!("unsafe track id {track_id:?}"),
        })?;
        // Resolve the renderer first: an unsupported format must not download.
        let renderer =
            renderer_for(&format, RENDER_SAMPLE_RATE).ok_or_else(|| MobileError::Client {
                message: format!("no local renderer for {format:?}"),
            })?;

        let renditions = self.data_dir.join("renditions");
        std::fs::create_dir_all(&renditions)?;
        let extension = safe_extension(&format);
        let flac_path = renditions.join(format!("{id}.{extension}.v{RENDITION_VERSION}.flac"));
        if flac_path.is_file() {
            return Ok(flac_path.to_string_lossy().into_owned());
        }

        let (_, token) = self.fresh_token()?;
        let raw_path = renditions.join(format!("{id}.{extension}"));
        {
            let mut file = std::fs::File::create(&raw_path)?;
            self.client.download_to(&token, &track_id, &mut file)?;
        }

        // Render and encode, always removing the raw download afterwards.
        let encoded = (|| -> Result<Vec<u8>, MobileError> {
            let pcm = renderer
                .render(&raw_path, &RenderOptions::default())
                .map_err(|error| MobileError::Client {
                    message: format!("render failed: {error}"),
                })?;
            flac::encode(&pcm).map_err(|error| MobileError::Client {
                message: format!("encode failed: {error}"),
            })
        })();
        let _ = std::fs::remove_file(&raw_path);
        let bytes = encoded?;

        let temporary = flac_path.with_extension("flac.tmp");
        std::fs::write(&temporary, &bytes)?;
        crate::util::replace_file(&temporary, &flac_path)?;
        Ok(flac_path.to_string_lossy().into_owned())
    }

    /// Downloads and caches an album's cover art, returning the local path.
    pub fn album_art_file(&self, album_id: String) -> Result<String, MobileError> {
        let id = safe_id(&album_id).ok_or_else(|| MobileError::Client {
            message: format!("unsafe album id {album_id:?}"),
        })?;
        let art = self.data_dir.join("art");
        std::fs::create_dir_all(&art)?;
        let destination = art.join(id);
        if destination.is_file() {
            return Ok(destination.to_string_lossy().into_owned());
        }
        let (_, token) = self.fresh_token()?;
        let bytes = self.client.album_art(&token, &album_id)?;
        let temporary = art.join(format!("{id}.tmp"));
        std::fs::write(&temporary, &bytes)?;
        crate::util::replace_file(&temporary, &destination)?;
        Ok(destination.to_string_lossy().into_owned())
    }
}

/// Applies `updates` and `deleted` to `tracks` in place, preserving order.
fn merge_tracks(tracks: &mut Vec<Track>, updates: Vec<Track>, deleted: &[String]) {
    let mut positions: std::collections::HashMap<String, usize> = tracks
        .iter()
        .enumerate()
        .map(|(index, track)| (track.id.clone(), index))
        .collect();
    for track in updates {
        match positions.get(&track.id).copied() {
            Some(index) => tracks[index] = track,
            None => {
                positions.insert(track.id.clone(), tracks.len());
                tracks.push(track);
            }
        }
    }
    if !deleted.is_empty() {
        let removed: std::collections::HashSet<&str> = deleted.iter().map(String::as_str).collect();
        tracks.retain(|track| !removed.contains(track.id.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_urls_and_normalizes_good_ones() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_string_lossy().into_owned();
        assert!(MobileCore::new("music.example.com".into(), data.clone()).is_err());
        let core = MobileCore::new("https://music.example.com/".into(), data).unwrap();
        assert_eq!(core.base_url(), "https://music.example.com");
    }

    #[test]
    fn not_paired_until_credentials_are_stored() {
        let dir = tempfile::tempdir().unwrap();
        let core = MobileCore::new(
            "https://music.example.com".into(),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();
        let state = core.auth_state().unwrap();
        assert!(!state.paired);
        assert!(state.device_id.is_none());
        assert!(core.bearer_token().is_err());
        assert!(core.sync().is_err());
    }

    #[test]
    fn builds_stream_and_render_urls() {
        let dir = tempfile::tempdir().unwrap();
        let core = MobileCore::new(
            "https://music.example.com".into(),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();
        assert_eq!(
            core.stream_url("abc123".into()),
            "https://music.example.com/api/v1/tracks/abc123/stream"
        );
        assert_eq!(
            core.stream_url("a/b".into()),
            "https://music.example.com/api/v1/tracks/a%2Fb/stream"
        );
        assert_eq!(
            core.render_url("abc123".into(), 3),
            "https://music.example.com/api/v1/tracks/abc123/render?codec=flac&subtune=3"
        );
        assert_eq!(
            core.render_url("abc123".into(), 0),
            "https://music.example.com/api/v1/tracks/abc123/render?codec=flac"
        );
    }

    #[test]
    fn encode_segment_escapes_reserved_bytes() {
        assert_eq!(encode_segment("abc123"), "abc123");
        assert_eq!(encode_segment("a b#c"), "a%20b%23c");
    }

    #[test]
    fn new_does_not_register_until_paired() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_string_lossy().into_owned();
        let core = MobileCore::new("https://music.example.com".into(), data.clone()).unwrap();
        assert!(list_servers(data.clone()).unwrap().is_empty());
        core.remove_server().unwrap();
        assert!(list_servers(data).unwrap().is_empty());
    }

    #[test]
    fn knows_which_formats_it_can_render_locally() {
        let dir = tempfile::tempdir().unwrap();
        let core = MobileCore::new(
            "https://music.example.com".into(),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();
        for format in ["mod", "MOD", "xm", "s3m", "it", "sid", "psid"] {
            assert!(core.can_render(format.into()), "{format} should render");
        }
        for format in ["flac", "mp3", "mid", "mo3", "ape"] {
            assert!(!core.can_render(format.into()), "{format} is not local");
        }
    }

    #[test]
    fn cached_library_is_empty_until_synced() {
        let dir = tempfile::tempdir().unwrap();
        let core = MobileCore::new(
            "https://music.example.com".into(),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();
        let cached = core.cached_library().unwrap();
        assert_eq!(cached.version, 0);
        assert!(cached.tracks.is_empty());
    }

    fn track(id: &str, title: &str) -> Track {
        Track {
            id: id.into(),
            filename: None,
            directory: String::new(),
            format: "mp3".into(),
            kind: "stream".into(),
            specialized: false,
            title: Some(title.into()),
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
        }
    }

    #[test]
    fn merge_adds_updates_and_deletes() {
        let mut tracks = vec![track("a", "A"), track("b", "B")];
        // Update b, add c and delete a in one batch.
        merge_tracks(
            &mut tracks,
            vec![track("b", "B2"), track("c", "C")],
            &["a".to_string()],
        );
        let ids: Vec<&str> = tracks.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, ["b", "c"]);
        assert_eq!(tracks[0].title.as_deref(), Some("B2"));
    }

    #[test]
    fn merge_is_idempotent() {
        let mut tracks = vec![track("a", "A")];
        merge_tracks(&mut tracks, vec![track("a", "A")], &[]);
        merge_tracks(&mut tracks, vec![track("a", "A")], &[]);
        assert_eq!(tracks.len(), 1);
    }
}
