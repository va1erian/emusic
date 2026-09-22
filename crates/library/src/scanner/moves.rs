//! Move/rename detection: classifying walked files against the store and
//! finding stored rows whose files vanished, safely.

use std::collections::HashMap;
use std::path::PathBuf;

use super::paths;
use super::progress::CancelToken;
use super::walk::{FoundFile, WalkOutcome};

/// A candidate file whose tags must be (re)read, plus the stored row it
/// replaces, if any.
pub(super) struct PendingRead {
    pub(super) file: FoundFile,
    /// The stored path of the row whose normalised key matches this file;
    /// `None` for files new to the library.
    pub(super) prior_path: Option<PathBuf>,
}

/// Splits walked files into unchanged (dropped) and new/changed (to read),
/// tagging each candidate with the stored row it replaces, matched by
/// normalised path key.
pub(super) fn classify_files(
    files: Vec<FoundFile>,
    existing: &HashMap<PathBuf, (u64, i64)>,
) -> Vec<PendingRead> {
    let mut prior_by_key: HashMap<String, (PathBuf, u64, i64)> =
        HashMap::with_capacity(existing.len());
    for (path, &(size, mtime)) in existing {
        prior_by_key
            .entry(paths::normalize_key(path))
            .or_insert((path.clone(), size, mtime));
    }

    let mut pending = Vec::new();
    for file in files {
        let key = paths::normalize_key(&file.path);
        match prior_by_key.get(&key) {
            Some((prior, size, mtime)) if *size == file.size && *mtime == file.mtime => {}
            Some((prior, ..)) => pending.push(PendingRead {
                file,
                prior_path: Some(prior.clone()),
            }),
            None => pending.push(PendingRead {
                file,
                prior_path: None,
            }),
        }
    }
    pending
}

/// Stored rows whose files were not seen on disk — but only those safe to
/// act on: under a root that was walked completely, and not under a
/// directory that failed to list. Empty when the walk was cancelled (the
/// "seen" set would be incomplete).
pub(super) fn vanished_rows(
    existing: &HashMap<PathBuf, (u64, i64)>,
    outcome: &WalkOutcome,
    cancel: &CancelToken,
) -> Vec<PathBuf> {
    if cancel.is_cancelled() {
        return Vec::new();
    }
    let scanned_root_keys: Vec<String> = outcome
        .scanned_roots
        .iter()
        .map(|root| paths::normalize_key(root))
        .collect();
    let failed_keys: Vec<String> = outcome
        .failed_subtrees
        .iter()
        .map(|dir| paths::normalize_key(dir))
        .collect();

    let mut vanished = Vec::new();
    for path in existing.keys() {
        let key = paths::normalize_key(path);
        if outcome.seen_keys.contains(&key) {
            continue;
        }
        if !scanned_root_keys
            .iter()
            .any(|root| paths::key_is_under(&key, root))
        {
            continue;
        }
        if failed_keys
            .iter()
            .any(|failed| paths::key_is_under(&key, failed))
        {
            continue;
        }
        vanished.push(path.clone());
    }
    vanished.sort();
    vanished
}
