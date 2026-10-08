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
    if wal_works(path) {
        conn.pragma_update(None, "journal_mode", "WAL")?;
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
    let works = (|| -> rusqlite::Result<()> {
        let conn = Connection::open(probe)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS probe(x); INSERT INTO probe VALUES (1);")
    })()
    .is_ok();
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
}
