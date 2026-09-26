use std::path::{Path, PathBuf};
use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub public_key: String,
    pub paired_at: String,
    pub last_seen: Option<String>,
    pub is_revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecord {
    pub id: String,
    pub relative_path: String,
    pub format: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub subtunes: u16,
    pub file_size: u64,
    pub mtime: u64,
    pub hash: String,
    pub version: u64,
}

pub struct DbStore {
    conn_path: PathBuf,
}

impl DbStore {
    pub fn new(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir).ok();
        let conn_path = data_dir.join("emusic-server.db");
        let store = Self { conn_path };
        store.init_tables()?;
        Ok(store)
    }

    fn get_conn(&self) -> Result<Connection> {
        Connection::open(&self.conn_path)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                public_key TEXT NOT NULL,
                paired_at TEXT NOT NULL,
                last_seen TEXT,
                is_revoked INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL UNIQUE,
                format TEXT NOT NULL,
                title TEXT,
                artist TEXT,
                album TEXT,
                duration_secs REAL,
                subtunes INTEGER DEFAULT 1,
                file_size INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                hash TEXT NOT NULL,
                version INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS pairing_codes (
                code TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS server_settings (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn get_or_create_secret_key(&self) -> Result<Vec<u8>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT value FROM server_settings WHERE key = 'secret_key'")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let secret: Vec<u8> = row.get(0)?;
            Ok(secret)
        } else {
            use rand::RngCore;
            let mut secret = vec![0u8; 32];
            rand::thread_rng().fill_bytes(&mut secret);
            conn.execute(
                "INSERT INTO server_settings (key, value) VALUES ('secret_key', ?1)",
                params![secret],
            )?;
            Ok(secret)
        }
    }

    pub fn register_device(&self, device: &PairedDevice) -> Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "INSERT INTO devices (id, name, public_key, paired_at, last_seen, is_revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name,
               public_key=excluded.public_key,
               last_seen=excluded.last_seen,
               is_revoked=excluded.is_revoked",
            params![
                device.id,
                device.name,
                device.public_key,
                device.paired_at,
                device.last_seen,
                device.is_revoked as i32
            ],
        )?;
        Ok(())
    }

    pub fn get_device(&self, id: &str) -> Result<Option<PairedDevice>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, public_key, paired_at, last_seen, is_revoked FROM devices WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            let is_revoked_int: i32 = row.get(5)?;
            Ok(Some(PairedDevice {
                id: row.get(0)?,
                name: row.get(1)?,
                public_key: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
                is_revoked: is_revoked_int != 0,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_track(&self, track: &TrackRecord) -> Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "INSERT INTO tracks (id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(relative_path) DO UPDATE SET
               format=excluded.format,
               title=excluded.title,
               artist=excluded.artist,
               album=excluded.album,
               duration_secs=excluded.duration_secs,
               subtunes=excluded.subtunes,
               file_size=excluded.file_size,
               mtime=excluded.mtime,
               hash=excluded.hash,
               version=excluded.version",
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
                track.version
            ],
        )?;
        Ok(())
    }

    pub fn get_tracks_since(&self, since_version: u64) -> Result<Vec<TrackRecord>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash, version
             FROM tracks WHERE version > ?1 ORDER BY version ASC",
        )?;

        let rows = stmt.query_map(params![since_version], |row| {
            Ok(TrackRecord {
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
                version: row.get(11)?,
            })
        })?;

        let mut tracks = Vec::new();
        for track in rows {
            tracks.push(track?);
        }
        Ok(tracks)
    }

    pub fn get_track_by_id(&self, id: &str) -> Result<Option<TrackRecord>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, relative_path, format, title, artist, album, duration_secs, subtunes, file_size, mtime, hash, version
             FROM tracks WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(TrackRecord {
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
                version: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_pairing_code(&self, code: &str, ttl_secs: u64) -> Result<()> {
        let conn = self.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now + ttl_secs;

        conn.execute(
            "INSERT INTO pairing_codes (code, created_at, expires_at) VALUES (?1, ?2, ?3)",
            params![code, now, expires_at],
        )?;
        Ok(())
    }

    pub fn consume_pairing_code(&self, code: &str) -> Result<bool> {
        let conn = self.get_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut stmt = conn.prepare(
            "SELECT expires_at FROM pairing_codes WHERE code = ?1",
        )?;
        let mut rows = stmt.query(params![code])?;

        if let Some(row) = rows.next()? {
            let expires_at: u64 = row.get(0)?;
            conn.execute("DELETE FROM pairing_codes WHERE code = ?1", params![code])?;
            Ok(now <= expires_at)
        } else {
            Ok(false)
        }
    }
}
