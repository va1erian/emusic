//! The payload exchanged between a secondary instance and the primary.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A request sent by a secondary process to the primary instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcMessage {
    /// `true` to add `files` to the play queue instead of replacing it and
    /// starting playback.
    pub enqueue: bool,
    /// The file paths the secondary process was launched with (e.g. via
    /// Explorer's "Open with").
    pub files: Vec<PathBuf>,
    /// The secondary process's working directory, in case `files` contains
    /// relative paths.
    pub cwd: PathBuf,
}

impl IpcMessage {
    /// Merges `other` into `self`, as done when several messages arrive
    /// within the primary's batching window (Explorer spawns one process
    /// per file on a multi-select "Open with").
    ///
    /// `enqueue` becomes `true` if either message requested it; `cwd` is
    /// kept from `self` (the earliest message in the batch).
    pub(crate) fn merge(&mut self, other: IpcMessage) {
        self.enqueue |= other.enqueue;
        self.files.extend(other.files);
    }
}
