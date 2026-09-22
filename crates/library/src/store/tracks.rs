use std::collections::HashMap;
use std::path::{Path, PathBuf};

use emusic_core::{ArtSource, Track, TrackId, TrackKind};
use rusqlite::{OptionalExtension, Row, params};

use super::Store;
use crate::error::Result;

const TRACK_COLUMNS: &str = "id, path, dir, filename, ext, size, mtime, kind, duration_ms, \
     bitrate, sample_rate, channels, title, artist, album_artist, album, genre, year, \
     track_no, disc_no, composer, comment, art_source_kind, art_source_path, added_at";

const UPSERT_SQL: &str = "
    INSERT INTO tracks (
        path, dir, filename, ext, size, mtime, kind, duration_ms,
        bitrate, sample_rate, channels, title, artist, album_artist, album, genre, year,
        track_no, disc_no, composer, comment, art_source_kind, art_source_path, added_at
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
        ?18, ?19, ?20, ?21, ?22, ?23, ?24
    )
    ON CONFLICT(path) DO UPDATE SET
        dir = excluded.dir,
        filename = excluded.filename,
        ext = excluded.ext,
        size = excluded.size,
        mtime = excluded.mtime,
        kind = excluded.kind,
        duration_ms = excluded.duration_ms,
        bitrate = excluded.bitrate,
        sample_rate = excluded.sample_rate,
        channels = excluded.channels,
        title = excluded.title,
        artist = excluded.artist,
        album_artist = excluded.album_artist,
        album = excluded.album,
        genre = excluded.genre,
        year = excluded.year,
        track_no = excluded.track_no,
        disc_no = excluded.disc_no,
        composer = excluded.composer,
        comment = excluded.comment,
        art_source_kind = excluded.art_source_kind,
        art_source_path = excluded.art_source_path,
        added_at = excluded.added_at
    RETURNING id
";

const MOVE_SQL: &str = "
    UPDATE tracks SET
        path = ?1, dir = ?2, filename = ?3, ext = ?4, size = ?5, mtime = ?6,
        kind = ?7, duration_ms = ?8, bitrate = ?9, sample_rate = ?10,
        channels = ?11, title = ?12, artist = ?13, album_artist = ?14,
        album = ?15, genre = ?16, year = ?17, track_no = ?18, disc_no = ?19,
        composer = ?20, comment = ?21, art_source_kind = ?22, art_source_path = ?23
    WHERE path = ?24
    RETURNING id, added_at
";

impl Store {
    /// Inserts or updates `tracks` (matched by `path`) in a single
    /// transaction, writing back the store-assigned [`TrackId`] into each
    /// track.
    pub fn upsert_tracks(&mut self, tracks: &mut [Track]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(UPSERT_SQL)?;
            for track in tracks.iter_mut() {
                let (art_kind, art_path) = art_source_to_columns(&track.art_source);
                let id: i64 = stmt.query_row(
                    params![
                        path_to_string(&track.path),
                        path_to_string(&track.dir),
                        track.filename,
                        track.ext,
                        track.size,
                        track.mtime,
                        track_kind_to_i64(track.kind),
                        track.duration_ms,
                        track.bitrate,
                        track.sample_rate,
                        track.channels,
                        track.title,
                        track.artist,
                        track.album_artist,
                        track.album,
                        track.genre,
                        track.year,
                        track.track_no,
                        track.disc_no,
                        track.composer,
                        track.comment,
                        art_kind,
                        art_path,
                        track.added_at,
                    ],
                    |row| row.get(0),
                )?;
                track.id = TrackId(id);
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes tracks matching any of `paths`. Returns the number of rows
    /// deleted.
    pub fn delete_tracks_by_paths(&mut self, paths: &[PathBuf]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        {
            let mut stmt = tx.prepare_cached("DELETE FROM tracks WHERE path = ?1")?;
            for path in paths {
                deleted += stmt.execute(params![path_to_string(path)])?;
            }
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// Updates the row currently stored at `old_path` to hold `track`'s
    /// data — including its new path — preserving the row's id, original
    /// `added_at` and (via the id) its play history.
    ///
    /// The scanner uses this for changed files and for detected moves: a
    /// rename must not go through [`Store::upsert_tracks`], which matches
    /// by path, inserts a fresh row and leaves the old row's play history
    /// to cascade-delete.
    ///
    /// Returns `false` if no row currently lives at `old_path`.
    pub fn move_track(&mut self, old_path: &Path, track: &mut Track) -> Result<bool> {
        let (art_kind, art_path) = art_source_to_columns(&track.art_source);
        let tx = self.conn.transaction()?;
        let existing = {
            let mut stmt = tx.prepare_cached(MOVE_SQL)?;
            stmt.query_row(
                params![
                    path_to_string(&track.path),
                    path_to_string(&track.dir),
                    track.filename,
                    track.ext,
                    track.size,
                    track.mtime,
                    track_kind_to_i64(track.kind),
                    track.duration_ms,
                    track.bitrate,
                    track.sample_rate,
                    track.channels,
                    track.title,
                    track.artist,
                    track.album_artist,
                    track.album,
                    track.genre,
                    track.year,
                    track.track_no,
                    track.disc_no,
                    track.composer,
                    track.comment,
                    art_kind,
                    art_path,
                    path_to_string(old_path),
                ],
                |row| Ok((TrackId(row.get(0)?), row.get::<_, i64>(1)?)),
            )
            .optional()?
        };
        tx.commit()?;

        match existing {
            Some((id, added_at)) => {
                track.id = id;
                track.added_at = added_at;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Loads every track in the library.
    pub fn load_all_tracks(&self) -> Result<Vec<Track>> {
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks ORDER BY path");
        let mut stmt = self.conn.prepare(&sql)?;
        let tracks = stmt
            .query_map([], row_to_track)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tracks)
    }

    /// Fetches a single track by its file path, if present.
    pub fn get_track_by_path(&self, path: &Path) -> Result<Option<Track>> {
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE path = ?1");
        let track = self
            .conn
            .query_row(&sql, params![path_to_string(path)], row_to_track)
            .optional()?;
        Ok(track)
    }

    /// Returns a `path -> (size, mtime)` map for every known track, used by
    /// the scanner to decide which files changed since the last scan
    /// without loading full track rows.
    pub fn size_mtime_map(&self) -> Result<HashMap<PathBuf, (u64, i64)>> {
        let mut stmt = self.conn.prepare("SELECT path, size, mtime FROM tracks")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let size: u64 = row.get(1)?;
            let mtime: i64 = row.get(2)?;
            Ok((PathBuf::from(path), (size, mtime)))
        })?;
        Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn track_kind_to_i64(kind: TrackKind) -> i64 {
    match kind {
        TrackKind::Stream => 0,
        TrackKind::Module => 1,
    }
}

fn track_kind_from_i64(value: i64) -> TrackKind {
    match value {
        1 => TrackKind::Module,
        _ => TrackKind::Stream,
    }
}

fn art_source_to_columns(art_source: &ArtSource) -> (i64, Option<String>) {
    match art_source {
        ArtSource::None => (0, None),
        ArtSource::Embedded => (1, None),
        ArtSource::ExternalFile(path) => (2, Some(path_to_string(path))),
    }
}

fn art_source_from_columns(kind: i64, path: Option<String>) -> ArtSource {
    match kind {
        1 => ArtSource::Embedded,
        2 => path
            .map(PathBuf::from)
            .map_or(ArtSource::None, ArtSource::ExternalFile),
        _ => ArtSource::None,
    }
}

fn row_to_track(row: &Row) -> rusqlite::Result<Track> {
    let path: String = row.get("path")?;
    let dir: String = row.get("dir")?;
    let kind: i64 = row.get("kind")?;
    let art_source_kind: i64 = row.get("art_source_kind")?;
    let art_source_path: Option<String> = row.get("art_source_path")?;

    Ok(Track {
        id: TrackId(row.get("id")?),
        path: PathBuf::from(path),
        dir: PathBuf::from(dir),
        filename: row.get("filename")?,
        ext: row.get("ext")?,
        size: row.get("size")?,
        mtime: row.get("mtime")?,
        kind: track_kind_from_i64(kind),
        duration_ms: row.get("duration_ms")?,
        bitrate: row.get("bitrate")?,
        sample_rate: row.get("sample_rate")?,
        channels: row.get("channels")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album_artist: row.get("album_artist")?,
        album: row.get("album")?,
        genre: row.get("genre")?,
        year: row.get("year")?,
        track_no: row.get("track_no")?,
        disc_no: row.get("disc_no")?,
        composer: row.get("composer")?,
        comment: row.get("comment")?,
        art_source: art_source_from_columns(art_source_kind, art_source_path),
        added_at: row.get("added_at")?,
    })
}

#[cfg(test)]
mod tests;
