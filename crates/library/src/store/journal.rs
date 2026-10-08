//! Choosing the database's journal: WAL where it works, SQLite's rollback
//! journal where it cannot.
//!
//! WAL keeps its index in a `-shm` file mapped shared between connections. A
//! system without shared file mappings (LazyOS) accepts the
//! `journal_mode = WAL` pragma, records WAL in the database header, and then
//! fails every write with `SQLITE_IOERR_SHMMAP`. So WAL is tried once per
//! directory and process on a throwaway file beside the database, never on
//! the database itself; without it the database runs on the rollback journal, where the
//! busy timeout still serialises the scanner's writes against the UI's reads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use rusqlite::Connection;

use crate::error::Result;

/// What the probe found, by directory: a store reopens its database, and a
/// second database may live on another file system.
static WAL_WORKS: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();

/// Numbers this process's probes, so concurrent ones never share a file.
static PROBES: AtomicU64 = AtomicU64::new(0);

/// Puts `conn`, a connection to the file `path`, in the best journal mode.
pub(super) fn choose(conn: &Connection, path: &Path) -> Result<()> {
    let path = &resolve(path);
    if wal_works(path) && set_journal(conn, "WAL")?.eq_ignore_ascii_case("wal") {
        return Ok(());
    }
    // A database an earlier run left in WAL mode opens without the index file
    // under an exclusive lock, which is enough to convert it.
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
    conn.pragma_update(None, "journal_mode", "DELETE")?;
    conn.pragma_update(None, "locking_mode", "NORMAL")?;
    // The exclusive lock is released at the next access.
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")?;
    Ok(())
}

/// Sets the journal mode and returns the one SQLite chose, which is not the
/// one asked for when the change is impossible (no error is raised then).
fn set_journal(conn: &Connection, mode: &str) -> rusqlite::Result<String> {
    conn.pragma_update_and_check(None, "journal_mode", mode, |row| row.get(0))
}

/// `path` with its directory's symlinks resolved, so the probe runs on the
/// file system the database really lives on. The file itself may not exist
/// yet; a directory that cannot be resolved is kept as it is.
fn resolve(path: &Path) -> PathBuf {
    if let Ok(target) = std::fs::canonicalize(path) {
        return target;
    }
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => std::fs::canonicalize(dir)
            .map(|dir| dir.join(name))
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// Whether WAL works in `path`'s directory, probed once per directory.
fn wal_works(path: &Path) -> bool {
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut cache = WAL_WORKS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *cache.entry(dir).or_insert_with(|| {
        let works = wal_works_beside(path);
        if !works {
            tracing::warn!("WAL journaling unavailable; using a rollback journal");
        }
        works
    })
}

/// Whether a WAL database next to `path` can be written. The probe file is
/// named after this process and probe, so another process probing beside the
/// same database never removes it mid-probe.
fn wal_works_beside(path: &Path) -> bool {
    let mut probe = path.as_os_str().to_owned();
    let number = PROBES.fetch_add(1, Ordering::Relaxed);
    probe.push(format!(".walprobe-{}-{number}", std::process::id()));
    let probe = Path::new(&probe);
    let works = (|| -> rusqlite::Result<bool> {
        let conn = Connection::open(probe)?;
        if !set_journal(&conn, "WAL")?.eq_ignore_ascii_case("wal") {
            return Ok(false);
        }
        conn.execute_batch("CREATE TABLE IF NOT EXISTS probe(x); INSERT INTO probe VALUES (1);")?;
        Ok(true)
    })()
    .unwrap_or(false);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut file = probe.as_os_str().to_owned();
        file.push(suffix);
        let _ = std::fs::remove_file(file);
    }
    works
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_leaves_nothing_behind() {
        let dir = std::env::temp_dir().join(format!("emusic-walprobe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(wal_works_beside(&dir.join("library.db")));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn a_local_database_gets_wal_through_its_resolved_path() {
        let dir = std::env::temp_dir().join(format!("emusic-walchoose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.db");
        let conn = Connection::open(&path).unwrap();
        choose(&conn, &path).unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
