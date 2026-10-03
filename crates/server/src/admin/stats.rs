//! Read-only database statistics for the admin page.
//!
//! Everything here is synchronous; call it from `spawn_blocking`.

use std::path::Path;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::db::{DB_FILE, Db};
use crate::error::Result;

/// Row count of one table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TableCount {
    /// Table name.
    pub table: String,
    /// Number of rows.
    pub rows: i64,
}

/// On-disk size of the database files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FileSizes {
    /// Main database file, in bytes.
    pub db_bytes: u64,
    /// Write-ahead log, in bytes (0 when absent).
    pub wal_bytes: u64,
}

/// Pairing-code rows by state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PairingCodeStats {
    /// All rows.
    pub total: i64,
    /// Unused and unexpired.
    pub outstanding: i64,
    /// Already used.
    pub used: i64,
    /// Unused but expired (purged on the next code generation).
    pub expired: i64,
}

/// Tombstone summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TombstoneStats {
    /// Number of tombstones.
    pub count: i64,
    /// Oldest deletion time (Unix seconds), if any.
    pub oldest_deleted_at: Option<i64>,
}

/// SQLite page accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PageStats {
    /// Page size in bytes.
    pub page_size: i64,
    /// Pages in the database.
    pub page_count: i64,
    /// Unused pages.
    pub freelist_count: i64,
}

/// `PRAGMA user_version`.
pub fn schema_version(db: &Db) -> Result<i64> {
    let conn = db.conn()?;
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Row counts for every user table, by name.
pub fn table_counts(db: &Db) -> Result<Vec<TableCount>> {
    let conn = db.conn()?;
    let names = {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        stmt.query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    names
        .into_iter()
        .map(|table| {
            // Names come from sqlite_master, not from a request; quote them
            // anyway so an odd name cannot break the statement.
            let quoted = table.replace('"', "\"\"");
            let rows =
                conn.query_row(&format!("SELECT COUNT(*) FROM \"{quoted}\""), [], |row| {
                    row.get(0)
                })?;
            Ok(TableCount { table, rows })
        })
        .collect()
}

/// Sizes of the database and WAL files under `data_dir`.
pub fn file_sizes(data_dir: &Path) -> FileSizes {
    let size = |name: String| {
        std::fs::metadata(data_dir.join(name))
            .map(|meta| meta.len())
            .unwrap_or(0)
    };
    FileSizes {
        db_bytes: size(DB_FILE.to_string()),
        wal_bytes: size(format!("{DB_FILE}-wal")),
    }
}

/// Pairing-code rows by state at `now`.
pub fn pairing_code_stats(db: &Db, now: i64) -> Result<PairingCodeStats> {
    let conn = db.conn()?;
    Ok(conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(used = 0 AND expires_at > ?1), 0),
                COALESCE(SUM(used <> 0), 0),
                COALESCE(SUM(used = 0 AND expires_at <= ?1), 0)
         FROM pairing_codes",
        [now],
        |row| {
            Ok(PairingCodeStats {
                total: row.get(0)?,
                outstanding: row.get(1)?,
                used: row.get(2)?,
                expired: row.get(3)?,
            })
        },
    )?)
}

/// Tombstone count and age.
pub fn tombstone_stats(db: &Db) -> Result<TombstoneStats> {
    let conn = db.conn()?;
    Ok(conn.query_row(
        "SELECT COUNT(*), MIN(deleted_at) FROM tombstones",
        [],
        |row| {
            Ok(TombstoneStats {
                count: row.get(0)?,
                oldest_deleted_at: row.get(1)?,
            })
        },
    )?)
}

/// Page size, page count and free pages.
pub fn page_stats(db: &Db) -> Result<PageStats> {
    let conn = db.conn()?;
    let pragma = |name: &str| -> rusqlite::Result<i64> {
        conn.pragma_query_value(None, name, |row| row.get(0))
    };
    Ok(PageStats {
        page_size: pragma("page_size")?,
        page_count: pragma("page_count")?,
        freelist_count: pragma("freelist_count")?,
    })
}

/// Runs `PRAGMA quick_check` (`full = false`) or `PRAGMA integrity_check`
/// and returns its messages; `["ok"]` means healthy. At most 100 lines.
pub fn check_integrity(db: &Db, full: bool) -> Result<Vec<String>> {
    let conn = db.conn()?;
    let sql = if full {
        "PRAGMA integrity_check(100)"
    } else {
        "PRAGMA quick_check(100)"
    };
    let mut stmt = conn.prepare(sql)?;
    let lines = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(lines)
}

/// A track's display title (`artist - title`, falling back to the path).
pub fn track_label(db: &Db, track_id: &str) -> Result<Option<String>> {
    let conn = db.conn()?;
    let row = conn
        .query_row(
            "SELECT title, artist, relative_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(title, artist, path)| match (artist, title) {
        (Some(artist), Some(title)) => format!("{artist} - {title}"),
        (None, Some(title)) => title,
        _ => path,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_reflect_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path()).unwrap();
        db.insert_pairing_code("aa", 0, 10).unwrap();
        db.insert_pairing_code("bb", 0, i64::MAX).unwrap();

        assert_eq!(
            schema_version(&db).unwrap(),
            crate::db::schema::CURRENT_VERSION
        );
        let counts = table_counts(&db).unwrap();
        let codes = counts.iter().find(|c| c.table == "pairing_codes").unwrap();
        assert_eq!(codes.rows, 2);
        assert!(counts.iter().any(|c| c.table == "audit_log"));

        let stats = pairing_code_stats(&db, 100).unwrap();
        assert_eq!((stats.total, stats.outstanding, stats.expired), (2, 1, 1));
        assert_eq!(tombstone_stats(&db).unwrap().count, 0);
        assert_eq!(check_integrity(&db, false).unwrap(), ["ok"]);
        assert_eq!(check_integrity(&db, true).unwrap(), ["ok"]);
        assert!(file_sizes(dir.path()).db_bytes > 0);
        assert!(page_stats(&db).unwrap().page_size > 0);
    }
}
