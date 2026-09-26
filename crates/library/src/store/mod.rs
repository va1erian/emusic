//! The SQLite-backed library store.
//!
//! [`Store`] owns a single [`rusqlite::Connection`] and exposes the
//! persistence API used by the scanner, player and UI. Table-specific
//! operations live in sibling modules (`tracks`, `folders`, `stats`) as
//! `impl Store` blocks so this file stays focused on connection setup.

mod folders;
mod migrations;
mod remote;
mod schema;
mod stats;
mod tracks;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{LibraryError, Result};

pub use folders::Folder;
pub use remote::RemoteTrack;
pub use stats::{MostPlayedEntry, PlayHistoryEntry, TrackStats};

/// How long a connection waits for another writer's lock before returning
/// `SQLITE_BUSY`. The scanner and the UI share one database file through
/// separate connections (#69), so a brief wait lets a quick UI write or read
/// ride out a scan's batched commit instead of failing immediately.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Handle to the library database.
///
/// Not `Sync`: callers that need to share a `Store` across threads (e.g.
/// the UI thread and a scanner writer thread) should wrap it in a mutex.
/// Background workers that must not contend on that mutex can instead open
/// their own connection with [`Store::open_second`].
pub struct Store {
    conn: Connection,
    /// The file this store was opened from; `None` for in-memory stores,
    /// which cannot be shared with a second connection.
    path: Option<PathBuf>,
}

impl Store {
    /// Opens (creating if necessary) the database file at `path`, applying
    /// any pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|source| LibraryError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let conn = Connection::open(path)?;
        Self::init(&conn)?;
        Ok(Self {
            conn,
            path: Some(path.to_path_buf()),
        })
    }

    /// Opens a second connection to the same database file.
    ///
    /// Long-running background work (the scanner's writer, the purge path)
    /// uses this so it never holds the shared [`Store`] mutex, which would
    /// freeze every UI read behind it for the whole scan (#69). SQLite in
    /// WAL mode supports concurrent readers and one writer; each connection
    /// applies migrations idempotently
    /// and shares the [`BUSY_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::NoSecondConnection`] for an in-memory store
    /// (tests use [`Store::open_in_memory`], which has no file to reopen).
    pub fn open_second(&self) -> Result<Self> {
        let path = self
            .path
            .as_deref()
            .ok_or(LibraryError::NoSecondConnection)?;
        Self::open(path)
    }

    /// The database file backing this store; `None` for in-memory stores.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Opens a private in-memory database. Intended for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(&conn)?;
        Ok(Self { conn, path: None })
    }

    /// Opens the store at the default location,
    /// `%LOCALAPPDATA%\emusic\library.db`.
    pub fn open_default() -> Result<Self> {
        Self::open(&default_db_path()?)
    }

    /// Configures a freshly opened connection: a busy timeout so a concurrent
    /// writer (the scanner, #69) is waited out rather than failing with
    /// `SQLITE_BUSY`, WAL journaling, foreign keys and migrations.
    fn init(conn: &Connection) -> Result<()> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::apply(conn)?;
        Ok(())
    }
}

/// The default database path, `%LOCALAPPDATA%\emusic\library.db`.
///
/// Does not create the directory; [`Store::open`] does that for whatever
/// path it is given.
pub fn default_db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().ok_or(LibraryError::NoDataDir)?;
    Ok(base.join("emusic").join("library.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_succeeds() {
        Store::open_in_memory().unwrap();
    }

    #[test]
    fn open_creates_parent_directory_and_file() {
        let dir = std::env::temp_dir().join(format!("emusic-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db_path = dir.join("nested").join("library.db");

        let store = Store::open(&db_path).unwrap();
        drop(store);

        assert!(db_path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_second_reopens_same_file() {
        let dir = std::env::temp_dir().join(format!("emusic-test-second-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db_path = dir.join("library.db");

        let store = Store::open(&db_path).unwrap();
        store.add_folder(std::path::Path::new(r"C:\music")).unwrap();
        let second = store.open_second().unwrap();
        assert_eq!(second.list_folders().unwrap().len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_second_rejects_in_memory_store() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            store.open_second(),
            Err(LibraryError::NoSecondConnection)
        ));
    }

    #[test]
    fn default_db_path_ends_with_expected_components() {
        let path = default_db_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "library.db");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "emusic");
    }
}
