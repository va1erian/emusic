//! Remote `emusic-server` configuration, persisted in `config.toml` (#391).
//!
//! The server list is non-secret; the device keypair and access token live in
//! the credential store (`emusic_client::CredentialStore`), not here. The
//! [`id`](RemoteServer::id) is derived from the normalized URL, so it is stable
//! across restarts and matches the credential and cache namespaces.

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
}
