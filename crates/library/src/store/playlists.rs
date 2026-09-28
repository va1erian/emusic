//! User playlists: named, ordered lists of library tracks.

use emusic_core::{PlaylistId, TrackId};
use rusqlite::{OptionalExtension, Transaction, params};

use super::Store;
use crate::error::Result;

/// A playlist and how many entries it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub created_at: i64,
    pub track_count: u32,
}

/// One position in a playlist. The same track may appear in several
/// entries, so `id` (not `track_id`) identifies an entry for removal and
/// reordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistEntry {
    pub id: i64,
    pub track_id: TrackId,
}

impl Store {
    /// Creates an empty playlist named `name` and returns its id.
    pub fn create_playlist(&self, name: &str, created_at: i64) -> Result<PlaylistId> {
        self.conn.execute(
            "INSERT INTO playlists (name, created_at) VALUES (?1, ?2)",
            params![name, created_at],
        )?;
        Ok(PlaylistId(self.conn.last_insert_rowid()))
    }

    /// Renames a playlist, returning whether it exists.
    pub fn rename_playlist(&self, id: PlaylistId, name: &str) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE playlists SET name = ?1 WHERE id = ?2",
            params![name, id.0],
        )?;
        Ok(updated > 0)
    }

    /// Deletes a playlist and its entries, returning whether it existed.
    pub fn delete_playlist(&self, id: PlaylistId) -> Result<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![id.0])?;
        Ok(deleted > 0)
    }

    /// All playlists in creation order.
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.created_at,
                    (SELECT count(*) FROM playlist_tracks t WHERE t.playlist_id = p.id)
             FROM playlists p ORDER BY p.id",
        )?;
        let playlists = stmt
            .query_map([], |row| {
                Ok(Playlist {
                    id: PlaylistId(row.get(0)?),
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    track_count: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(playlists)
    }

    /// The entries of a playlist in play order; empty if it does not exist.
    pub fn playlist_entries(&self, id: PlaylistId) -> Result<Vec<PlaylistEntry>> {
        entries(&self.conn, id)
    }

    /// Appends `tracks` (in order, duplicates allowed) to the end of a
    /// playlist and returns how many were added: `0` if the playlist does
    /// not exist. Ids that match no track are skipped.
    pub fn add_to_playlist(&self, id: PlaylistId, tracks: &[TrackId]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let exists = tx
            .query_row(
                "SELECT 1 FROM playlists WHERE id = ?1",
                params![id.0],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Ok(0);
        }
        let mut next: i64 = tx.query_row(
            "SELECT count(*) FROM playlist_tracks WHERE playlist_id = ?1",
            params![id.0],
            |row| row.get(0),
        )?;
        let mut added = 0;
        for track in tracks {
            let inserted = tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 SELECT ?1, id, ?3 FROM tracks WHERE id = ?2",
                params![id.0, track.0, next],
            )?;
            next += inserted as i64;
            added += inserted;
        }
        tx.commit()?;
        Ok(added)
    }

    /// Removes the given entries from a playlist and closes the gaps,
    /// returning how many were removed.
    pub fn remove_playlist_entries(&self, id: PlaylistId, entry_ids: &[i64]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut removed = 0;
        for entry in entry_ids {
            removed += tx.execute(
                "DELETE FROM playlist_tracks WHERE id = ?1 AND playlist_id = ?2",
                params![entry, id.0],
            )?;
        }
        let remaining: Vec<i64> = entries(&tx, id)?.iter().map(|e| e.id).collect();
        renumber(&tx, &remaining)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Moves the given entries, keeping their relative order, so they sit
    /// together starting at index `to` of the list as it was before the
    /// move (`to == len` appends). Unknown entry ids are ignored.
    pub fn move_playlist_entries(
        &self,
        id: PlaylistId,
        entry_ids: &[i64],
        to: usize,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let current: Vec<i64> = entries(&tx, id)?.iter().map(|e| e.id).collect();
        let (moved, mut rest): (Vec<i64>, Vec<i64>) =
            current.iter().partition(|entry| entry_ids.contains(entry));
        let before = current
            .iter()
            .take(to)
            .filter(|entry| entry_ids.contains(entry))
            .count();
        let at = to.saturating_sub(before).min(rest.len());
        rest.splice(at..at, moved);
        renumber(&tx, &rest)?;
        tx.commit()?;
        Ok(())
    }
}

fn entries(conn: &rusqlite::Connection, id: PlaylistId) -> Result<Vec<PlaylistEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
    )?;
    let entries = stmt
        .query_map(params![id.0], |row| {
            Ok(PlaylistEntry {
                id: row.get(0)?,
                track_id: TrackId(row.get(1)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

/// Rewrites `position` so `order` (entry ids) becomes `0..n`.
fn renumber(tx: &Transaction, order: &[i64]) -> Result<()> {
    let mut stmt = tx.prepare("UPDATE playlist_tracks SET position = ?1 WHERE id = ?2")?;
    for (position, entry) in order.iter().enumerate() {
        stmt.execute(params![position as i64, entry])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
