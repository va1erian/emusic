//! Track persistence and delta-sync queries.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, TransactionBehavior};

use crate::db::Db;
use crate::db::models::{NewTrack, SyncDelta, TrackRecord};
use crate::db::schema::META_LIBRARY_VERSION;
use crate::error::{Result, ServerError};
use crate::util::unix_now;

const TRACK_COLUMNS: &str = "id, root_index, relative_path, format, kind, title, artist, \
    album_artist, album, album_id, genre, year, track_no, disc_no, duration_secs, subtunes, \
    channels, file_size, mtime, hash, has_art, sync_version, added_at";

impl Db {
    /// Inserts new tracks and updates changed ones, assigning a single new
    /// library version to the whole batch. Unchanged rows are left untouched
    /// and do not consume a version. Returns the number of inserted/updated
    /// rows.
    pub fn upsert_tracks(&self, tracks: &[NewTrack]) -> Result<usize> {
        if tracks.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next = library_version_tx(&tx)? + 1;
        let mut changed = 0usize;
        {
            let mut upsert = tx.prepare(UPSERT_SQL)?;
            let mut clear_tombstone = tx.prepare("DELETE FROM tombstones WHERE track_id = ?1")?;
            for track in tracks {
                let affected = upsert.execute(rusqlite::params![
                    track.id,
                    track.root_index,
                    track.relative_path,
                    track.format,
                    track.kind,
                    track.title,
                    track.artist,
                    track.album_artist,
                    track.album,
                    track.album_id,
                    track.genre,
                    track.year,
                    track.track_no,
                    track.disc_no,
                    track.duration_secs,
                    track.subtunes,
                    track.channels,
                    track.file_size,
                    track.mtime,
                    track.hash,
                    track.has_art as i64,
                    next,
                    track.added_at,
                ])?;
                if affected > 0 {
                    changed += 1;
                    clear_tombstone.execute([&track.id])?;
                }
            }
        }
        if changed > 0 {
            set_library_version_tx(&tx, next)?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Deletes tracks by id and records tombstones so clients can sync the
    /// removals. Returns the number of rows deleted.
    pub fn delete_tracks(&self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next = library_version_tx(&tx)? + 1;
        let now = unix_now();
        let mut removed = 0usize;
        {
            let mut delete = tx.prepare("DELETE FROM tracks WHERE id = ?1")?;
            let mut tombstone = tx.prepare(
                "INSERT OR REPLACE INTO tombstones (track_id, sync_version, deleted_at)
                 VALUES (?1, ?2, ?3)",
            )?;
            for id in ids {
                if delete.execute([id])? > 0 {
                    tombstone.execute(rusqlite::params![id, next, now])?;
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            set_library_version_tx(&tx, next)?;
        }
        tx.commit()?;
        Ok(removed)
    }

    /// Fetches a track by id.
    pub fn track_by_id(&self, id: &str) -> Result<Option<TrackRecord>> {
        let conn = self.conn()?;
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt.query_row([id], row_to_track).optional()?)
    }

    /// A track that can supply artwork for `album_id`, preferring ones that
    /// actually have art.
    pub fn art_candidate_for_album(&self, album_id: &str) -> Result<Option<TrackRecord>> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE album_id = ?1
             ORDER BY has_art DESC, track_no IS NULL, track_no LIMIT 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt.query_row([album_id], row_to_track).optional()?)
    }

    /// The current library version.
    pub fn library_version(&self) -> Result<i64> {
        let conn = self.conn()?;
        library_version_conn(&conn)
    }

    /// Returns every change (upserts and deletions) newer than `since`.
    pub fn sync_since(&self, since: i64) -> Result<SyncDelta> {
        let conn = self.conn()?;
        let version = library_version_conn(&conn)?;

        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE sync_version > ?1 ORDER BY sync_version, id"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tracks = stmt
            .query_map([since], row_to_track)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = conn.prepare(
            "SELECT track_id FROM tombstones WHERE sync_version > ?1 ORDER BY sync_version, track_id",
        )?;
        let deleted = stmt
            .query_map([since], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SyncDelta {
            version,
            tracks,
            deleted,
        })
    }

    /// All track ids currently under `root_index`.
    pub fn track_ids_for_root(&self, root_index: i64) -> Result<HashSet<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id FROM tracks WHERE root_index = ?1")?;
        let rows = stmt.query_map([root_index], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
    }

    /// Total number of tracks in the store.
    pub fn track_count(&self) -> Result<u64> {
        let conn = self.conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
        Ok(count as u64)
    }
}

fn library_version_conn(conn: &rusqlite::Connection) -> Result<i64> {
    let value: String = conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        [META_LIBRARY_VERSION],
        |row| row.get(0),
    )?;
    value
        .parse::<i64>()
        .map_err(|_| ServerError::Metadata(format!("library_version {value:?} is not an integer")))
}

fn library_version_tx(tx: &rusqlite::Transaction<'_>) -> Result<i64> {
    let value: String = tx.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        [META_LIBRARY_VERSION],
        |row| row.get(0),
    )?;
    value
        .parse::<i64>()
        .map_err(|_| ServerError::Metadata(format!("library_version {value:?} is not an integer")))
}

fn set_library_version_tx(tx: &rusqlite::Transaction<'_>, version: i64) -> Result<()> {
    tx.execute(
        "UPDATE meta SET value = ?1 WHERE key = ?2",
        rusqlite::params![version.to_string(), META_LIBRARY_VERSION],
    )?;
    Ok(())
}

/// `ON CONFLICT ... WHERE` makes change detection atomic: the row is only
/// rewritten (and the version advanced) when a meaningful field differs.
const UPSERT_SQL: &str = r"
INSERT INTO tracks (
    id, root_index, relative_path, format, kind, title, artist, album_artist, album, album_id,
    genre, year, track_no, disc_no, duration_secs, subtunes, channels, file_size, mtime, hash,
    has_art, sync_version, added_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
)
ON CONFLICT(id) DO UPDATE SET
    root_index = excluded.root_index,
    relative_path = excluded.relative_path,
    format = excluded.format,
    kind = excluded.kind,
    title = excluded.title,
    artist = excluded.artist,
    album_artist = excluded.album_artist,
    album = excluded.album,
    album_id = excluded.album_id,
    genre = excluded.genre,
    year = excluded.year,
    track_no = excluded.track_no,
    disc_no = excluded.disc_no,
    duration_secs = excluded.duration_secs,
    subtunes = excluded.subtunes,
    channels = excluded.channels,
    file_size = excluded.file_size,
    mtime = excluded.mtime,
    hash = excluded.hash,
    has_art = excluded.has_art,
    sync_version = excluded.sync_version
WHERE tracks.file_size <> excluded.file_size
   OR tracks.mtime <> excluded.mtime
   OR tracks.hash <> excluded.hash
   OR tracks.relative_path <> excluded.relative_path
   OR tracks.has_art <> excluded.has_art
   OR tracks.format <> excluded.format
   OR tracks.kind <> excluded.kind
   OR tracks.title IS NOT excluded.title
   OR tracks.artist IS NOT excluded.artist
   OR tracks.album_artist IS NOT excluded.album_artist
   OR tracks.album IS NOT excluded.album
   OR tracks.album_id IS NOT excluded.album_id
   OR tracks.genre IS NOT excluded.genre
   OR tracks.year IS NOT excluded.year
   OR tracks.track_no IS NOT excluded.track_no
   OR tracks.disc_no IS NOT excluded.disc_no
   OR tracks.duration_secs IS NOT excluded.duration_secs
   OR tracks.subtunes <> excluded.subtunes
   OR tracks.channels IS NOT excluded.channels
";

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrackRecord> {
    Ok(TrackRecord {
        id: row.get("id")?,
        root_index: row.get("root_index")?,
        relative_path: row.get("relative_path")?,
        format: row.get("format")?,
        kind: row.get("kind")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album_artist: row.get("album_artist")?,
        album: row.get("album")?,
        album_id: row.get("album_id")?,
        genre: row.get("genre")?,
        year: row.get("year")?,
        track_no: row.get("track_no")?,
        disc_no: row.get("disc_no")?,
        duration_secs: row.get("duration_secs")?,
        subtunes: row.get::<_, i64>("subtunes")? as u32,
        channels: row
            .get::<_, Option<i64>>("channels")?
            .map(|value| value as u32),
        file_size: row.get::<_, i64>("file_size")? as u64,
        mtime: row.get("mtime")?,
        hash: row.get("hash")?,
        has_art: row.get::<_, i64>("has_art")? != 0,
        sync_version: row.get("sync_version")?,
        added_at: row.get("added_at")?,
    })
}
