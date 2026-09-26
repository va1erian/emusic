use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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
            data_dir: PathBuf::from("/var/lib/emusic-server"),
            trusted_proxies: vec!["172.16.0.0/12".to_string(), "127.0.0.1".to_string(), "::1".to_string()],
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
            max_pairing_attempts_per_min: 5,
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
            paths: vec![PathBuf::from("/media/music")],
            hvsc_songlengths_path: None,
            scan_interval_secs: 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub library: LibraryConfig,
}

impl AppConfig {
    pub fn load_or_default(path: Option<&std::path::Path>) -> Self {
        if let Some(p) = path
            && let Ok(content) = std::fs::read_to_string(p)
            && let Ok(cfg) = toml::from_str::<AppConfig>(&content)
        {
            return cfg;
        }

        // Check environment variables for Docker / Cosmos Cloud overrides
        let mut cfg = AppConfig::default();
        if let Ok(val) = std::env::var("EMUSIC_HOST") {
            cfg.server.host = val;
        }
        if let Ok(val) = std::env::var("EMUSIC_PORT")
            && let Ok(port) = val.parse()
        {
            cfg.server.port = port;
        }
        if let Ok(val) = std::env::var("EMUSIC_DATA_DIR") {
            cfg.server.data_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("EMUSIC_MUSIC_DIR") {
            cfg.library.paths = vec![PathBuf::from(val)];
        }
        cfg
    }
}
