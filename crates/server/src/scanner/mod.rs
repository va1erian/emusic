//! Library scanning: walk configured roots, extract metadata and persist
//! delta-syncable rows.
//!
//! The scanner is synchronous and intended to run inside `spawn_blocking`.
//! Progress is reported through a callback so the async layer can fan it out
//! over WebSocket without the scanner knowing about tokio.

pub mod art;
pub mod formats;
pub mod index;
pub mod module;
pub mod sid;
pub mod songlengths;
pub mod tags;
pub mod walk;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::db::Db;
use crate::error::Result;
use crate::util::unix_now;

pub use songlengths::SongLengths;

/// Default number of rows committed per database transaction.
pub const DEFAULT_BATCH_SIZE: usize = 500;

/// The scanner's immutable configuration.
pub struct Scanner {
    roots: Vec<PathBuf>,
    songlengths: Arc<SongLengths>,
}

impl Scanner {
    /// Builds a scanner over `roots` with a SID song-length table.
    pub fn new(roots: Vec<PathBuf>, songlengths: Arc<SongLengths>) -> Self {
        Self { roots, songlengths }
    }

    /// Number of configured roots.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// Scans every root, persisting changes, and returns a summary.
    ///
    /// `on_update` is called after each committed batch and records the root
    /// being scanned and the running file count.
    pub fn scan(
        &self,
        db: &Db,
        batch_size: usize,
        on_update: &mut dyn FnMut(ScanUpdate),
    ) -> Result<ScanReport> {
        let started = Instant::now();
        let now = unix_now();
        let batch_size = batch_size.max(1);
        let mut report = ScanReport::default();

        for (root_index, root) in self.roots.iter().enumerate() {
            let walked = walk::walk_root(root);
            if walked.unreachable {
                report.unreachable_roots.push(root.clone());
                report.partial = true;
                continue;
            }
            if walked.partial {
                report.partial = true;
            }
            report.files_found += walked.files.len() as u64;

            let mut seen = HashSet::with_capacity(walked.files.len());
            let mut art = index::ArtCache::new();
            let mut batch = Vec::with_capacity(batch_size);
            for file in &walked.files {
                seen.insert(index::track_id(root_index as i64, &file.relative_path));
                let (track, skipped) =
                    index::index_file(file, root_index as i64, &self.songlengths, &mut art, now);
                if skipped {
                    report.files_skipped += 1;
                }
                batch.push(track);
                if batch.len() >= batch_size {
                    report.tracks_changed += db.upsert_tracks(&batch)? as u64;
                    batch.clear();
                    on_update(ScanUpdate {
                        root_index,
                        root: root.clone(),
                        files_found: report.files_found,
                    });
                }
            }
            if !batch.is_empty() {
                report.tracks_changed += db.upsert_tracks(&batch)? as u64;
            }

            if walked.can_delete() {
                let removed = missing_ids(db, root_index as i64, &seen)?;
                report.tracks_deleted += db.delete_tracks(&removed)? as u64;
            }

            on_update(ScanUpdate {
                root_index,
                root: root.clone(),
                files_found: report.files_found,
            });
        }

        report.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(report)
    }
}

fn missing_ids(db: &Db, root_index: i64, seen: &HashSet<String>) -> Result<Vec<String>> {
    let existing = db.track_ids_for_root(root_index)?;
    Ok(existing
        .into_iter()
        .filter(|id| !seen.contains(id))
        .collect())
}

/// Progress notification emitted during a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanUpdate {
    /// Index of the root currently being scanned.
    pub root_index: usize,
    /// The root currently being scanned.
    pub root: PathBuf,
    /// Running total of supported files seen.
    pub files_found: u64,
}

/// Summary of a completed scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Supported files discovered.
    pub files_found: u64,
    /// Rows inserted or updated.
    pub tracks_changed: u64,
    /// Rows deleted and tombstoned.
    pub tracks_deleted: u64,
    /// Files whose tags could not be parsed.
    pub files_skipped: u64,
    /// Roots that could not be accessed.
    pub unreachable_roots: Vec<PathBuf>,
    /// Whether some part of the library could not be seen.
    pub partial: bool,
    /// Wall-clock duration in milliseconds.
    pub elapsed_ms: u64,
}
