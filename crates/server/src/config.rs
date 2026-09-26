//! Server configuration loader and models.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub library: LibraryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub trusted_proxies: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            data_dir: PathBuf::from("data"),
            trusted_proxies: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub token_ttl_hours: u64,
    pub max_pairing_attempts_per_min: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tls_cert: None,
            tls_key: None,
            token_ttl_hours: 168,
            max_pairing_attempts_per_min: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryConfig {
    pub paths: Vec<PathBuf>,
    pub hvsc_songlengths_path: Option<PathBuf>,
    pub scan_interval_secs: u64,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            hvsc_songlengths_path: None,
            scan_interval_secs: 3600,
        }
    }
}

impl Config {
    pub fn load_from_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.security.token_ttl_hours, 168);
    }

    #[test]
    fn test_toml_parse() {
        let toml_str = r#"
        [server]
        host = "127.0.0.1"
        port = 9000
        data_dir = "/tmp/emusic"
        trusted_proxies = ["127.0.0.1"]

        [security]
        token_ttl_hours = 24
        max_pairing_attempts_per_min = 5

        [library]
        paths = ["/media/music"]
        scan_interval_secs = 1800
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse toml");
        assert_eq!(cfg.server.port, 9000);
        assert_eq!(cfg.library.paths, vec![PathBuf::from("/media/music")]);
    }
}
