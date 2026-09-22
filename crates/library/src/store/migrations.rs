use rusqlite::Connection;

use super::schema::{CURRENT_VERSION, MIGRATIONS};
use crate::error::{LibraryError, Result};

/// Applies any pending schema migrations to `conn`, tracked via SQLite's
/// built-in `PRAGMA user_version`.
pub(crate) fn apply(conn: &Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current > CURRENT_VERSION {
        return Err(LibraryError::UnsupportedSchemaVersion {
            found: current,
            supported: CURRENT_VERSION,
        });
    }

    for (index, migration) in MIGRATIONS.iter().enumerate() {
        let version = index as i64 + 1;
        if version <= current {
            continue;
        }
        conn.execute_batch(migration)?;
        conn.pragma_update(None, "user_version", version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_all_migrations_and_sets_user_version() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);

        // Tables from the migration should now exist.
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'tracks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        apply(&conn).unwrap();
        apply(&conn).unwrap();
    }

    #[test]
    fn rejects_newer_schema_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", CURRENT_VERSION + 1)
            .unwrap();

        let err = apply(&conn).unwrap_err();
        assert!(matches!(err, LibraryError::UnsupportedSchemaVersion { .. }));
    }
}
