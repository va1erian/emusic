//! Typed audit helpers.
//!
//! Each helper fixes the event name, level and field set, so call sites cannot
//! emit a malformed event. Events not covered here can be built with
//! [`AuditRecord`] and passed to [`AuditLog::record`].

use super::record::{Actor, AuditRecord, PairFailure};
use super::writer::AuditLog;
use crate::db::Migration;
use crate::scanner::ScanReport;

/// Rows per minute per client persisted for events anyone can trigger.
pub const PER_CLIENT_LIMIT: u32 = 30;

/// Event names this crate emits, for filter menus and tests.
pub const KNOWN_EVENTS: &[&str] = &[
    "admin_auth_failed",
    "admin_integrity_check",
    "audit_events_dropped",
    "audit_pruned",
    "auth_failed",
    "device_paired",
    "device_revoked",
    "library_version_changed",
    "migration_applied",
    "pair_failed",
    "pairing_code_created",
    "path_violation",
    "rate_limited",
    "refresh_failed",
    "scan_failed",
    "scan_finished",
    "scan_started",
    "starred_changed",
    "token_refreshed",
    "tombstones_pruned",
];

impl AuditLog {
    /// A failed authentication attempt: bad token, revoked device.
    pub fn auth_failed(&self, client_ip: &str, reason: &str) {
        self.record_per_client(
            AuditRecord::new("auth_failed")
                .warn()
                .ip(client_ip)
                .field("reason", reason),
            PER_CLIENT_LIMIT,
        );
    }

    /// A refused pairing attempt.
    pub fn pair_failed(&self, client_ip: &str, reason: PairFailure) {
        self.record_per_client(
            AuditRecord::new("pair_failed")
                .warn()
                .ip(client_ip)
                .field("reason", reason.label()),
            PER_CLIENT_LIMIT,
        );
    }

    /// A device successfully paired.
    pub fn device_paired(&self, client_ip: &str, device_id: &str, device_name: &str) {
        self.record(
            AuditRecord::new("device_paired")
                .ip(client_ip)
                .device(device_id)
                .field("device_name", device_name),
        );
    }

    /// A one-time pairing code was minted by `by`.
    pub fn pairing_code_created(&self, client_ip: &str, by: &Actor, ttl_secs: u64) {
        let mut record = AuditRecord::new("pairing_code_created")
            .ip(client_ip)
            .field("ttl_secs", ttl_secs);
        if let Actor::Device(id) = by {
            record = record.device(id.as_str());
        }
        self.record(by.annotate(record));
    }

    /// `device_id` was revoked by `by`.
    pub fn device_revoked(&self, client_ip: &str, device_id: &str, by: &Actor) {
        let record = AuditRecord::new("device_revoked")
            .warn()
            .ip(client_ip)
            .device(device_id);
        self.record(by.annotate(record));
    }

    /// A token was refreshed after a successful proof of possession.
    pub fn token_refreshed(&self, client_ip: &str, device_id: &str) {
        self.record(
            AuditRecord::new("token_refreshed")
                .ip(client_ip)
                .device(device_id),
        );
    }

    /// A refresh was refused for an otherwise authenticated device.
    pub fn refresh_failed(&self, client_ip: &str, device_id: &str, reason: &str) {
        self.record(
            AuditRecord::new("refresh_failed")
                .warn()
                .ip(client_ip)
                .device(device_id)
                .field("reason", reason),
        );
    }

    /// The server-wide starred set changed: `starred`/`unstarred` count the
    /// ids that actually changed state.
    pub fn starred_changed(
        &self,
        client_ip: &str,
        device_id: &str,
        starred: usize,
        unstarred: usize,
    ) {
        self.record(
            AuditRecord::new("starred_changed")
                .ip(client_ip)
                .device(device_id)
                .field("starred", starred)
                .field("unstarred", unstarred),
        );
    }

    /// A stored path failed the library-root jail.
    pub fn path_violation(&self, client_ip: &str, device_id: &str, track_id: &str) {
        self.record(
            AuditRecord::new("path_violation")
                .warn()
                .ip(client_ip)
                .device(device_id)
                .field("track_id", track_id),
        );
    }

    /// A request was rejected by a rate limiter.
    pub fn rate_limited(&self, client_ip: &str, scope: &str) {
        self.record_per_client(
            AuditRecord::new("rate_limited")
                .warn()
                .ip(client_ip)
                .field("scope", scope),
            PER_CLIENT_LIMIT,
        );
    }

    /// The schema was upgraded at startup.
    pub fn migration_applied(&self, source: &str, migration: Migration) {
        self.record(
            AuditRecord::new("migration_applied")
                .ip(source)
                .field("from", migration.from)
                .field("to", migration.to),
        );
    }

    /// A library scan started.
    pub fn scan_started(&self, roots: usize) {
        self.record(AuditRecord::new("scan_started").field("roots", roots));
    }

    /// A library scan finished.
    pub fn scan_finished(&self, report: &ScanReport) {
        self.record(
            AuditRecord::new("scan_finished")
                .field("files_found", report.files_found)
                .field("changed", report.tracks_changed)
                .field("deleted", report.tracks_deleted)
                .field("skipped", report.files_skipped)
                .field("partial", report.partial)
                .field("elapsed_ms", report.elapsed_ms),
        );
        if report.tombstones_pruned > 0 {
            self.record(
                AuditRecord::new("tombstones_pruned").field("count", report.tombstones_pruned),
            );
        }
    }

    /// A library scan failed outright.
    pub fn scan_failed(&self, error: &str) {
        self.record(AuditRecord::new("scan_failed").warn().field("error", error));
    }

    /// The library version advanced.
    pub fn library_version_changed(&self, from: i64, to: i64) {
        self.record(
            AuditRecord::new("library_version_changed")
                .field("from", from)
                .field("to", to),
        );
    }

    /// Retention removed old audit rows.
    pub fn audit_pruned(&self, removed: usize, retention_days: u32) {
        self.record(
            AuditRecord::new("audit_pruned")
                .field("removed", removed)
                .field("retention_days", retention_days),
        );
    }

    /// A request to the admin listener failed authentication.
    pub fn admin_auth_failed(&self, client_ip: &str, reason: &str) {
        self.record_per_client(
            AuditRecord::new("admin_auth_failed")
                .warn()
                .ip(client_ip)
                .field("reason", reason),
            PER_CLIENT_LIMIT,
        );
    }

    /// An operator ran `PRAGMA integrity_check` from the admin page.
    pub fn admin_integrity_check(&self, client_ip: &str, ok: bool) {
        let record = AuditRecord::new("admin_integrity_check")
            .ip(client_ip)
            .field("ok", ok);
        self.record(Actor::Admin.annotate(if ok { record } else { record.warn() }));
    }
}
