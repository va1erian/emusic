//! Album-thumbnail cache (#17).
//!
//! Thumbnails are decoded from embedded/folder artwork on a small worker
//! pool, never on the UI thread. The raw result is cached on disk under
//! `%LOCALAPPDATA%\emusic\thumbs\<hash>.jpg` (about 200 px on the long edge)
//! and the uploaded GPU textures are kept in a bounded LRU.
//!
//! Until a thumbnail is ready the caller draws a placeholder, so this module
//! simply reports "not ready" (`None`) for missing or in-flight artwork.
//! Mock library paths do not exist on disk and are never requested, which
//! keeps headless screenshots deterministic.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::panels::now_playing::load_artwork;

/// Longest edge of a cached thumbnail, in pixels.
const THUMB_SIZE: u32 = 200;
/// Maximum number of GPU textures kept alive; older ones are evicted.
const MAX_TEXTURES: usize = 300;
/// Maximum number of texture uploads (`ctx.load_texture`) performed per frame.
///
/// A fast scroll reveals a burst of tiles at once, so many decodes can finish
/// around the same time. Uploading all of them in one frame stalls it, so the
/// rest stay queued in [`ThumbnailCache::pending`] and are uploaded over the
/// following frames instead.
const MAX_UPLOADS_PER_FRAME: usize = 4;
/// Number of threads decoding/encoding thumbnails.
const WORKERS: usize = 2;

/// A decode request handed to the worker pool.
struct Job {
    key: u64,
    source: PathBuf,
    ctx: egui::Context,
}

/// A finished decode, sent back to the UI thread.
struct Finished {
    key: u64,
    image: Option<ColorImage>,
}

/// One cached GPU texture plus its LRU timestamp.
struct Entry {
    texture: TextureHandle,
    used: u64,
}

/// Bounded LRU cache of album thumbnails.
#[derive(Default)]
pub struct ThumbnailCache {
    entries: HashMap<u64, Entry>,
    loading: HashSet<u64>,
    failed: HashSet<u64>,
    jobs: Option<Sender<Job>>,
    finished: Option<Receiver<Finished>>,
    /// Decodes finished but not yet uploaded, kept across frames so the
    /// per-frame upload cap can be respected without losing work.
    pending: VecDeque<Finished>,
    /// Monotonic LRU clock; a larger value means more recently used.
    clock: u64,
}

impl ThumbnailCache {
    /// Uploads thumbnails the workers finished and trims the LRU. Call once
    /// per frame before [`Self::get`].
    ///
    /// At most [`MAX_UPLOADS_PER_FRAME`] textures are uploaded per call; the
    /// rest stay queued and a repaint is requested while work remains, so a
    /// burst of decodes is spread over several frames instead of stalling one.
    pub fn drain(&mut self, ctx: &egui::Context) {
        if let Some(finished) = self.finished.as_ref() {
            while let Ok(result) = finished.try_recv() {
                self.pending.push_back(result);
            }
        }

        let mut uploaded = 0;
        while let Some(result) = self.pending.pop_front() {
            match result.image {
                Some(image) if uploaded < MAX_UPLOADS_PER_FRAME => {
                    self.loading.remove(&result.key);
                    self.insert(result.key, image, ctx);
                    uploaded += 1;
                }
                Some(_) => {
                    // Cap reached: keep this one and the rest for later frames.
                    self.pending.push_front(result);
                    break;
                }
                None => {
                    self.loading.remove(&result.key);
                    self.failed.insert(result.key);
                }
            }
        }

        self.evict();
        if !self.pending.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Returns the texture for `source` if ready, requesting it otherwise.
    ///
    /// `source` is the file a cover is read from (a representative track or a
    /// folder image); paths that do not exist - e.g. mock data - are never
    /// requested, so the caller keeps drawing its placeholder.
    pub fn get(&mut self, ctx: &egui::Context, source: &str) -> Option<&TextureHandle> {
        let key = hash(source);
        if self.entries.contains_key(&key) {
            self.clock += 1;
            let used = self.clock;
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.used = used;
            }
            return self.entries.get(&key).map(|entry| &entry.texture);
        }
        if self.loading.contains(&key) || self.failed.contains(&key) {
            return None;
        }
        let path = Path::new(source);
        if !path.exists() {
            return None;
        }
        self.request(ctx, key, path);
        None
    }

    fn insert(&mut self, key: u64, image: ColorImage, ctx: &egui::Context) {
        self.clock += 1;
        let texture = ctx.load_texture(
            format!("album_thumb_{key:016x}"),
            image,
            TextureOptions::LINEAR,
        );
        self.entries.insert(
            key,
            Entry {
                texture,
                used: self.clock,
            },
        );
    }

    fn request(&mut self, ctx: &egui::Context, key: u64, source: &Path) {
        if !self.loading.insert(key) {
            return;
        }
        let sender = self.ensure_pool();
        let job = Job {
            key,
            source: source.to_path_buf(),
            ctx: ctx.clone(),
        };
        if sender.send(job).is_err() {
            self.loading.remove(&key);
        }
    }

    /// Lazily starts the worker pool on the first real request, so a UI with
    /// no on-disk artwork (mock data, screenshots, tests) spawns no threads.
    fn ensure_pool(&mut self) -> Sender<Job> {
        if let Some(sender) = &self.jobs {
            return sender.clone();
        }
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done_rx) = mpsc::channel::<Finished>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for _ in 0..WORKERS {
            let job_rx = Arc::clone(&job_rx);
            let done_tx = done_tx.clone();
            std::thread::spawn(move || worker(job_rx, done_tx));
        }
        self.jobs = Some(job_tx.clone());
        self.finished = Some(done_rx);
        job_tx
    }

    fn evict(&mut self) {
        while self.entries.len() > MAX_TEXTURES {
            let Some(&key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key)
            else {
                break;
            };
            self.entries.remove(&key);
        }
    }
}

/// Pulls jobs off the shared queue until the sender is dropped.
fn worker(jobs: Arc<Mutex<Receiver<Job>>>, done: Sender<Finished>) {
    loop {
        let job = {
            let Ok(jobs) = jobs.lock() else { return };
            let Ok(job) = jobs.recv() else { return };
            job
        };
        let image = load_thumbnail(&job.source);
        if done
            .send(Finished {
                key: job.key,
                image,
            })
            .is_err()
        {
            return;
        }
        job.ctx.request_repaint();
    }
}

/// Reads the disk cache, falling back to decoding the source artwork and
/// writing the resized result back to the cache.
fn load_thumbnail(source: &Path) -> Option<ColorImage> {
    if let Some(image) = read_cache(source) {
        return Some(image);
    }
    let image = load_artwork(source, source.parent())?;
    let thumbnail = image.thumbnail(THUMB_SIZE, THUMB_SIZE);
    write_cache(source, &thumbnail);
    Some(to_color_image(thumbnail))
}

/// `%LOCALAPPDATA%\emusic\thumbs\<hash>.jpg`, or `None` if there is no local
/// data directory (thumbnails are then decoded on every request).
fn cache_path(source: &Path) -> Option<PathBuf> {
    let dir = dirs::data_local_dir()?.join("emusic").join("thumbs");
    Some(dir.join(format!("{:016x}.jpg", hash(&source.to_string_lossy()))))
}

fn read_cache(source: &Path) -> Option<ColorImage> {
    image::open(cache_path(source)?).ok().map(to_color_image)
}

fn write_cache(source: &Path, image: &image::DynamicImage) {
    let Some(path) = cache_path(source) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = image
        .to_rgb8()
        .save_with_format(&path, image::ImageFormat::Jpeg);
}

fn to_color_image(image: image::DynamicImage) -> ColorImage {
    let rgba = image.to_rgba8();
    ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba)
}

/// 64-bit hash of a source path, used as both the texture key and the disk
/// cache file name. `DefaultHasher` is not guaranteed stable across Rust
/// releases, which only means the cache is rebuilt after a toolchain change.
fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> ColorImage {
        ColorImage::from_rgba_unmultiplied([1, 1], &[255, 0, 0, 255])
    }

    fn queue(
        cache: &mut ThumbnailCache,
        keys: impl IntoIterator<Item = (u64, Option<ColorImage>)>,
    ) {
        for (key, image) in keys {
            cache.pending.push_back(Finished { key, image });
        }
    }

    #[test]
    fn drain_caps_uploads_per_call_and_keeps_the_rest_queued() {
        let ctx = egui::Context::default();
        let mut cache = ThumbnailCache::default();
        let total = MAX_UPLOADS_PER_FRAME * 3;
        queue(
            &mut cache,
            (0..total as u64).map(|key| (key, Some(image()))),
        );

        cache.drain(&ctx);
        assert_eq!(cache.entries.len(), MAX_UPLOADS_PER_FRAME);
        assert_eq!(cache.pending.len(), total - MAX_UPLOADS_PER_FRAME);

        cache.drain(&ctx);
        assert_eq!(cache.entries.len(), MAX_UPLOADS_PER_FRAME * 2);

        cache.drain(&ctx);
        assert_eq!(cache.entries.len(), total);
        assert!(cache.pending.is_empty());
    }

    #[test]
    fn drain_records_failures_without_counting_them_as_uploads() {
        let ctx = egui::Context::default();
        let mut cache = ThumbnailCache::default();
        let successes = (0..MAX_UPLOADS_PER_FRAME as u64).map(|key| (key, Some(image())));
        let failures = (100..110u64).map(|key| (key, None));
        queue(&mut cache, successes.chain(failures));

        cache.drain(&ctx);

        assert_eq!(cache.entries.len(), MAX_UPLOADS_PER_FRAME);
        assert_eq!(cache.failed.len(), 10);
        assert!(cache.pending.is_empty());
    }
}
