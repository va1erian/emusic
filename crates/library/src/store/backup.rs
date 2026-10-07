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
//! up, repaired.

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
        let dest = dir.join(format!("{PREFIX}{}{EXTENSION}", self.timestamp(now)?));
        if !dest.exists() {
            self.backup_to(&dest)?;
        }
        if let Some(problem) = repair_and_check(&dest)? {
            let _ = std::fs::remove_file(&dest);
            return Ok(BackupOutcome::SkippedDamaged(problem));
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
/// integrity problem, if any.
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
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(LibraryError::Backup {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut backups: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_backup(path))
        .collect();
    backups.sort();
    Ok(backups)
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
