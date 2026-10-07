//! Rotating backups of the library database.
//!
//! Stars, playlists and play history exist only in the database: a rescan
//! rebuilds the tracks but not those. So once a day, [`Store::backup_if_due`]
//! copies the database to `backups/library-YYYYMMDD-HHMMSS.db` next to it
//! (`VACUUM INTO`, a consistent snapshot taken while the app keeps running)
//! and keeps the newest [`KEEP`].
//!
//! Each copy has its indexes rebuilt (`VACUUM INTO` copies index b-trees
//! as they are, so a stale index would carry over) and is then checked with
//! SQLite's `quick_check` before it counts. A copy that fails is deleted and
//! nothing is rotated out, so a damaged database can never push the last good
//! backups out, while one whose only fault is a stale index is still backed
//! up, repaired. The copy is written under a temporary name and only renamed
//! to its backup name once it passes, so an interrupted backup never looks
//! like a finished one.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::Store;
use crate::error::{LibraryError, Result};

/// Number of backups kept; older ones are deleted after a new one is written.
pub const KEEP: usize = 7;

/// Minimum age of the newest backup before another one is written.
pub const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Folder, next to the database, that holds the backups.
const DIR_NAME: &str = "backups";

/// File-name prefix and extension of a backup; the timestamp between them
/// sorts chronologically.
const PREFIX: &str = "library-";
const EXTENSION: &str = ".db";

/// Suffix of a backup still being written or checked; not a backup name.
const PARTIAL: &str = ".partial";

/// What [`Store::backup_if_due`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum BackupOutcome {
    /// The store has no file to back up (in-memory store).
    NoFile,
    /// The newest backup is younger than [`INTERVAL`].
    NotDue,
    /// A backup was written to this path.
    Written(PathBuf),
    /// The copy failed its integrity check and was deleted; carries SQLite's
    /// first reported problem.
    SkippedDamaged(String),
}

impl Store {
    /// Writes a backup if the newest one is older than [`INTERVAL`] (or there
    /// is none) and, once the copy passes its integrity check, deletes all but
    /// the newest [`KEEP`]. `now` names the backup and decides whether one is
    /// due.
    ///
    /// # Errors
    ///
    /// Fails if the backup folder cannot be created or listed, or SQLite
    /// cannot run the integrity check or write the copy.
    pub fn backup_if_due(&self, now: SystemTime) -> Result<BackupOutcome> {
        let Some(db) = self.path() else {
            return Ok(BackupOutcome::NoFile);
        };
        let dir = backup_dir(db);
        let existing = list_backups(&dir)?;
        if let Some(newest) = existing.last()
            && !is_due(newest, now)
        {
            return Ok(BackupOutcome::NotDue);
        }
        std::fs::create_dir_all(&dir).map_err(|source| LibraryError::Backup {
            path: dir.clone(),
            source,
        })?;
        remove_partials(&dir)?;
        let dest = dir.join(format!("{PREFIX}{}{EXTENSION}", self.timestamp(now)?));
        if !dest.exists() {
            let partial = dir.join(format!(
                "{PREFIX}{}{EXTENSION}{PARTIAL}",
                self.timestamp(now)?
            ));
            let checked = self
                .backup_to(&partial)
                .and_then(|()| repair_and_check(&partial));
            match checked {
                Ok(None) => {}
                Ok(Some(problem)) => {
                    let _ = std::fs::remove_file(&partial);
                    return Ok(BackupOutcome::SkippedDamaged(problem));
                }
                Err(err) => {
                    let _ = std::fs::remove_file(&partial);
                    return Err(err);
                }
            }
            std::fs::rename(&partial, &dest).map_err(|source| LibraryError::Backup {
                path: dest.clone(),
                source,
            })?;
        }
        prune(&dir, KEEP)?;
        Ok(BackupOutcome::Written(dest))
    }

    /// Writes a compacted, consistent copy of the database to `dest`, which
    /// must not exist yet.
    ///
    /// # Errors
    ///
    /// Fails if SQLite cannot write `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        self.conn
            .execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }

    /// `now` as a local `YYYYMMDD-HHMMSS` stamp, formatted by SQLite so the
    /// crate needs no date library.
    fn timestamp(&self, now: SystemTime) -> Result<String> {
        let secs = now
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let secs = i64::try_from(secs).unwrap_or(i64::MAX);
        Ok(self.conn.query_row(
            "SELECT strftime('%Y%m%d-%H%M%S', ?1, 'unixepoch', 'localtime')",
            [secs],
            |row| row.get(0),
        )?)
    }
}

/// Rebuilds the indexes of the copy at `path`, then returns its first
/// integrity problem, if any. The connection is closed on return, so the file
/// can be renamed or deleted.
fn repair_and_check(path: &Path) -> Result<Option<String>> {
    let copy = Connection::open(path)?;
    copy.execute_batch("REINDEX")?;
    integrity_problem(&copy)
}

/// SQLite's first `quick_check` complaint about `conn`'s database, or `None`
/// when it is healthy.
fn integrity_problem(conn: &Connection) -> Result<Option<String>> {
    let report: String = conn.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    Ok((report != "ok").then_some(report))
}

/// The backup folder for the database at `db`.
#[must_use]
pub fn backup_dir(db: &Path) -> PathBuf {
    db.parent().unwrap_or(Path::new("")).join(DIR_NAME)
}

/// The backups in `dir`, oldest first; empty if the folder does not exist.
fn list_backups(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut backups: Vec<PathBuf> = list_dir(dir)?
        .into_iter()
        .filter(|path| is_backup(path))
        .collect();
    backups.sort();
    Ok(backups)
}

/// Every entry in `dir`; empty if the folder does not exist. An unreadable
/// entry is an error rather than skipped, so the due check and the rotation
/// never work from an incomplete list.
fn list_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let error = |source| LibraryError::Backup {
        path: dir.to_path_buf(),
        source,
    };
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(error(source)),
    };
    entries
        .map(|entry| entry.map(|entry| entry.path()).map_err(error))
        .collect()
}

/// Deletes copies left half-written by an interrupted backup.
fn remove_partials(dir: &Path) -> Result<()> {
    for path in list_dir(dir)? {
        let partial = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(PREFIX) && name.ends_with(PARTIAL));
        if partial && let Err(err) = std::fs::remove_file(&path) {
            tracing::warn!(%err, path = %path.display(), "could not delete a partial library backup");
        }
    }
    Ok(())
}

/// Whether `path` is named like a backup this module wrote.
fn is_backup(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PREFIX) && name.ends_with(EXTENSION))
}

/// Whether the backup at `newest` is old enough for another one. A backup
/// whose age cannot be read counts as due, and so does one dated more than an
/// [`INTERVAL`] ahead of `now` (the clock was set back); a backup a moment in
/// the future is just the one written after `now` was taken.
fn is_due(newest: &Path, now: SystemTime) -> bool {
    let Ok(modified) = std::fs::metadata(newest).and_then(|meta| meta.modified()) else {
        return true;
    };
    match now.duration_since(modified) {
        Ok(age) => age >= INTERVAL,
        Err(ahead) => ahead.duration() >= INTERVAL,
    }
}

/// Deletes all but the newest `keep` backups in `dir`.
fn prune(dir: &Path, keep: usize) -> Result<()> {
    let backups = list_backups(dir)?;
    let excess = backups.len().saturating_sub(keep);
    for old in &backups[..excess] {
        if let Err(err) = std::fs::remove_file(old) {
            tracing::warn!(%err, path = %old.display(), "could not delete an old library backup");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
