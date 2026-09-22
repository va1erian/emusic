//! The SQLite-backed library store.
//!
//! [`Store`] owns a single [`rusqlite::Connection`] and exposes the
//! persistence API used by the scanner, player and UI. Table-specific
//! operations live in sibling modules (`tracks`, `folders`, `stats`) as
//! `impl Store` blocks so this file stays focused on connection setup.

mod folders;
mod migrations;
mod schema;
mod stats;
mod tracks;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{LibraryError, Result};

pub use folders::Folder;
pub use stats::TrackStats;

/// Handle to the library database.
///
/// Not `Sync`: callers that need to share a `Store` across threads (e.g.
/// the UI thread and a scanner writer thread) should wrap it in a mutex.
pub struct Store {
    conn: Connection,
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
        Self::init(conn)
    }

    /// Opens a private in-memory database. Intended for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    /// Opens the store at the default location,
    /// `%LOCALAPPDATA%\emusic\library.db`.
    pub fn open_default() -> Result<Self> {
        Self::open(&default_db_path()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::apply(&conn)?;
        Ok(Self { conn })
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
    fn default_db_path_ends_with_expected_components() {
        let path = default_db_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "library.db");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "emusic");
    }
}
