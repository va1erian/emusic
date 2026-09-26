//! Remote `emusic-server` configuration, persisted in `config.toml` (#391).
//!
//! The server list is non-secret; the device keypair and access token live in
//! the credential store (`emusic_client::CredentialStore`), not here. The
//! [`id`](RemoteServer::id) is derived from the normalized URL, so it is stable
//! across restarts and matches the credential and cache namespaces.

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use emusic_client::auth::{generate_keypair, public_key_paserk, secret_key_paserk};
use emusic_client::{CredentialStore, Credentials, RemoteClient};

/// A live, shared view of the configured servers, so the playback backend can
/// resolve a cache path's server id to its URL without owning the config.
#[derive(Debug, Clone, Default)]
pub struct RemoteRegistry {
    inner: Arc<RwLock<HashMap<String, RemoteServer>>>,
}

impl RemoteRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the contents with `servers`.
    pub fn replace(&self, servers: &[RemoteServer]) {
        let mut map = self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.clear();
        for server in servers {
            map.insert(server.id.clone(), server.clone());
        }
    }

    /// Looks up a server by its id.
    pub fn get(&self, id: &str) -> Option<RemoteServer> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(id)
            .cloned()
    }
}

/// A configured remote server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteServer {
    /// Stable id derived from the URL.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Normalized base URL, without a trailing slash.
    pub url: String,
}

impl RemoteServer {
    /// Builds a server entry, normalizing and validating the URL.
    pub fn new(name: impl Into<String>, url: &str) -> Result<Self, String> {
        let endpoint = emusic_client::ServerEndpoint::new(name, url).map_err(|e| e.to_string())?;
        Ok(Self {
            id: endpoint.id,
            name: endpoint.name,
            url: endpoint.url,
        })
    }

    /// The equivalent client endpoint.
    pub fn endpoint(&self) -> emusic_client::ServerEndpoint {
        emusic_client::ServerEndpoint {
            id: self.id.clone(),
            name: self.name.clone(),
            url: self.url.clone(),
        }
    }
}

/// Pairs a new device with a server on the calling (background) thread:
/// generates a device keypair, exchanges the one-time code for a token and
/// stores the credentials. Returns the server entry and a success message.
///
/// This is a blocking network call; the UI runs it on a worker thread.
pub fn pair(device_name: &str, url: &str, code: &str) -> Result<(RemoteServer, String), String> {
    let name = device_name.trim();
    let name = if name.is_empty() { "emusic" } else { name };
    let server = RemoteServer::new(name, url)?;
    let client = RemoteClient::from_endpoint(&server.endpoint()).map_err(|e| e.to_string())?;
    let (secret, public) = generate_keypair().map_err(|e| e.to_string())?;
    let public_key = public_key_paserk(&public).map_err(|e| e.to_string())?;
    let response = client
        .pair(code.trim(), &server.name, &public_key)
        .map_err(|e| e.to_string())?;
    let credentials = Credentials {
        device_id: response.device_id,
        device_name: response.device_name.clone(),
        secret: secret_key_paserk(&secret).map_err(|e| e.to_string())?,
        token: response.auth_token,
        expires_at: response.expires_at,
        since_version: 0,
    };
    CredentialStore::new()
        .map_err(|e| e.to_string())?
        .save(&server.id, &credentials)
        .map_err(|e| e.to_string())?;
    Ok((server, format!("Paired as {}", response.device_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalizes_url_and_derives_a_stable_id() {
        let a = RemoteServer::new("Home", "https://music.example.com/").unwrap();
        let b = RemoteServer::new("Home", "https://music.example.com").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.id.len(), 16);
        assert_eq!(a.url, "https://music.example.com");
    }

    #[test]
    fn new_rejects_scheme_less_urls() {
        assert!(RemoteServer::new("Home", "music.example.com").is_err());
    }

    #[test]
    fn registry_replaces_and_looks_up_by_id() {
        let registry = RemoteRegistry::new();
        let server = RemoteServer::new("Home", "https://music.example.com").unwrap();
        registry.replace(std::slice::from_ref(&server));
        assert_eq!(registry.get(&server.id), Some(server.clone()));
        assert_eq!(registry.get("missing"), None);
        registry.replace(&[]);
        assert_eq!(registry.get(&server.id), None);
    }

    #[test]
    fn pair_rejects_a_scheme_less_url_before_networking() {
        let error = pair("pc", "music.example.com", "123456").unwrap_err();
        assert!(error.contains("http"), "unexpected error: {error}");
    }
}
