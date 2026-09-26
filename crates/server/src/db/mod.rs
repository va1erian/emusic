//! Database store for emusic-server.

pub mod models;

use std::path::Path;
use std::sync::{Arc, Mutex};

use models::{Device, ServerTrack};
use rusqlite::{Connection, Result as SqlResult, params};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Item not found: {0}")]
    NotFound(String),
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self, DbError> {
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                public_key TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER,
                is_revoked INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL,
                format TEXT NOT NULL,
                title TEXT,
                artist TEXT,
                album TEXT,
                duration_secs REAL,
                subtunes INTEGER DEFAULT 1,
                file_size INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                hash TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_path ON tracks(relative_path);

            CREATE TABLE IF NOT EXISTS pairing_codes (
                code TEXT PRIMARY KEY,
                expires_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn add_pairing_code(&self, code: &str, ttl_secs: i64) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        let expires_at = now + ttl_secs;
        conn.execute(
            "INSERT OR REPLACE INTO pairing_codes (code, expires_at, created_at) VALUES (?1, ?2, ?3)",
            params![code, expires_at, now],
        )?;
        Ok(())
    }

    pub fn verify_and_consume_pairing_code(&self, code: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        let mut stmt = conn.prepare("SELECT expires_at FROM pairing_codes WHERE code = ?1")?;
        let result = stmt.query_row(params![code], |row| {
            let expires_at: i64 = row.get(0)?;
            Ok(expires_at)
        });

        match result {
            Ok(expires_at) => {
                conn.execute("DELETE FROM pairing_codes WHERE code = ?1", params![code])?;
                Ok(expires_at >= now)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(DbError::Sql(e)),
        }
    }

    pub fn register_device(
        &self,
        id: &str,
        name: &str,
        public_key: &str,
    ) -> Result<Device, DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        conn.execute(
            "INSERT OR REPLACE INTO devices (id, name, public_key, paired_at, last_seen, is_revoked) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![id, name, public_key, now, now],
        )?;
        Ok(Device {
            id: id.to_string(),
            name: name.to_string(),
            public_key: public_key.to_string(),
            paired_at: now,
            last_seen: Some(now),
            is_revoked: false,
        })
    }

    pub fn get_device(&self, id: &str) -> Result<Option<Device>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, public_key, paired_at, last_seen, is_revoked FROM devices WHERE id = ?1")?;
        let res = stmt.query_row(params![id], |row| {
            let is_revoked_int: i32 = row.get(5)?;
            Ok(Device {
                id: row.get(0)?,
                name: row.get(1)?,
                public_key: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
                is_revoked: is_revoked_int != 0,
            })
        });

        match res {
            Ok(dev) => Ok(Some(dev)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sql(e)),
        }
    }

    pub fn touch_device(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        conn.execute(
            "UPDATE devices SET last_seen = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn revoke_device(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET is_revoked = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn upsert_track(&self, track: &ServerTrack) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tracks (id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                track.id,
                track.relative_path,
                track.format,
                track.title,
                track.artist,
                track.album,
                track.duration_secs,
                track.subtunes,
                track.file_size,
                track.mtime,
                track.hash,
            ],
        )?;
        Ok(())
    }

    pub fn get_track(&self, id: &str) -> Result<Option<ServerTrack>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash FROM tracks WHERE id = ?1")?;
        let res = stmt.query_row(params![id], row_to_track);
        match res {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::Sql(e)),
        }
    }

    pub fn list_tracks(&self, since_mtime: i64) -> Result<Vec<ServerTrack>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash FROM tracks WHERE mtime >= ?1 ORDER BY mtime ASC")?;
        let rows = stmt.query_map(params![since_mtime], row_to_track)?;
        let mut tracks = Vec::new();
        for r in rows {
            tracks.push(r?);
        }
        Ok(tracks)
    }
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<ServerTrack> {
    Ok(ServerTrack {
        id: row.get(0)?,
        relative_path: row.get(1)?,
        format: row.get(2)?,
        title: row.get(3)?,
        artist: row.get(4)?,
        album: row.get(5)?,
        duration_secs: row.get(6)?,
        subtunes: row.get(7)?,
        file_size: row.get(8)?,
        mtime: row.get(9)?,
        hash: row.get(10)?,
    })
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_pairing_and_device() {
        let db = Database::in_memory().unwrap();

        db.add_pairing_code("123456", 600).unwrap();
        assert!(db.verify_and_consume_pairing_code("123456").unwrap());
        // Code consumed, second verify fails
        assert!(!db.verify_and_consume_pairing_code("123456").unwrap());

        let dev = db
            .register_device("dev-1", "LivingRoom", "pubkey123")
            .unwrap();
        assert_eq!(dev.name, "LivingRoom");

        let fetched = db.get_device("dev-1").unwrap().unwrap();
        assert!(!fetched.is_revoked);

        db.revoke_device("dev-1").unwrap();
        let revoked = db.get_device("dev-1").unwrap().unwrap();
        assert!(revoked.is_revoked);
    }
}
