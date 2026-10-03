//! The audit record type and the small enums that describe who did what.

use serde_json::{Map, Value};

use crate::util::unix_now;

/// `client_ip` recorded for events the server raises on its own (scans,
/// migrations, retention).
pub const SYSTEM_SOURCE: &str = "system";

/// `client_ip` recorded for events raised by an `emusic-server` CLI command.
pub const CLI_SOURCE: &str = "cli";

/// Severity of an audit event, mirrored onto the tracing level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditLevel {
    /// Routine, expected activity.
    Info,
    /// A rejected or security-relevant action.
    Warn,
}

/// One audit event: the tracing line and the `audit_log` row are both built
/// from it.
///
/// Build one with [`AuditRecord::new`] and the chained setters, then hand it
/// to [`crate::audit::AuditLog::record`]. Prefer the typed helpers on
/// `AuditLog` for the events they cover.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditRecord {
    /// Unix timestamp (seconds).
    pub at: i64,
    /// Event name, a short `snake_case` identifier.
    pub event: &'static str,
    /// Severity.
    pub level: AuditLevel,
    /// Real client address, or [`SYSTEM_SOURCE`] / [`CLI_SOURCE`].
    pub client_ip: String,
    /// The device the event concerns, when there is one.
    pub device_id: Option<String>,
    /// Event-specific fields, stored as a JSON object.
    pub detail: Map<String, Value>,
}

impl AuditRecord {
    /// A new informational event raised now by the server itself.
    pub fn new(event: &'static str) -> Self {
        Self {
            at: unix_now(),
            event,
            level: AuditLevel::Info,
            client_ip: SYSTEM_SOURCE.to_string(),
            device_id: None,
            detail: Map::new(),
        }
    }

    /// Marks the event as a warning.
    pub fn warn(mut self) -> Self {
        self.level = AuditLevel::Warn;
        self
    }

    /// Sets the client address (or source label).
    pub fn ip(mut self, client_ip: impl Into<String>) -> Self {
        self.client_ip = client_ip.into();
        self
    }

    /// Sets the device the event concerns.
    pub fn device(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// Adds a detail field.
    pub fn field(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.detail.insert(key.to_string(), value.into());
        self
    }

    /// The detail object serialized as JSON text.
    pub fn detail_json(&self) -> String {
        Value::Object(self.detail.clone()).to_string()
    }
}

/// Who initiated an administrative action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// The `emusic-server` command line.
    Cli,
    /// The admin page.
    Admin,
    /// A paired device, through the public API.
    Device(String),
}

impl Actor {
    /// Stable label stored in the `by` detail field.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Admin => "admin",
            Self::Device(_) => "device",
        }
    }

    /// Adds `by` (and `by_device`) to `record`.
    pub(crate) fn annotate(&self, record: AuditRecord) -> AuditRecord {
        let record = record.field("by", self.label());
        match self {
            Self::Device(id) => record.field("by_device", id.as_str()),
            _ => record,
        }
    }
}

/// Why a pairing attempt was refused. The client always sees the same `401`;
/// only the audit trail tells these apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairFailure {
    /// No code matched.
    Wrong,
    /// The code matched but had expired.
    Expired,
    /// The code matched but was already used.
    Reused,
    /// The client hit the pairing rate limit.
    RateLimited,
    /// The request was malformed (bad name, key or code format).
    InvalidRequest,
}

impl PairFailure {
    /// Stable label stored in the `reason` detail field.
    pub fn label(self) -> &'static str {
        match self {
            Self::Wrong => "wrong",
            Self::Expired => "expired",
            Self::Reused => "reused",
            Self::RateLimited => "rate_limited",
            Self::InvalidRequest => "invalid_request",
        }
    }
}
