//! The server-wide starred-track set (#513).
//!
//! The server is single-user, so there is one set shared by every paired
//! device. Rows are keyed by the server track id and deliberately survive a
//! scan deleting the track (the file may come back); reads only list ids that
//! currently exist in `tracks`. Every effective change bumps
//! `meta.starred_version`, which clients use as an ETag.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::db::Db;
use crate::db::schema::META_STARRED_VERSION;
use crate::error::{Result, ServerError};
use crate::util::unix_now;

/// One starred track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarredEntry {
    /// Server track id.
    pub track_id: String,
    /// Unix timestamp (seconds) when the track was starred.
    pub starred_at: i64,
}

/// The starred set at one version, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarredSnapshot {
    /// The starred-set version.
    pub version: i64,
    /// Starred tracks that currently exist, newest first.
    pub entries: Vec<StarredEntry>,
}

/// The outcome of [`Db::apply_starred`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StarredUpdate {
    /// The starred-set version after the change.
    pub version: i64,
    /// Ids that became starred.
    pub starred: Vec<String>,
    /// Ids that became unstarred.
    pub unstarred: Vec<String>,
    /// Ids asked to be starred that are not in the library (skipped).
    pub unknown: Vec<String>,
}

impl StarredUpdate {
    /// Whether the set actually changed.
    pub fn changed(&self) -> bool {
        !self.starred.is_empty() || !self.unstarred.is_empty()
    }
}

impl Db {
    /// The current starred-set version.
    pub fn starred_version(&self) -> Result<i64> {
        let conn = self.conn()?;
        starred_version(&conn)
    }

    /// The starred tracks that currently exist in the library, newest first.
    pub fn starred(&self) -> Result<StarredSnapshot> {
        let mut conn = self.conn()?;
        // One read transaction so the version matches the rows.
        let tx = conn.transaction()?;
        let version = starred_version(&tx)?;
        let entries = {
            let mut stmt = tx.prepare(
                "SELECT s.track_id, s.starred_at FROM starred s
                 JOIN tracks t ON t.id = s.track_id
                 ORDER BY s.starred_at DESC, s.rowid DESC",
            )?;
            stmt.query_map([], |row| {
                Ok(StarredEntry {
                    track_id: row.get(0)?,
                    starred_at: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        Ok(StarredSnapshot { version, entries })
    }

    /// Stars and unstars tracks in one transaction. Stars are applied first,
    /// then unstars, so an id in both lists ends up unstarred.
    ///
    /// Starring is idempotent (an existing star keeps its timestamp) and only
    /// accepts ids present in `tracks`; others are reported in `unknown`.
    /// Unstarring is idempotent and also removes rows kept for deleted
    /// tracks. The version is bumped once, and only if something changed.
    pub fn apply_starred(&self, star: &[String], unstar: &[String]) -> Result<StarredUpdate> {
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = unix_now();
        let mut update = StarredUpdate::default();
        let mut seen = HashSet::new();
        {
            let mut exists = tx.prepare("SELECT 1 FROM tracks WHERE id = ?1")?;
            let mut insert =
                tx.prepare("INSERT OR IGNORE INTO starred (track_id, starred_at) VALUES (?1, ?2)")?;
            for id in star.iter().filter(|id| seen.insert(id.as_str())) {
                if exists.query_row([id], |_| Ok(())).optional()?.is_none() {
                    update.unknown.push(id.clone());
                } else if insert.execute(rusqlite::params![id, now])? > 0 {
                    update.starred.push(id.clone());
                }
            }
            let mut delete = tx.prepare("DELETE FROM starred WHERE track_id = ?1")?;
            seen.clear();
            for id in unstar.iter().filter(|id| seen.insert(id.as_str())) {
                if delete.execute([id])? > 0 {
                    update.unstarred.push(id.clone());
                }
            }
        }
        // A star then unstar of the same id in one batch is a net no-op.
        let net_noop: HashSet<String> = update
            .starred
            .iter()
            .filter(|id| update.unstarred.contains(id))
            .cloned()
            .collect();
        update.starred.retain(|id| !net_noop.contains(id));
        update.unstarred.retain(|id| !net_noop.contains(id));
        let mut version = starred_version(&tx)?;
        if update.changed() {
            version += 1;
            tx.execute(
                "UPDATE meta SET value = ?1 WHERE key = ?2",
                rusqlite::params![version.to_string(), META_STARRED_VERSION],
            )?;
        }
        tx.commit()?;
        update.version = version;
        Ok(update)
    }
}

fn starred_version(conn: &rusqlite::Connection) -> Result<i64> {
    let value: String = conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        [META_STARRED_VERSION],
        |row| row.get(0),
    )?;
    value
        .parse::<i64>()
        .map_err(|_| ServerError::Metadata(format!("starred_version {value:?} is not an integer")))
}
