//! Audit-log retention: prune at startup, then once a day.

use std::time::Duration;

use super::writer::AuditLog;
use crate::db::Db;
use crate::util::unix_now;

/// How often the retention task runs after startup.
pub const PRUNE_INTERVAL: Duration = Duration::from_secs(24 * 3600);

/// Deletes rows older than `retention_days` before `now`, auditing the prune
/// when anything was removed. Returns the number of rows deleted.
pub fn prune(db: &Db, audit: &AuditLog, retention_days: u32, now: i64) -> crate::Result<usize> {
    let cutoff = now.saturating_sub(i64::from(retention_days) * 24 * 3600);
    let removed = db.prune_audit_before(cutoff)?;
    if removed > 0 {
        audit.audit_pruned(removed, retention_days);
    }
    Ok(removed)
}

/// Spawns the daily retention task. The first tick runs immediately, which
/// is the startup prune.
pub fn spawn_daily(db: Db, audit: AuditLog, retention_days: u32) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(PRUNE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let (db, audit) = (db.clone(), audit.clone());
            let result =
                tokio::task::spawn_blocking(move || prune(&db, &audit, retention_days, unix_now()))
                    .await;
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => tracing::error!(%error, "audit retention failed"),
                Err(error) => tracing::error!(%error, "audit retention task panicked"),
            }
        }
    });
}
