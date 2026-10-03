//! `audit_log` table queries.

use serde::Serialize;
use serde_json::Value;

use super::record::AuditRecord;
use crate::db::Db;
use crate::error::Result;

/// Largest page [`Db::audit_rows`] returns.
pub const MAX_AUDIT_PAGE: u32 = 500;

/// A persisted audit event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditRow {
    /// Row id; newer rows have larger ids.
    pub id: i64,
    /// Unix timestamp (seconds).
    pub at: i64,
    /// Event name.
    pub event: String,
    /// Client address or source label.
    pub client_ip: String,
    /// The device the event concerns, if any.
    pub device_id: Option<String>,
    /// Event-specific fields.
    pub detail: Value,
}

/// Filters for [`Db::audit_rows`]. Every field is optional; rows come back
/// newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditQuery {
    /// Exact event name.
    pub event: Option<String>,
    /// Exact device id.
    pub device_id: Option<String>,
    /// Only rows at or after this time (Unix seconds).
    pub since: Option<i64>,
    /// Only rows strictly before this time (Unix seconds).
    pub until: Option<i64>,
    /// Keyset cursor: only rows with an id below this one.
    pub before_id: Option<i64>,
    /// Page size, clamped to `1..=MAX_AUDIT_PAGE`.
    pub limit: u32,
}

impl Db {
    /// Inserts `records` in one transaction.
    pub fn insert_audit_records(&self, records: &[AuditRecord]) -> Result<usize> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO audit_log (at, event, client_ip, device_id, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for record in records {
                insert.execute(rusqlite::params![
                    record.at,
                    record.event,
                    record.client_ip,
                    record.device_id,
                    record.detail_json(),
                ])?;
            }
        }
        tx.commit()?;
        Ok(records.len())
    }

    /// Returns one page of audit rows matching `query`, newest first.
    pub fn audit_rows(&self, query: &AuditQuery) -> Result<Vec<AuditRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, at, event, client_ip, device_id, detail FROM audit_log
             WHERE (?1 IS NULL OR event = ?1)
               AND (?2 IS NULL OR device_id = ?2)
               AND (?3 IS NULL OR at >= ?3)
               AND (?4 IS NULL OR at < ?4)
               AND (?5 IS NULL OR id < ?5)
             ORDER BY id DESC
             LIMIT ?6",
        )?;
        let limit = query.limit.clamp(1, MAX_AUDIT_PAGE);
        let rows = stmt.query_map(
            rusqlite::params![
                query.event,
                query.device_id,
                query.since,
                query.until,
                query.before_id,
                limit,
            ],
            |row| {
                let detail: String = row.get(5)?;
                Ok(AuditRow {
                    id: row.get(0)?,
                    at: row.get(1)?,
                    event: row.get(2)?,
                    client_ip: row.get(3)?,
                    device_id: row.get(4)?,
                    detail: serde_json::from_str(&detail).unwrap_or(Value::String(detail)),
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Distinct event names present in the log, for filter menus.
    pub fn audit_event_names(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT DISTINCT event FROM audit_log ORDER BY event")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Deletes audit rows recorded before `cutoff` (Unix seconds).
    pub fn prune_audit_before(&self, cutoff: i64) -> Result<usize> {
        let conn = self.conn()?;
        Ok(conn.execute("DELETE FROM audit_log WHERE at < ?1", [cutoff])?)
    }
}
