//! Integration tests for the rendition cache: key stability, atomic writes and
//! LRU eviction.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use emusic_render::{Codec, RenditionCache, RenditionKey};

/// A unique temporary root, cleaned up on drop.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "emusic-render-{tag}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp root");
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(track: &str, subtune: u16) -> RenditionKey<'_> {
    RenditionKey::new(track, subtune, Codec::Flac, "sid", 1)
}

fn set_modified(path: &Path, time: SystemTime) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open rendition")
        .set_modified(time)
        .expect("set modified time");
}

/// Collects any leftover `.part.<pid>.<n>` temporary files under `root`.
fn temp_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(listing) = std::fs::read_dir(&directory) else {
            continue;
        };
        for item in listing.flatten() {
            let path = item.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.to_string_lossy().contains(".part.") {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn digest_is_stable_and_sensitive_to_every_field() {
    let baseline = key("abc", 1).digest();
    assert_eq!(baseline, key("abc", 1).digest());
    assert_ne!(baseline, key("abd", 1).digest());
    assert_ne!(baseline, key("abc", 2).digest());
    assert_ne!(
        baseline,
        RenditionKey::new("abc", 1, Codec::Flac, "mod", 1).digest()
    );
    assert_ne!(
        baseline,
        RenditionKey::new("abc", 1, Codec::Flac, "sid", 2).digest()
    );
}

#[test]
fn path_is_deterministic_under_the_root() {
    let root = TempRoot::new("path");
    let cache = RenditionCache::new(root.path(), u64::MAX);
    let first = cache.path_for(&key("abc", 1)).unwrap();
    assert_eq!(first, cache.path_for(&key("abc", 1)).unwrap());
    assert!(first.starts_with(root.path()));
    assert!(first.to_string_lossy().ends_with(".flac"));
}

#[test]
fn put_then_get_round_trips_and_leaves_no_temp_file() {
    let root = TempRoot::new("put");
    let cache = RenditionCache::new(root.path(), u64::MAX);
    let path = cache.put(&key("abc", 1), b"rendition-bytes").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"rendition-bytes");
    assert_eq!(cache.get(&key("abc", 1)).unwrap(), Some(path));
    assert_eq!(cache.get(&key("abc", 2)).unwrap(), None);
    assert!(temp_files(&root.path()).is_empty());
}

// Ignored on macOS CI: the uniquely-named `.part` temp file is intermittently
// reported `NotFound` while being created/written, even though nothing else
// removes it. Tracked in #465; remove the `#[ignore]` once that is fixed.
#[test]
#[ignore = "flaky on macOS; see #465"]
fn concurrent_writers_never_leave_a_partial_file() {
    let root = TempRoot::new("concurrent");
    let cache = Arc::new(RenditionCache::new(root.path(), u64::MAX));
    let mut handles = Vec::new();
    for writer in 0..8u8 {
        let cache = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            let payload = vec![writer; 1_024];
            cache.put(&key("abc", 1), &payload).expect("put")
        }));
    }
    for handle in handles {
        handle.join().expect("writer thread");
    }
    let path = cache.path_for(&key("abc", 1)).unwrap();
    let content = std::fs::read(&path).unwrap();
    assert_eq!(content.len(), 1_024);
    assert!(content.iter().all(|&byte| byte == content[0]));
    assert!(temp_files(&root.path()).is_empty());
}

#[test]
fn evicts_least_recently_used_until_under_the_cap() {
    let root = TempRoot::new("evict");
    let writer = RenditionCache::new(root.path(), u64::MAX);
    let oldest = writer.put(&key("a", 1), &[0u8; 10]).unwrap();
    let middle = writer.put(&key("b", 1), &[0u8; 10]).unwrap();
    let newest = writer.put(&key("c", 1), &[0u8; 10]).unwrap();

    let now = SystemTime::now();
    set_modified(&oldest, now - Duration::from_secs(300));
    set_modified(&middle, now - Duration::from_secs(200));
    set_modified(&newest, now - Duration::from_secs(100));

    let cache = RenditionCache::new(root.path(), 25);
    assert_eq!(cache.evict().unwrap(), 10);
    assert!(!oldest.exists(), "oldest must be evicted");
    assert!(middle.exists());
    assert!(newest.exists());
    assert_eq!(cache.total_bytes().unwrap(), 20);
}
