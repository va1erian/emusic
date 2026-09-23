//! Orchestrating a tag edit across the file and the store.
//!
//! [`edit_tags`] is the seam the tag editor calls: for each request it writes
//! the tags to disk with [`write_tags`], re-stats the file, and then syncs the
//! changed rows with the store in one transaction. A request that fails — a
//! missing or read-only file — yields an [`EditOutcome`] error without
//! disturbing the rest of the batch.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use tracing::error;

use crate::error::{LibraryError, Result};
use crate::store::Store;

use super::{EditableTags, write_tags};

/// A request to write `tags` to the audio file at `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRequest {
    /// The file to rewrite.
    pub path: PathBuf,
    /// The scalar tag fields to write.
    pub tags: EditableTags,
}

impl EditRequest {
    /// Creates a request to write `tags` to the file at `path`.
    pub fn new(path: impl Into<PathBuf>, tags: EditableTags) -> Self {
        Self {
            path: path.into(),
            tags,
        }
    }
}

/// The outcome of one [`EditRequest`].
#[derive(Debug)]
pub struct EditOutcome {
    /// The file the request targeted.
    pub path: PathBuf,
    /// `Ok(())` once the file was rewritten, re-statted and its store row
    /// synced; otherwise the error that stopped the edit.
    pub result: Result<()>,
}

/// A file that was rewritten on disk and is ready to be stored.
pub(crate) struct EditedFile {
    /// The file that was rewritten.
    pub path: PathBuf,
    /// The tags written to it.
    pub tags: EditableTags,
    /// The file's size after the write.
    pub size: u64,
    /// The file's mtime (Unix seconds) after the write.
    pub mtime: i64,
}

/// Applies every request, rewriting the file and syncing its store row.
///
/// Each request is independent: a missing or read-only file produces an
/// [`EditOutcome`] with an error and leaves its store row untouched, while the
/// rest of the batch proceeds. The rows of every request that succeeded are
/// written together in one store transaction, so the database sees a single
/// atomic update rather than one write per file. If that final write fails,
/// the files have still been edited and the failure is logged.
pub fn edit_tags(store: &mut Store, requests: &[EditRequest]) -> Vec<EditOutcome> {
    let mut outcomes = Vec::with_capacity(requests.len());
    let mut edited = Vec::new();

    for request in requests {
        match write_and_stat(&request.path, &request.tags) {
            Ok((size, mtime)) => {
                edited.push(EditedFile {
                    path: request.path.clone(),
                    tags: request.tags.clone(),
                    size,
                    mtime,
                });
                outcomes.push(EditOutcome {
                    path: request.path.clone(),
                    result: Ok(()),
                });
            }
            Err(error) => outcomes.push(EditOutcome {
                path: request.path.clone(),
                result: Err(error),
            }),
        }
    }

    if !edited.is_empty()
        && let Err(error) = store.store_tag_edits(&edited)
    {
        error!(%error, "failed to sync edited tag rows");
    }

    outcomes
}

/// Writes `tags` to `path` and returns the file's refreshed `(size, mtime)`.
fn write_and_stat(path: &Path, tags: &EditableTags) -> Result<(u64, i64)> {
    write_tags(path, tags)?;
    let metadata = std::fs::metadata(path).map_err(|source| LibraryError::StatFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok((metadata.len(), mtime_of(&metadata)))
}

/// The file's mtime as a Unix timestamp (seconds), or 0 when unavailable.
fn mtime_of(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
