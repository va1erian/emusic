//! Structured audit logging for security-relevant and operational events.
//!
//! Operational logs go to stdout; audit events additionally go to a JSON log
//! file under the data directory, one JSON object per line, and to the
//! `audit_log` table so the admin page and `emusic-server audit` can query
//! them. Every event flows through an [`AuditLog`] handle (held by
//! `AppState`): its typed helpers fix each event's name and fields, and
//! [`AuditRecord`] covers anything else. Persistence is asynchronous and
//! lossy under extreme load (see [`writer`]), so recording an event never
//! blocks or fails a request.

mod events;
pub mod record;
pub mod retention;
pub mod store;
pub mod writer;

#[cfg(test)]
mod tests;

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub use events::{KNOWN_EVENTS, PER_CLIENT_LIMIT};
pub use record::{Actor, AuditLevel, AuditRecord, CLI_SOURCE, PairFailure, SYSTEM_SOURCE};
pub use store::{AuditQuery, AuditRow};
pub use writer::AuditLog;

/// Tracing target reserved for audit events.
pub const AUDIT_TARGET: &str = "emusic_server::audit";

/// Initializes logging. Keep the returned guard alive for the process
/// lifetime; dropping it stops the audit writer.
pub fn init(data_dir: &Path) -> std::io::Result<WorkerGuard> {
    std::fs::create_dir_all(data_dir)?;
    let file_appender = tracing_appender::rolling::daily(data_dir, "audit.log");
    // Security events must not be dropped under load: block the audit event
    // producer rather than lose a record (`lossy(false)`).
    let (audit_writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .finish(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,emusic_server=debug"));

    let console = tracing_subscriber::fmt::layer().with_target(true).compact();
    let audit = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(audit_writer)
        .with_target(false)
        .with_current_span(false)
        .with_filter(EnvFilter::new(format!("{AUDIT_TARGET}=info")));

    tracing_subscriber::registry()
        .with(filter)
        .with(console)
        .with(audit)
        .init();
    Ok(guard)
}
