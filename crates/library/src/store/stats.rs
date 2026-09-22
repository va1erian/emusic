//! Play/skip history and aggregated stats.

use std::path::Path;

use emusic_core::{PlayEvent, TrackId};
use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;
use crate::scanner::paths;

/// Aggregated play statistics for a single track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackStats {
    pub track_id: TrackId,
    pub play_count: u32,
    pub skip_count: u32,
    pub last_played_at: Option<i64>,
}

/// A single row of playback history, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayHistoryEntry {
    /// Primary key of the `plays` row; lets the UI remove one entry.
    pub id: i64,
    pub track_id: TrackId,
    pub played_at: i64,
    pub duration_played_ms: u32,
    pub completed: bool,
}

/// A track's play count within some window, for "most played" listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MostPlayedEntry {
    pub track_id: TrackId,
    pub play_count: u32,
}

impl Store {
    /// Records a play event and updates the track's aggregated stats.
    ///
    /// Inserts one row into `plays` and, in the same transaction, bumps
    /// either `play_count` (when [`PlayEvent::completed`] is `true`) or
    /// `skip_count` on `track_stats`, refreshing `last_played_at` either
    /// way.
    pub fn record_play(&mut self, event: &PlayEvent) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO plays (track_id, played_at, duration_played_ms, completed)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                event.track_id.0,
                event.played_at,
                event.duration_played_ms,
                event.completed,
            ],
        )?;
        let (play_delta, skip_delta) = if event.completed { (1, 0) } else { (0, 1) };
        tx.execute(
            "INSERT INTO track_stats (track_id, play_count, skip_count, last_played_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(track_id) DO UPDATE SET
                play_count = play_count + excluded.play_count,
                skip_count = skip_count + excluded.skip_count,
                last_played_at = excluded.last_played_at",
            params![event.track_id.0, play_delta, skip_delta, event.played_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Fetches aggregated stats for a track, if it has ever been played or
    /// skipped.
    pub fn track_stats(&self, track_id: TrackId) -> Result<Option<TrackStats>> {
        let stats = self
            .conn
            .query_row(
                "SELECT track_id, play_count, skip_count, last_played_at
                 FROM track_stats WHERE track_id = ?1",
                params![track_id.0],
                |row| {
                    Ok(TrackStats {
                        track_id: TrackId(row.get(0)?),
                        play_count: row.get(1)?,
                        skip_count: row.get(2)?,
                        last_played_at: row.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(stats)
    }

    /// Resolves a filesystem path to the [`TrackId`] of its library row.
    ///
    /// Tries an exact match first — the common case, since paths reported
    /// by the player round-trip from what the store handed out via the
    /// queue. Falls back to the scanner's normalised-key comparison (see
    /// `scanner::paths::normalize_key`) for paths spelled differently, e.g.
    /// a mapped drive vs. the equivalent UNC share.
    pub fn resolve_track_id(&self, path: &Path) -> Result<Option<TrackId>> {
        if let Some(track) = self.get_track_by_path(path)? {
            return Ok(Some(track.id));
        }

        let key = paths::normalize_key(path);
        let mut stmt = self.conn.prepare("SELECT id, path FROM tracks")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let stored_path: String = row.get(1)?;
            if paths::normalize_key(Path::new(&stored_path)) == key {
                return Ok(Some(TrackId(id)));
            }
        }
        Ok(None)
    }

    /// Returns a page of playback history, newest first.
    ///
    /// `limit` is clamped to at least 1; `offset` is the number of newest
    /// rows to skip (`offset = page * page_size` for simple pagination).
    pub fn play_history_page(&self, limit: u32, offset: u32) -> Result<Vec<PlayHistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, track_id, played_at, duration_played_ms, completed
             FROM plays
             ORDER BY played_at DESC, id DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit.max(1), offset], |row| {
            Ok(PlayHistoryEntry {
                id: row.get(0)?,
                track_id: TrackId(row.get(1)?),
                played_at: row.get(2)?,
                duration_played_ms: row.get(3)?,
                completed: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Deletes a single playback history row, returning whether it existed.
    ///
    /// Aggregate [`TrackStats`] are intentionally left untouched: this only
    /// removes one history entry, it does not "un-play" the track.
    pub fn delete_play(&self, id: i64) -> Result<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM plays WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    /// Deletes every playback history row, returning how many were removed.
    ///
    /// Like [`Store::delete_play`], aggregate [`TrackStats`] are preserved;
    /// only the per-play history (and therefore the windowed "most played"
    /// rankings, which are computed from `plays`) is cleared.
    pub fn clear_plays(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM plays", [])?)
    }

    /// Returns the most-played tracks (by completed play count), optionally
    /// restricted to plays at or after `since` (a Unix timestamp).
    /// `since = None` computes an all-time ranking.
    pub fn most_played_since(
        &self,
        since: Option<i64>,
        limit: u32,
    ) -> Result<Vec<MostPlayedEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_id, COUNT(*) AS plays
             FROM plays
             WHERE completed = 1 AND (?1 IS NULL OR played_at >= ?1)
             GROUP BY track_id
             ORDER BY plays DESC, track_id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![since, limit.max(1)], |row| {
            Ok(MostPlayedEntry {
                track_id: TrackId(row.get(0)?),
                play_count: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
mod tests;
