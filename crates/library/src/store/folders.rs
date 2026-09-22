use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;

/// A watched library folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: i64,
    pub path: PathBuf,
    /// Whether this folder is included in scans.
    pub enabled: bool,
}

impl Store {
    /// Adds a folder to the library, or returns the existing row if the
    /// path is already present.
    pub fn add_folder(&self, path: &Path) -> Result<Folder> {
        self.conn.execute(
            "INSERT INTO folders (path, enabled) VALUES (?1, 1)
             ON CONFLICT(path) DO NOTHING",
            params![path_to_string(path)],
        )?;
        // ON CONFLICT DO NOTHING means the row may already have existed;
        // fetch it either way so callers always get its id.
        self.folder_by_path(path)?
            .ok_or_else(|| unreachable!("row was just inserted or already present"))
    }

    /// Removes a folder (and, via `ON DELETE CASCADE`, nothing else — track
    /// rows are independent of folders).
    pub fn remove_folder(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Lists every watched folder.
    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, enabled FROM folders ORDER BY path")?;
        let folders = stmt
            .query_map([], row_to_folder)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(folders)
    }

    /// Enables or disables a folder without removing it from the library.
    pub fn set_folder_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET enabled = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        Ok(())
    }

    fn folder_by_path(&self, path: &Path) -> Result<Option<Folder>> {
        let folder = self
            .conn
            .query_row(
                "SELECT id, path, enabled FROM folders WHERE path = ?1",
                params![path_to_string(path)],
                row_to_folder,
            )
            .optional()?;
        Ok(folder)
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn row_to_folder(row: &rusqlite::Row) -> rusqlite::Result<Folder> {
    let path: String = row.get(1)?;
    Ok(Folder {
        id: row.get(0)?,
        path: PathBuf::from(path),
        enabled: row.get(2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_list_folders() {
        let store = Store::open_in_memory().unwrap();
        let folder = store.add_folder(Path::new(r"C:\music")).unwrap();
        assert!(folder.enabled);

        let folders = store.list_folders().unwrap();
        assert_eq!(folders, vec![folder]);
    }

    #[test]
    fn add_folder_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        let first = store.add_folder(Path::new(r"C:\music")).unwrap();
        let second = store.add_folder(Path::new(r"C:\music")).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(store.list_folders().unwrap().len(), 1);
    }

    #[test]
    fn set_folder_enabled_toggles_flag() {
        let store = Store::open_in_memory().unwrap();
        let folder = store.add_folder(Path::new(r"C:\music")).unwrap();

        store.set_folder_enabled(folder.id, false).unwrap();
        let folders = store.list_folders().unwrap();
        assert!(!folders[0].enabled);
    }

    #[test]
    fn remove_folder_deletes_row() {
        let store = Store::open_in_memory().unwrap();
        let folder = store.add_folder(Path::new(r"C:\music")).unwrap();
        store.remove_folder(folder.id).unwrap();
        assert!(store.list_folders().unwrap().is_empty());
    }
}
