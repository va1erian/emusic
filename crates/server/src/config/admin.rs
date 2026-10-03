//! `[admin]` and `[audit]` configuration tables.
//!
//! The admin page is served by a second listener that is never mounted on the
//! public router. It binds to loopback by default; binding anywhere else
//! requires a long shared token so an exposed port is never an open door.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::parse_env;
use crate::error::{Result, ServerError};

/// Default admin listener port.
pub const DEFAULT_ADMIN_PORT: u16 = 8081;

/// Minimum length of `admin.token`.
pub const MIN_ADMIN_TOKEN_LEN: usize = 24;

/// Upper bound for `audit.retention_days` (ten years).
pub const MAX_AUDIT_RETENTION_DAYS: u32 = 3650;

/// `[admin]` table.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct AdminConfig {
    /// Whether the admin listener runs at all.
    pub enabled: bool,
    /// Interface the admin listener binds. Must be an IP literal or
    /// `localhost`; anything but loopback requires `token`.
    pub host: String,
    /// Admin listener port.
    pub port: u16,
    /// Shared secret for HTTP Basic auth (any user name, password = token).
    /// Empty disables auth, which is only allowed on loopback.
    pub token: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: DEFAULT_ADMIN_PORT,
            token: String::new(),
        }
    }
}

// Hand-written so the token never ends up in a log line or panic message.
impl std::fmt::Debug for AdminConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminConfig")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field(
                "token",
                &if self.token.is_empty() {
                    ""
                } else {
                    "<redacted>"
                },
            )
            .finish()
    }
}

impl AdminConfig {
    /// Whether HTTP Basic auth is required.
    pub fn requires_auth(&self) -> bool {
        !self.token.is_empty()
    }

    /// The bind address, or `None` when `host` is not an IP or `localhost`.
    pub fn ip(&self) -> Option<IpAddr> {
        let host = self.host.trim();
        if host.eq_ignore_ascii_case("localhost") {
            return Some(IpAddr::from([127, 0, 0, 1]));
        }
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .ok()
    }

    pub(super) fn apply_env(&mut self, get: &impl Fn(&str) -> Option<String>) -> Result<()> {
        if let Some(value) = get("EMUSIC_SERVER_ADMIN_ENABLED") {
            self.enabled = parse_env(&value, "EMUSIC_SERVER_ADMIN_ENABLED")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_ADMIN_HOST") {
            self.host = value.trim().to_string();
        }
        if let Some(value) = get("EMUSIC_SERVER_ADMIN_PORT") {
            self.port = parse_env(&value, "EMUSIC_SERVER_ADMIN_PORT")?;
        }
        if let Some(value) = get("EMUSIC_SERVER_ADMIN_TOKEN") {
            self.token = value.trim().to_string();
        }
        Ok(())
    }

    /// Refuses an admin listener that would be reachable without a token.
    pub(super) fn validate(&self, public_port: u16) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.port == 0 {
            return Err(ServerError::Config("admin.port must not be 0".into()));
        }
        if self.port == public_port {
            return Err(ServerError::Config(
                "admin.port must differ from server.port".into(),
            ));
        }
        let Some(ip) = self.ip() else {
            return Err(ServerError::Config(format!(
                "admin.host must be an IP address or \"localhost\", got {:?}",
                self.host
            )));
        };
        if !self.token.is_empty() && self.token.chars().count() < MIN_ADMIN_TOKEN_LEN {
            return Err(ServerError::Config(format!(
                "admin.token must be at least {MIN_ADMIN_TOKEN_LEN} characters"
            )));
        }
        if self.token.chars().any(char::is_control) {
            return Err(ServerError::Config(
                "admin.token must not contain control characters".into(),
            ));
        }
        if !ip.is_loopback() && self.token.is_empty() {
            return Err(ServerError::Config(format!(
                "admin.host {ip} is not loopback: set admin.token \
                 (at least {MIN_ADMIN_TOKEN_LEN} characters) or bind 127.0.0.1"
            )));
        }
        Ok(())
    }
}

/// `[audit]` table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct AuditConfig {
    /// How long rows are kept in the `audit_log` table, in days.
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self { retention_days: 90 }
    }
}

impl AuditConfig {
    /// The retention window in seconds.
    pub fn retention_secs(&self) -> i64 {
        i64::from(self.retention_days) * 24 * 3600
    }

    pub(super) fn apply_env(&mut self, get: &impl Fn(&str) -> Option<String>) -> Result<()> {
        if let Some(value) = get("EMUSIC_SERVER_AUDIT_RETENTION_DAYS") {
            self.retention_days = parse_env(&value, "EMUSIC_SERVER_AUDIT_RETENTION_DAYS")?;
        }
        Ok(())
    }

    pub(super) fn validate(&self) -> Result<()> {
        if self.retention_days == 0 || self.retention_days > MAX_AUDIT_RETENTION_DAYS {
            return Err(ServerError::Config(format!(
                "audit.retention_days must be between 1 and {MAX_AUDIT_RETENTION_DAYS}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    const MINIMAL: &str = "[library]\npaths = [\"/media/music\"]\n";

    fn minimal() -> Config {
        Config::from_toml(MINIMAL).expect("parse")
    }

    #[test]
    fn admin_defaults_to_loopback_without_a_token() {
        let config = minimal();
        assert!(config.admin.enabled);
        assert_eq!(config.admin.host, "127.0.0.1");
        assert_eq!(config.admin.port, DEFAULT_ADMIN_PORT);
        assert!(!config.admin.requires_auth());
        assert_eq!(config.audit.retention_days, 90);
        config.validate().expect("valid");
    }

    #[test]
    fn non_loopback_admin_without_a_token_is_refused() {
        let mut config = minimal();
        config.admin.host = "0.0.0.0".into();
        let error = config.validate().expect_err("must refuse").to_string();
        assert!(error.contains("admin.token"), "{error}");

        config.admin.token = "short".into();
        assert!(config.validate().is_err(), "a short token is refused");

        config.admin.token = "x".repeat(MIN_ADMIN_TOKEN_LEN);
        config
            .validate()
            .expect("a long token unlocks non-loopback");

        config.admin.enabled = false;
        config.admin.token.clear();
        config
            .validate()
            .expect("a disabled listener needs nothing");
    }

    #[test]
    fn admin_host_must_be_an_ip_or_localhost() {
        let mut config = minimal();
        for ok in ["localhost", "::1", "[::1]", "127.0.0.1"] {
            config.admin.host = ok.into();
            config.validate().unwrap_or_else(|e| panic!("{ok}: {e}"));
        }
        config.admin.host = "admin.example.com".into();
        assert!(config.validate().is_err());
        config.admin.host = "127.0.0.1".into();
        config.admin.port = config.server.port;
        assert!(config.validate().is_err(), "ports must differ");
    }

    #[test]
    fn admin_and_audit_env_overrides_apply() {
        let mut config = minimal();
        let token = "t".repeat(32);
        let vars = [
            ("EMUSIC_SERVER_ADMIN_ENABLED", "true"),
            ("EMUSIC_SERVER_ADMIN_HOST", "0.0.0.0"),
            ("EMUSIC_SERVER_ADMIN_PORT", "9091"),
            ("EMUSIC_SERVER_ADMIN_TOKEN", token.as_str()),
            ("EMUSIC_SERVER_AUDIT_RETENTION_DAYS", "30"),
        ];
        config
            .apply_env_with(|key| {
                vars.iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| (*value).to_string())
            })
            .expect("apply");
        assert_eq!(config.admin.host, "0.0.0.0");
        assert_eq!(config.admin.port, 9091);
        assert_eq!(config.admin.token, token);
        assert_eq!(config.audit.retention_days, 30);
        config.validate().expect("valid");
        assert!(
            !format!("{config:?}").contains(&token),
            "Debug redacts the token"
        );

        config.audit.retention_days = 0;
        assert!(config.validate().is_err());
    }
}
