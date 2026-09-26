//! Stream cache manager for remote files fetched from `emusic-server`.

use std::path::{Path, PathBuf};

pub struct StreamCacheManager {
    cache_dir: PathBuf,
}

impl StreamCacheManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        Self { cache_dir }
    }

    pub fn cached_track_path(&self, track_id: &str, ext: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.{}", track_id, ext))
    }

    pub fn is_cached(&self, track_id: &str, ext: &str) -> bool {
        self.cached_track_path(track_id, ext).exists()
    }
}
