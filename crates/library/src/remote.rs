//! Remote library synchronization client models and helpers for `emusic-server`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteTrack {
    pub id: String,
    pub relative_path: String,
    pub format: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub subtunes: u16,
    pub file_size: u64,
    pub mtime: u64,
    pub hash: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteServerConfig {
    pub server_url: String,
    pub device_id: String,
    pub auth_token: String,
    pub last_synced_version: u64,
}
