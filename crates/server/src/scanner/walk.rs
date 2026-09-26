//! Directory walking for one library root.
//!
//! The walk collects file metadata only; nothing is opened here. A root that
//! cannot be stat'd, or a subdirectory that cannot be listed, marks the walk
//! partial so its existing rows are never deleted on the strength of a scan
//! that could not see everything.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::formats::{self, FormatInfo};

/// One supported file discovered under a root.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the root, using forward slashes.
    pub relative_path: String,
    /// File size in bytes.
    pub size: u64,
    /// Modification time as Unix seconds.
    pub mtime: i64,
    /// Classified format.
    pub format: FormatInfo,
}

/// Result of walking a single root.
#[derive(Debug, Default)]
pub struct RootWalk {
    /// Supported files found.
    pub files: Vec<FoundFile>,
    /// Whether the root itself could not be accessed.
    pub unreachable: bool,
    /// Whether some subtree could not be listed.
    pub partial: bool,
}

impl RootWalk {
    /// Whether deletions may be derived from this walk.
    pub fn can_delete(&self) -> bool {
        !self.unreachable && !self.partial
    }
}

/// Walks `root`, classifying every supported audio file.
pub fn walk_root(root: &Path) -> RootWalk {
    let mut outcome = RootWalk::default();
    if std::fs::metadata(root).is_err() {
        outcome.unreachable = true;
        return outcome;
    }

    for entry in WalkDir::new(root) {
        match entry {
            Ok(entry) => {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                let Some(ext) = formats::extension_of(path) else {
                    continue;
                };
                let Some(format) = formats::classify(&ext) else {
                    continue;
                };
                let Ok(metadata) = entry.metadata() else {
                    // The file exists but we cannot stat it: do not let its
                    // row be deleted as "missing".
                    outcome.partial = true;
                    continue;
                };
                let Some(relative_path) = relative_slash_path(root, path) else {
                    outcome.partial = true;
                    continue;
                };
                outcome.files.push(FoundFile {
                    path: path.to_path_buf(),
                    relative_path,
                    size: metadata.len(),
                    mtime: mtime_of(&metadata),
                    format,
                });
            }
            Err(error) => {
                let path = error.path().map(Path::to_path_buf);
                if path.as_deref() == Some(root) {
                    outcome.unreachable = true;
                    return outcome;
                }
                outcome.partial = true;
            }
        }
    }
    outcome
}

/// The path of `path` relative to `root`, with forward slashes.
fn relative_slash_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn mtime_of(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "emusic-srv-walk-{name}-{}-{}",
            std::process::id(),
            crate::util::unix_now()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_and_classifies_supported_files() {
        let root = temp_root("find");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.flac"), b"x").unwrap();
        std::fs::write(root.join("b.txt"), b"x").unwrap();
        std::fs::write(root.join("sub/c.xm"), b"x").unwrap();
        std::fs::write(root.join("sub/d.sid"), b"x").unwrap();

        let walk = walk_root(&root);
        assert_eq!(walk.files.len(), 3);
        assert!(walk.can_delete());
        let names: Vec<&str> = walk
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert!(names.contains(&"a.flac"));
        assert!(names.contains(&"sub/c.xm"));
        assert!(names.contains(&"sub/d.sid"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_root_is_unreachable() {
        let walk = walk_root(Path::new("does/not/exist"));
        assert!(walk.unreachable);
        assert!(!walk.can_delete());
    }
}
