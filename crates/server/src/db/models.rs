//! SQLite database models for emusic-server.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub public_key: String,
    pub paired_at: i64,
    pub last_seen: Option<i64>,
    pub is_revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerTrack {
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
