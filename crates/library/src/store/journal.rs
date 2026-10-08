//! Choosing the database's journal: WAL where it works, SQLite's rollback
//! journal where it cannot.
//!
//! WAL keeps its index in a `-shm` file mapped shared between connections. A
//! system without shared file mappings (LazyOS) accepts the
//! `journal_mode = WAL` pragma, records WAL in the database header, and then
//! fails every write with `SQLITE_IOERR_SHMMAP`. So WAL is tried once per
//! process on a throwaway file beside the database, never on the database
//! itself; without it the database runs on the rollback journal, where the
//! busy timeout still serialises the scanner's writes against the UI's reads.

use std::path::Path;
use std::sync::OnceLock;

use rusqlite::Connection;

use crate::error::Result;

static WAL_WORKS: OnceLock<bool> = OnceLock::new();

/// Puts `conn`, a connection to the file `path`, in the best journal mode.
pub(super) fn choose(conn: &Connection, path: &Path) -> Result<()> {
    let works = *WAL_WORKS.get_or_init(|| {
        let works = wal_works_beside(path);
        if !works {
            tracing::warn!("WAL journaling unavailable; using a rollback journal");
        }
        works
    });
    if works {
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

/// Whether a WAL database next to `path` can be written.
fn wal_works_beside(path: &Path) -> bool {
    let mut probe = path.as_os_str().to_owned();
    probe.push(".walprobe");
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
