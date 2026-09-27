//! Server configuration: TOML file plus `EMUSIC_SERVER_*` environment
//! overrides.
//!
//! The file is the source of truth for structure; environment variables are
//! applied on top so a container can be configured without baking a file into
//! the image (see `docs/server.md`). Validation is intentionally strict:
//! inconsistent or out-of-range values are refused at startup rather than
//! silently weakening a control at first request.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

/// Default listen port when behind a TLS-terminating reverse proxy.
pub const DEFAULT_PORT: u16 = 8080;

/// Upper bound for `security.token_ttl_hours` (one year).
pub const MAX_TOKEN_TTL_HOURS: u64 = 24 * 365;

/// Upper bound for `security.pairing_code_ttl_secs` (one hour).
pub const MAX_PAIRING_CODE_TTL_SECS: u64 = 3600;

/// Upper bound for `security.max_body_bytes` (1 MiB).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Upper bound for `render.sample_rate` (192 kHz).
pub const MAX_RENDER_SAMPLE_RATE: u32 = 192_000;

/// Upper bound for `render.max_concurrent` renders.
pub const MAX_RENDER_CONCURRENT: u32 = 64;

/// Upper bound for `render.cache_max_bytes` (64 GiB).
pub const MAX_RENDER_CACHE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// The only `render.codec` supported today.
pub const RENDER_CODEC_FLAC: &str = "flac";

/// Top-level configuration tree.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Listener and storage location.
    pub server: ServerConfig,
    /// Authentication, TLS and rate-limit knobs.
    pub security: SecurityConfig,
    /// Library roots and scan scheduling.
    pub library: LibraryConfig,
    /// Server-side rendering of SID, module and MIDI files for Android.
    pub render: RenderConfig,
}

/// `[server]` table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    /// Interface to bind. Use `0.0.0.0` only behind a trusted reverse proxy.
    pub host: String,
    /// TCP port to bind on the internal network.
    pub port: u16,
    /// Directory holding the database, server key and runtime state.
    pub data_dir: PathBuf,
    /// Reverse-proxy addresses whose `X-Forwarded-*` headers are trusted.
    /// Entries are CIDR ranges (`172.16.0.0/12`) or single IPs (`127.0.0.1`).
    pub trusted_proxies: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: DEFAULT_PORT,
            data_dir: default_data_dir(),
            trusted_proxies: Vec::new(),
        }
    }
}

/// `[security]` table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct SecurityConfig {
    /// PEM certificate chain for direct-TLS mode; empty behind a proxy.
    pub tls_cert: String,
    /// PEM private key for direct-TLS mode; empty behind a proxy.
    pub tls_key: String,
    /// Lifetime of an issued PASETO access token, in hours.
    pub token_ttl_hours: u64,
    /// Maximum pairing attempts per minute, per client IP.
    pub max_pairing_attempts_per_min: u32,
    /// How long a generated pairing code stays valid.
    pub pairing_code_ttl_secs: u64,
    /// Maximum accepted request body size, in bytes.
    pub max_body_bytes: usize,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tls_cert: String::new(),
            tls_key: String::new(),
            token_ttl_hours: 168,
            max_pairing_attempts_per_min: 3,
            pairing_code_ttl_secs: 600,
            max_body_bytes: 64 * 1024,
        }
    }
}

/// `[library]` table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct LibraryConfig {
    /// Filesystem roots to scan and serve. Directories only.
    pub paths: Vec<PathBuf>,
    /// Path to HVSC's `Songlengths.md5`, when SID song lengths are wanted.
    pub hvsc_songlengths_path: Option<PathBuf>,
    /// Seconds between background scans; `0` disables periodic scanning.
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

/// `[render]` table.
///
/// Rendering turns specialized files (SID today; tracker modules and MIDI when
/// their renderers land) into a codec Android can decode. It is off by default
/// because it is CPU-heavy and needs a writable cache.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct RenderConfig {
    /// Whether the `/render` endpoint and `renderable` metadata are available.
    pub enabled: bool,
    /// Output codec. Only `flac` is supported today.
    pub codec: String,
    /// Rendering sample rate, in Hz.
    pub sample_rate: u32,
    /// SoundFont for MIDI rendering; unused until the MIDI renderer lands.
    pub soundfont_path: Option<PathBuf>,
    /// Upper bound on the rendition cache, in bytes.
    pub cache_max_bytes: u64,
    /// Maximum number of concurrent renders.
    pub max_concurrent: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            codec: RENDER_CODEC_FLAC.to_string(),
            sample_rate: 44_100,
            soundfont_path: None,
            cache_max_bytes: 2 * 1024 * 1024 * 1024,
            max_concurrent: 2,
        }
    }
}

impl Config {
    /// Loads configuration from `path`, applies environment overrides and
    /// validates the result.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            ServerError::Config(format!("cannot read {}: {error}", path.display()))
        })?;
        let mut config = Self::from_toml(&text)?;
        config.apply_env_with(|key| std::env::var(key).ok())?;
        config.validate()?;
        Ok(config)
    }

    /// Loads configuration from `path` or, when `path` is `None`, from the
    /// `EMUSIC_SERVER_CONFIG` variable, falling back to defaults.
    pub fn load_or_default(path: Option<&Path>) -> Result<Self> {
        match path.map(Path::to_path_buf).or_else(env_config_path) {
            Some(path) => Self::load(&path),
            None => {
                let mut config = Self::default();
                config.apply_env_with(|key| std::env::var(key).ok())?;
                config.validate()?;
                Ok(config)
            }
        }
    }

    /// Parses configuration from a TOML string (without env overrides).
    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|error| ServerError::Config(error.to_string()))
    }

    /// Applies `EMUSIC_SERVER_*` overrides using `get` as the environment
    /// lookup, which keeps the logic testable without mutating the process
    /// environment.
    pub fn apply_env_with(&mut self, get: impl Fn(&str) -> Option<String>) -> Result<()> {
        if let Some(value) = get("EMUSIC_SERVER_HOST") {
            self.server.host = value;
        }
        if let Some(value) = get("EMUSIC_SERVER_PORT") {
            self.server.port = parse_env(&value, "EMUSIC_SERVER_PORT")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_DATA_DIR") {
            self.server.data_dir = PathBuf::from(value);
        }
        if let Some(value) = get("EMUSIC_SERVER_TRUSTED_PROXIES") {
            self.server.trusted_proxies = split_list(&value);
        }
        if let Some(value) = get("EMUSIC_SERVER_TLS_CERT") {
            self.security.tls_cert = value;
        }
        if let Some(value) = get("EMUSIC_SERVER_TLS_KEY") {
            self.security.tls_key = value;
        }
        if let Some(value) = get("EMUSIC_SERVER_TOKEN_TTL_HOURS") {
            self.security.token_ttl_hours = parse_env(&value, "EMUSIC_SERVER_TOKEN_TTL_HOURS")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_MAX_PAIRING_ATTEMPTS") {
            self.security.max_pairing_attempts_per_min =
                parse_env(&value, "EMUSIC_SERVER_MAX_PAIRING_ATTEMPTS")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_PAIRING_CODE_TTL") {
            self.security.pairing_code_ttl_secs =
                parse_env(&value, "EMUSIC_SERVER_PAIRING_CODE_TTL")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_MAX_BODY_BYTES") {
            self.security.max_body_bytes = parse_env(&value, "EMUSIC_SERVER_MAX_BODY_BYTES")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_LIBRARY_PATHS") {
            self.library.paths = split_list(&value).into_iter().map(PathBuf::from).collect();
        }
        if let Some(value) = get("EMUSIC_SERVER_HVSC_SONGLENGTHS") {
            self.library.hvsc_songlengths_path = if value.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
        if let Some(value) = get("EMUSIC_SERVER_SCAN_INTERVAL") {
            self.library.scan_interval_secs = parse_env(&value, "EMUSIC_SERVER_SCAN_INTERVAL")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_ENABLED") {
            self.render.enabled = parse_env(&value, "EMUSIC_SERVER_RENDER_ENABLED")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_CODEC") {
            self.render.codec = value.trim().to_ascii_lowercase();
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_SAMPLE_RATE") {
            self.render.sample_rate = parse_env(&value, "EMUSIC_SERVER_RENDER_SAMPLE_RATE")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_SOUNDFONT") {
            self.render.soundfont_path = if value.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_CACHE_MAX_BYTES") {
            self.render.cache_max_bytes =
                parse_env(&value, "EMUSIC_SERVER_RENDER_CACHE_MAX_BYTES")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_RENDER_MAX_CONCURRENT") {
            self.render.max_concurrent = parse_env(&value, "EMUSIC_SERVER_RENDER_MAX_CONCURRENT")?;
        }
        Ok(())
    }

    /// Rejects configurations that would be unsafe or unusable at runtime.
    pub fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(ServerError::Config("server.port must not be 0".into()));
        }
        if self.server.host.trim().is_empty() {
            return Err(ServerError::Config("server.host must not be empty".into()));
        }
        if self.server.data_dir.as_os_str().is_empty() {
            return Err(ServerError::Config(
                "server.data_dir must not be empty".into(),
            ));
        }
        for entry in &self.server.trusted_proxies {
            parse_proxy(entry).map_err(|error| {
                ServerError::Config(format!("invalid trusted proxy {entry:?}: {error}"))
            })?;
        }

        let cert_set = !self.security.tls_cert.trim().is_empty();
        let key_set = !self.security.tls_key.trim().is_empty();
        if cert_set != key_set {
            return Err(ServerError::Config(
                "security.tls_cert and security.tls_key must both be set or both be empty".into(),
            ));
        }
        if self.security.token_ttl_hours == 0 {
            return Err(ServerError::Config(
                "security.token_ttl_hours must be greater than 0".into(),
            ));
        }
        if self.security.token_ttl_hours > MAX_TOKEN_TTL_HOURS {
            return Err(ServerError::Config(format!(
                "security.token_ttl_hours must not exceed {MAX_TOKEN_TTL_HOURS}"
            )));
        }
        if self.security.max_pairing_attempts_per_min == 0 {
            return Err(ServerError::Config(
                "security.max_pairing_attempts_per_min must be greater than 0".into(),
            ));
        }
        if self.security.pairing_code_ttl_secs == 0 {
            return Err(ServerError::Config(
                "security.pairing_code_ttl_secs must be greater than 0".into(),
            ));
        }
        if self.security.pairing_code_ttl_secs > MAX_PAIRING_CODE_TTL_SECS {
            return Err(ServerError::Config(format!(
                "security.pairing_code_ttl_secs must not exceed {MAX_PAIRING_CODE_TTL_SECS}"
            )));
        }
        if self.security.max_body_bytes == 0 {
            return Err(ServerError::Config(
                "security.max_body_bytes must be greater than 0".into(),
            ));
        }
        if self.security.max_body_bytes > MAX_BODY_BYTES {
            return Err(ServerError::Config(format!(
                "security.max_body_bytes must not exceed {MAX_BODY_BYTES}"
            )));
        }

        if !self.render.codec.eq_ignore_ascii_case(RENDER_CODEC_FLAC) {
            return Err(ServerError::Config(format!(
                "render.codec must be {RENDER_CODEC_FLAC:?}"
            )));
        }
        if self.render.sample_rate == 0 {
            return Err(ServerError::Config(
                "render.sample_rate must be greater than 0".into(),
            ));
        }
        if self.render.sample_rate > MAX_RENDER_SAMPLE_RATE {
            return Err(ServerError::Config(format!(
                "render.sample_rate must not exceed {MAX_RENDER_SAMPLE_RATE}"
            )));
        }
        if self.render.cache_max_bytes == 0 {
            return Err(ServerError::Config(
                "render.cache_max_bytes must be greater than 0".into(),
            ));
        }
        if self.render.cache_max_bytes > MAX_RENDER_CACHE_BYTES {
            return Err(ServerError::Config(format!(
                "render.cache_max_bytes must not exceed {MAX_RENDER_CACHE_BYTES}"
            )));
        }
        if self.render.max_concurrent == 0 {
            return Err(ServerError::Config(
                "render.max_concurrent must be greater than 0".into(),
            ));
        }
        if self.render.max_concurrent > MAX_RENDER_CONCURRENT {
            return Err(ServerError::Config(format!(
                "render.max_concurrent must not exceed {MAX_RENDER_CONCURRENT}"
            )));
        }

        if self.library.paths.is_empty() {
            return Err(ServerError::Config(
                "library.paths must contain at least one root".into(),
            ));
        }
        for root in &self.library.paths {
            if root.as_os_str().is_empty() {
                return Err(ServerError::Config(
                    "library.paths contains an empty entry".into(),
                ));
            }
        }
        Ok(())
    }

    /// Whether direct TLS should be enabled (both cert and key present).
    pub fn tls_enabled(&self) -> bool {
        !self.security.tls_cert.trim().is_empty() && !self.security.tls_key.trim().is_empty()
    }

    /// Parses `trusted_proxies` into typed networks.
    pub fn trusted_proxy_nets(&self) -> Result<Vec<IpNet>> {
        self.server
            .trusted_proxies
            .iter()
            .map(|entry| {
                parse_proxy(entry).map_err(|error| {
                    ServerError::Config(format!("invalid trusted proxy {entry:?}: {error}"))
                })
            })
            .collect()
    }

    /// Whether `ip` is covered by any configured trusted-proxy network.
    pub fn is_trusted_proxy(&self, nets: &[IpNet], ip: IpAddr) -> bool {
        nets.iter().any(|net| match (net, ip) {
            (IpNet::V4(net), IpAddr::V4(ip)) => net.contains(&ip),
            (IpNet::V6(net), IpAddr::V6(ip)) => net.contains(&ip),
            _ => false,
        })
    }
}

/// The platform default for the server data directory.
pub fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("emusic-server")
    } else if cfg!(target_os = "macos") {
        // A server on macOS is usually run by a user, not as a system
        // service, so `/var/lib` is neither writable nor idiomatic there.
        dirs::data_local_dir()
            .map(|dir| dir.join("emusic-server"))
            .unwrap_or_else(|| PathBuf::from("/usr/local/var/emusic-server"))
    } else {
        PathBuf::from("/var/lib/emusic-server")
    }
}

fn env_config_path() -> Option<PathBuf> {
    std::env::var_os("EMUSIC_SERVER_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn parse_proxy(entry: &str) -> std::result::Result<IpNet, String> {
    let trimmed = entry.trim();
    if let Ok(net) = trimmed.parse::<IpNet>() {
        return Ok(net);
    }
    trimmed
        .parse::<IpAddr>()
        .map(IpNet::from)
        .map_err(|error| error.to_string())
}

fn parse_env<T: std::str::FromStr>(value: &str, key: &str) -> Result<T> {
    value
        .trim()
        .parse::<T>()
        .map_err(|_| ServerError::Config(format!("{key} is not a valid value: {value:?}")))
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
