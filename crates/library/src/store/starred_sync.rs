//! Persistence for starred-track sync with `emusic-server` (#516).
//!
//! The desktop keeps, per server, the starred set as of the last successful
//! sync (the merge base) plus its version, and stamps every local star change
//! with the time it was made. The UI's sync worker reads these, performs the
//! three-way merge and writes the outcome back in one transaction.

use std::collections::HashSet;

use emusic_core::TrackId;
use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;

/// The fields of one track needed to sync its star with a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarSyncRow {
    /// Library row id.
    pub id: TrackId,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged artist.
    pub artist: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Tagged album.
    pub album: Option<String>,
    /// Tagged track number.
    pub track_no: Option<u32>,
    /// Whether the track is starred.
    pub starred: bool,
    /// When the star was last changed on this desktop (Unix ms), if known.
    pub changed_at_ms: Option<i64>,
    /// The server track id, for a row synced from the queried server; `None`
    /// for a local library track.
    pub remote_track_id: Option<String>,
}

/// The starred set of one server as of the last successful sync.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StarredBase {
    /// The server's starred-set version (its ETag); `None` before the first
    /// sync.
    pub version: Option<i64>,
    /// When local state was read for that sync (Unix ms); local changes made
    /// after this are unsynced.
    pub synced_at_ms: i64,
    /// The starred server track ids.
    pub ids: HashSet<String>,
}

impl Store {
    /// Sets whether `track_id` is starred, stamping an actual change with
    /// `at_ms` (Unix ms). Returns whether a row matched.
    pub fn set_starred_at(&self, track_id: TrackId, starred: bool, at_ms: i64) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE tracks SET
                 starred_changed_at = CASE WHEN starred = ?1 THEN starred_changed_at ELSE ?3 END,
                 starred = ?1
             WHERE id = ?2",
            params![starred, track_id.0, at_ms],
        )?;
        Ok(updated > 0)
    }

    /// The local library tracks plus the rows synced from `server_id`, with
    /// the fields the starred sync needs. Rows of other servers are left out.
    pub fn star_sync_rows(&self, server_id: &str) -> Result<Vec<StarSyncRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, artist, album_artist, album, track_no, starred,
                    starred_changed_at, remote_track_id
             FROM tracks
             WHERE remote_server_id IS NULL OR remote_server_id = ?1",
        )?;
        let rows = stmt.query_map([server_id], |row| {
            Ok(StarSyncRow {
                id: TrackId(row.get(0)?),
                title: row.get(1)?,
                artist: row.get(2)?,
                album_artist: row.get(3)?,
                album: row.get(4)?,
                track_no: row.get(5)?,
                starred: row.get(6)?,
                changed_at_ms: row.get(7)?,
                remote_track_id: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The merge base stored for `server_id` (empty before the first sync).
    pub fn starred_base(&self, server_id: &str) -> Result<StarredBase> {
        let header: Option<(Option<i64>, i64)> = self
            .conn
            .query_row(
                "SELECT starred_version, starred_synced_at FROM remote_servers
                 WHERE server_id = ?1",
                [server_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (version, synced_at_ms) = header.unwrap_or((None, 0));
        let mut stmt = self
            .conn
            .prepare("SELECT track_id FROM remote_starred WHERE server_id = ?1")?;
        let ids = stmt
            .query_map([server_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        Ok(StarredBase {
            version,
            synced_at_ms,
            ids,
        })
    }

    /// Commits one sync of `server_id` atomically: applies `changes` to the
    /// local star flags and stores `base` as the new merge base.
    ///
    /// A change is skipped when the user changed that track's star after
    /// `base.synced_at_ms` (when local state was read), so a click made while
    /// the sync ran is never overwritten; the next sync picks it up. Applied
    /// changes are not stamped, as they are not local edits. Returns the
    /// number of rows changed.
    pub fn commit_star_sync(
        &mut self,
        server_id: &str,
        base: &StarredBase,
        changes: &[(TrackId, bool)],
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut applied = 0;
        {
            let mut update = tx.prepare_cached(
                "UPDATE tracks SET starred = ?1
                 WHERE id = ?2 AND starred != ?1 AND COALESCE(starred_changed_at, 0) <= ?3",
            )?;
            for (id, starred) in changes {
                applied += update.execute(params![starred, id.0, base.synced_at_ms])?;
            }
            tx.execute(
                "INSERT INTO remote_servers (server_id, starred_version, starred_synced_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(server_id) DO UPDATE SET
                     starred_version = excluded.starred_version,
                     starred_synced_at = excluded.starred_synced_at",
                params![server_id, base.version, base.synced_at_ms],
            )?;
            tx.execute(
                "DELETE FROM remote_starred WHERE server_id = ?1",
                [server_id],
            )?;
            let mut insert = tx.prepare_cached(
                "INSERT INTO remote_starred (server_id, track_id) VALUES (?1, ?2)",
            )?;
            for id in &base.ids {
                insert.execute(params![server_id, id])?;
            }
        }
        tx.commit()?;
        Ok(applied)
    }

    /// Drops the merge base of `server_id` (unpair), so pairing it again
    /// starts with a fresh union of both sides.
    pub fn forget_starred_base(&mut self, server_id: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM remote_starred WHERE server_id = ?1",
            [server_id],
        )?;
        tx.execute(
            "UPDATE remote_servers SET starred_version = NULL, starred_synced_at = 0
             WHERE server_id = ?1",
            [server_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// The current time as Unix milliseconds, or 0 when the clock is before 1970.
pub(crate) fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
