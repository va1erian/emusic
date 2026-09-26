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

    #[test]
    fn upgrading_from_the_previous_version_preserves_tracks() {
        // Build the v5 schema (all but the last migration) and insert one
        // starred track, then apply the pending migration.
        let conn = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..MIGRATIONS.len() - 1] {
            conn.execute_batch(migration).unwrap();
        }
        conn.pragma_update(None, "user_version", CURRENT_VERSION - 1)
            .unwrap();
        conn.execute(
            "INSERT INTO tracks
                (path, dir, filename, ext, size, mtime, kind, duration_ms,
                 art_source_kind, added_at, starred)
             VALUES ('p', 'd', 'f', 'flac', 1, 1, 0, 1, 0, 1, 1)",
            [],
        )
        .unwrap();

        apply(&conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let tracks: i64 = conn
            .query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))
            .unwrap();
        let starred: i64 = conn
            .query_row("SELECT count(*) FROM tracks WHERE starred = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);
        assert_eq!(tracks, 1);
        assert_eq!(starred, 1, "upgrading must not reset user data");
    }
}
