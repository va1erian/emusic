//! Walking library roots with `walkdir`, collecting file metadata only.
//!
//! The walk relies exclusively on directory metadata (size + mtime) gathered
//! by `walkdir` while descending; nothing is opened or read here. Roots that
//! cannot be accessed are skipped whole — their rows must never be deleted —
//! and individual unreadable subdirectories are recorded as "failed
//! subtrees" so rows underneath them are protected from deletion too.

use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::UNIX_EPOCH;

use tracing::warn;
use walkdir::WalkDir;

use emusic_core::TrackKind;

use super::paths::{classify, normalize_key};
use super::progress::{CancelToken, ScanEvent};

/// One supported audio file discovered on disk.
#[derive(Debug, Clone)]
pub(crate) struct FoundFile {
    pub path: PathBuf,
    pub size: u64,
    /// File modification time as a Unix timestamp (seconds, UTC).
    pub mtime: i64,
    /// Whether this is streamed audio or a tracker module.
    pub kind: TrackKind,
}

/// Everything a walk over the library roots produced.
#[derive(Debug, Default)]
pub(crate) struct WalkOutcome {
    /// Supported files, sorted by path and deduplicated by normalised key
    /// (in case roots overlap).
    pub files: Vec<FoundFile>,
    /// Normalised keys of every file found, used to detect vanished rows.
    pub seen_keys: HashSet<String>,
    /// Roots that were walked to completion.
    pub scanned_roots: Vec<PathBuf>,
    /// Roots that could not be accessed at all.
    pub unreachable_roots: Vec<PathBuf>,
    /// Directories that could not be listed. Rows underneath them must not
    /// be deleted: their files may exist but be unreadable.
    pub failed_subtrees: Vec<PathBuf>,
    /// Whether the walk could not see everything (unreachable roots,
    /// failed subtrees).
    pub partial: bool,
}

/// Walks every root, sending [`ScanEvent::FilesFound`] as files appear.
///
/// Stops early if `cancel` is requested; roots after the stop point are
/// simply not visited (and their rows therefore not treated as vanished).
pub(crate) fn walk_roots(
    roots: &[PathBuf],
    cancel: &CancelToken,
    progress: &Sender<ScanEvent>,
) -> WalkOutcome {
    let mut outcome = WalkOutcome::default();
    let mut found: u64 = 0;
    for root in roots {
        if cancel.is_cancelled() {
            break;
        }
        walk_root(root, cancel, progress, &mut outcome, &mut found);
    }
    outcome.files.sort_by(|a, b| a.path.cmp(&b.path));
    outcome
}

/// Walks a single root, recording reachability problems in `outcome`.
fn walk_root(
    root: &Path,
    cancel: &CancelToken,
    progress: &Sender<ScanEvent>,
    outcome: &mut WalkOutcome,
    found: &mut u64,
) {
    // A root that cannot even be stat'd is unreachable: skip it entirely so
    // its rows are never treated as vanished.
    if std::fs::metadata(root).is_err() {
        warn!(root = %root.display(), "library root unreachable, skipping");
        outcome.unreachable_roots.push(root.to_path_buf());
        outcome.partial = true;
        let _ = progress.send(ScanEvent::RootUnreachable {
            root: root.to_path_buf(),
        });
        return;
    }

    for entry in WalkDir::new(root) {
        if cancel.is_cancelled() {
            return;
        }
        match entry {
            Ok(entry) => {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Some(kind) = classify(entry.path()) else {
                    continue;
                };
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let key = normalize_key(entry.path());
                if outcome.seen_keys.insert(key) {
                    outcome.files.push(FoundFile {
                        path: entry.path().to_path_buf(),
                        size: metadata.len(),
                        mtime: mtime_of(&metadata),
                        kind,
                    });
                    *found += 1;
                    let _ = progress.send(ScanEvent::FilesFound { count: *found });
                }
            }
            Err(error) => {
                let path = error
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf());
                if path == root {
                    warn!(root = %root.display(), %error, "library root unreadable");
                    outcome.unreachable_roots.push(root.to_path_buf());
                    outcome.partial = true;
                    let _ = progress.send(ScanEvent::RootUnreachable {
                        root: root.to_path_buf(),
                    });
                    return;
                }
                warn!(dir = %path.display(), %error, "directory unreadable, keeping its rows");
                outcome.failed_subtrees.push(path);
                outcome.partial = true;
            }
        }
    }

    outcome.scanned_roots.push(root.to_path_buf());
}

/// The file's mtime as a Unix timestamp (seconds), or 0 when unavailable.
fn mtime_of(metadata: &Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn walk(roots: &[PathBuf]) -> WalkOutcome {
        let (tx, _rx) = mpsc::channel();
        walk_roots(roots, &CancelToken::default(), &tx)
    }

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emusic-walk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_supported_files_with_metadata() {
        let root = temp_root("supported");
        std::fs::write(root.join("a.flac"), b"fake flac").unwrap();
        std::fs::write(root.join("b.txt"), b"not audio").unwrap();
        std::fs::write(root.join("c.xm"), b"fake module").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("d.mp3"), b"fake mp3").unwrap();

        let outcome = walk(std::slice::from_ref(&root));
        assert_eq!(outcome.files.len(), 3);
        assert_eq!(outcome.seen_keys.len(), 3);
        let names: Vec<String> = outcome
            .files
            .iter()
            .map(|file| {
                file.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["a.flac", "c.xm", "d.mp3"]);
        assert_eq!(outcome.files[0].size, 9);
        assert_eq!(outcome.files[1].kind, TrackKind::Module);
        assert_eq!(outcome.files[2].kind, TrackKind::Stream);
        assert_eq!(outcome.scanned_roots, vec![root.clone()]);
        assert!(!outcome.partial);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_root_is_unreachable_and_partial() {
        let missing = std::env::temp_dir().join("emusic-walk-does-not-exist");
        let outcome = walk(std::slice::from_ref(&missing));
        assert!(outcome.files.is_empty());
        assert_eq!(outcome.unreachable_roots, vec![missing]);
        assert!(outcome.scanned_roots.is_empty());
        assert!(outcome.partial);
    }

    #[test]
    fn duplicate_roots_yield_each_file_once() {
        let root = temp_root("dupes");
        std::fs::write(root.join("a.flac"), b"fake flac").unwrap();
        let outcome = walk(&[root.clone(), root.clone()]);
        assert_eq!(outcome.files.len(), 1);
        assert_eq!(outcome.seen_keys.len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cancellation_stops_the_walk_immediately() {
        let root = temp_root("cancel");
        std::fs::write(root.join("a.flac"), b"fake flac").unwrap();
        let cancel = CancelToken::default();
        cancel.cancel();
        let (tx, _rx) = mpsc::channel();
        let outcome = walk_roots(std::slice::from_ref(&root), &cancel, &tx);
        assert!(outcome.files.is_empty());
        assert!(outcome.scanned_roots.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }
}
