#![forbid(unsafe_code)]

//! A size-capped LRU cache of encoded renditions.
//!
//! Keys are deterministic from the track, subtune, codec and renderer version,
//! so concurrent requests for the same rendition resolve to the same path and
//! the server can coalesce them. Writes go to a unique temporary sibling and
//! are atomically renamed into place (modelled on `emusic_client::cache`), so a
//! crashed render never leaves a partial file for a decoder to open.

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use md5::{Digest, Md5};

use crate::codec::Codec;
use crate::error::RenderError;

/// Bumped if the key construction ever changes, so old keys miss.
const KEY_PREFIX: &str = "emusic-render/v1";

/// Everything that identifies a rendition.
///
/// `track` is a track id or content hash; `renderer` and `renderer_version`
/// together identify the rendering algorithm (see [`crate::Renderer`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenditionKey<'a> {
    /// Track id or content hash.
    pub track: &'a str,
    /// 1-based subtune.
    pub subtune: u16,
    /// Output codec.
    pub codec: Codec,
    /// Renderer family, for example `"sid"`.
    pub renderer: &'a str,
    /// Renderer implementation version.
    pub renderer_version: u32,
}

impl<'a> RenditionKey<'a> {
    /// Builds a key.
    pub fn new(
        track: &'a str,
        subtune: u16,
        codec: Codec,
        renderer: &'a str,
        renderer_version: u32,
    ) -> Self {
        Self {
            track,
            subtune,
            codec,
            renderer,
            renderer_version,
        }
    }

    /// A stable hex digest of every key field.
    pub fn digest(&self) -> String {
        let mut hasher = Md5::new();
        hasher.update(KEY_PREFIX.as_bytes());
        hasher.update([0]);
        hasher.update(self.track.as_bytes());
        hasher.update([0]);
        hasher.update(self.subtune.to_be_bytes());
        hasher.update([0]);
        hasher.update(self.codec.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(self.renderer.as_bytes());
        hasher.update([0]);
        hasher.update(self.renderer_version.to_be_bytes());
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// A directory holding encoded renditions.
#[derive(Debug, Clone)]
pub struct RenditionCache {
    root: PathBuf,
    max_bytes: u64,
}

impl RenditionCache {
    /// A cache rooted at `root` holding at most `max_bytes` of renditions.
    pub fn new(root: PathBuf, max_bytes: u64) -> Self {
        Self { root, max_bytes }
    }

    /// The cache root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The size cap, in bytes.
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// The deterministic path for a rendition: `<root>/<aa>/<digest>.<ext>`.
    pub fn path_for(&self, key: &RenditionKey<'_>) -> Result<PathBuf, RenderError> {
        let digest = key.digest();
        let path = self
            .root
            .join(&digest[..2])
            .join(format!("{digest}.{}", key.codec.extension()));
        ensure_inside(&self.root, &path)
    }

    /// Returns the cached path for a key, marking it as recently used.
    pub fn get(&self, key: &RenditionKey<'_>) -> Result<Option<PathBuf>, RenderError> {
        let path = self.path_for(key)?;
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                self.touch(&path)?;
                Ok(Some(path))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(RenderError::io(&path, error)),
        }
    }

    /// Atomically stores `bytes` and evicts down to the size cap.
    pub fn put(&self, key: &RenditionKey<'_>, bytes: &[u8]) -> Result<PathBuf, RenderError> {
        let destination = self.path_for(key)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| RenderError::io(parent, error))?;
        }
        let temporary = unique_temp(&destination);
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(&temporary)?;
            std::io::Write::write_all(&mut file, bytes)?;
            file.sync_all()
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(RenderError::io(&temporary, error));
        }
        replace_file(&temporary, &destination)?;
        self.evict()?;
        Ok(destination)
    }

    /// The total size of the cached renditions, in bytes.
    pub fn total_bytes(&self) -> Result<u64, RenderError> {
        Ok(self.entries()?.into_iter().map(|entry| entry.len).sum())
    }

    /// Removes least-recently-used renditions until the size cap is met,
    /// returning how many bytes were freed.
    pub fn evict(&self) -> Result<u64, RenderError> {
        let mut entries = self.entries()?;
        let mut total: u64 = entries.iter().map(|entry| entry.len).sum();
        if total <= self.max_bytes {
            return Ok(0);
        }
        entries.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| a.path.cmp(&b.path))
        });
        let mut freed = 0;
        for entry in entries {
            if total <= self.max_bytes {
                break;
            }
            match std::fs::remove_file(&entry.path) {
                Ok(()) => {
                    total = total.saturating_sub(entry.len);
                    freed += entry.len;
                    tracing::debug!(path = %entry.path.display(), "evicted rendition");
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(RenderError::io(&entry.path, error)),
            }
        }
        Ok(freed)
    }

    /// Marks a cached file as most recently used.
    fn touch(&self, path: &Path) -> Result<(), RenderError> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|error| RenderError::io(path, error))?;
        file.set_modified(SystemTime::now())
            .map_err(|error| RenderError::io(path, error))
    }

    /// Every rendition file, with its size and last-modified time.
    fn entries(&self) -> Result<Vec<Entry>, RenderError> {
        let mut entries = Vec::new();
        let mut pending = vec![self.root.clone()];
        while let Some(directory) = pending.pop() {
            let listing = match std::fs::read_dir(&directory) {
                Ok(listing) => listing,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(RenderError::io(&directory, error)),
            };
            for item in listing {
                let item = item.map_err(|error| RenderError::io(&directory, error))?;
                let path = item.path();
                let metadata = item
                    .metadata()
                    .map_err(|error| RenderError::io(&path, error))?;
                if metadata.is_dir() {
                    pending.push(path);
                } else if metadata.is_file() {
                    entries.push(Entry {
                        path,
                        len: metadata.len(),
                        modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    });
                }
            }
        }
        Ok(entries)
    }
}

/// A rendition file and its eviction metadata.
struct Entry {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
}

/// A unique sibling path for an in-progress write.
fn unique_temp(destination: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rendition".to_string());
    destination.with_file_name(format!("{name}.part.{}.{sequence}", std::process::id()))
}

/// Replaces `destination` with `source`, tolerating concurrent writers.
///
/// On Unix `rename` atomically replaces the destination. On Windows it fails
/// when the destination exists, so remove and retry, tolerating another writer
/// having removed or created it in between.
fn replace_file(source: &Path, destination: &Path) -> Result<(), RenderError> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            match std::fs::remove_file(destination) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(RenderError::io(destination, error)),
            }
            if std::fs::rename(source, destination).is_ok() {
                return Ok(());
            }
            // A concurrent writer won; its complete file is already in place.
            let _ = std::fs::remove_file(source);
            Ok(())
        }
        Err(error) => Err(RenderError::io(destination, error)),
    }
}

/// Rejects a path that is not lexically inside `root`.
fn ensure_inside(root: &Path, path: &Path) -> Result<PathBuf, RenderError> {
    let inside = path.starts_with(root)
        && path.strip_prefix(root).is_ok_and(|rest| {
            rest.components()
                .all(|component| matches!(component, Component::Normal(_)))
        });
    if inside {
        Ok(path.to_path_buf())
    } else {
        Err(RenderError::Cache(format!(
            "refusing path outside the cache: {}",
            path.display()
        )))
    }
}
