//! Remote-track persistence (#391).
//!
//! Tracks pulled from an `emusic-server` live in the same `tracks` table as
//! local files, so the index, search and views need no special-casing. They
//! are distinguished by `remote_server_id`/`remote_track_id` and carry the
//! server's `sync_version`, which drives delta sync and deletion detection.

use std::collections::HashSet;

use emusic_core::{Track, TrackId};
use rusqlite::{OptionalExtension, params};

use super::Store;
use super::tracks::{art_source_to_columns, path_to_string, track_kind_to_i64};
use crate::error::Result;

/// One track fetched from a remote server, ready to be stored.
#[derive(Debug, Clone)]
pub struct RemoteTrack {
    /// The server's opaque track id.
    pub remote_id: String,
    /// The server library version at which this row last changed.
    pub sync_version: i64,
    /// The track mapped onto a synthetic local cache path.
    pub track: Track,
}

const REMOTE_UPSERT_SQL: &str = "
    INSERT INTO tracks (
        path, dir, filename, ext, size, mtime, kind, duration_ms,
        bitrate, sample_rate, channels, title, artist, album_artist, album, genre, year,
        track_no, disc_no, composer, comment, art_source_kind, art_source_path, added_at,
        remote_server_id, remote_track_id, remote_sync_version
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
        ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
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
        added_at = excluded.added_at,
        remote_server_id = excluded.remote_server_id,
        remote_track_id = excluded.remote_track_id,
        remote_sync_version = excluded.remote_sync_version
    RETURNING id
";

impl Store {
    /// Inserts or updates the remote tracks of `server_id`, matched by their
    /// synthetic path, and writes the assigned [`TrackId`] back into each.
    pub fn upsert_remote_tracks(
        &mut self,
        server_id: &str,
        tracks: &mut [RemoteTrack],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(REMOTE_UPSERT_SQL)?;
            for remote in tracks.iter_mut() {
                let track = &mut remote.track;
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
                        server_id,
                        remote.remote_id,
                        remote.sync_version,
                    ],
                    |row| row.get(0),
                )?;
                track.id = TrackId(id);
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The server track ids currently stored for `server_id`.
    pub fn remote_track_ids(&self, server_id: &str) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT remote_track_id FROM tracks
             WHERE remote_server_id = ?1 AND remote_track_id IS NOT NULL",
        )?;
        let rows = stmt.query_map([server_id], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
    }

    /// Deletes the rows for `remote_ids` under `server_id`. Returns the number
    /// of rows removed.
    pub fn delete_remote_tracks(
        &mut self,
        server_id: &str,
        remote_ids: &[String],
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        {
            let mut stmt = tx.prepare_cached(
                "DELETE FROM tracks WHERE remote_server_id = ?1 AND remote_track_id = ?2",
            )?;
            for remote_id in remote_ids {
                deleted += stmt.execute(params![server_id, remote_id])?;
            }
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// The last library version synced for `server_id` (`0` if never synced).
    pub fn remote_since_version(&self, server_id: &str) -> Result<i64> {
        let version = self
            .conn
            .query_row(
                "SELECT since_version FROM remote_servers WHERE server_id = ?1",
                [server_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(version.unwrap_or(0))
    }

    /// Records the library version last synced for `server_id`.
    pub fn set_remote_since_version(&mut self, server_id: &str, version: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO remote_servers (server_id, since_version) VALUES (?1, ?2)
             ON CONFLICT(server_id) DO UPDATE SET since_version = excluded.since_version",
            params![server_id, version],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use emusic_core::{ArtSource, TrackKind};

    use super::*;

    fn remote_track(id: &str, sync_version: i64, title: &str) -> RemoteTrack {
        RemoteTrack {
            remote_id: id.to_string(),
            sync_version,
            track: Track {
                id: TrackId::UNASSIGNED,
                path: PathBuf::from(format!(r"C:\cache\srv\{id}.flac")),
                dir: PathBuf::from(r"C:\cache\srv"),
                filename: format!("{id}.flac"),
                ext: "flac".into(),
                size: 123,
                mtime: 1,
                kind: TrackKind::Stream,
                duration_ms: 1000,
                bitrate: None,
                sample_rate: None,
                channels: None,
                title: Some(title.into()),
                artist: Some("Artist".into()),
                album_artist: None,
                album: Some("Album".into()),
                genre: None,
                year: None,
                track_no: None,
                disc_no: None,
                composer: None,
                comment: None,
                art_source: ArtSource::None,
                added_at: 1,
                starred: false,
            },
        }
    }

    #[test]
    fn upsert_lists_and_deletes_remote_tracks() {
        let mut store = Store::open_in_memory().unwrap();
        let mut tracks = vec![remote_track("a", 1, "One"), remote_track("b", 2, "Two")];
        store.upsert_remote_tracks("srv", &mut tracks).unwrap();
        assert!(tracks[0].track.id != TrackId::UNASSIGNED);

        let ids = store.remote_track_ids("srv").unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("a") && ids.contains("b"));

        // Re-upserting the same path updates in place (no duplicate rows).
        let mut updated = vec![remote_track("a", 5, "One (remix)")];
        store.upsert_remote_tracks("srv", &mut updated).unwrap();
        assert_eq!(store.load_all_tracks().unwrap().len(), 2);
        assert_eq!(store.remote_track_ids("srv").unwrap().len(), 2);

        let deleted = store
            .delete_remote_tracks("srv", &["a".to_string()])
            .unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(
            store.remote_track_ids("srv").unwrap(),
            HashSet::from(["b".to_string()])
        );
    }

    #[test]
    fn since_version_round_trips_per_server() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.remote_since_version("srv").unwrap(), 0);
        store.set_remote_since_version("srv", 42).unwrap();
        store.set_remote_since_version("other", 7).unwrap();
        assert_eq!(store.remote_since_version("srv").unwrap(), 42);
        assert_eq!(store.remote_since_version("other").unwrap(), 7);
        store.set_remote_since_version("srv", 43).unwrap();
        assert_eq!(store.remote_since_version("srv").unwrap(), 43);
    }
}
