//! Shared, frontend-agnostic image cache (#96).
//!
//! Album thumbnails and now-playing artwork both decode images off the UI
//! thread, de-duplicate in-flight requests, keep a bounded LRU of uploaded
//! handles and wake the UI when a decode finishes. That machinery lives here,
//! generic over the frontend's [`ImageSink`] — which turns a decoded
//! [`Rgba8Image`] into an `egui::TextureHandle`, an `HBITMAP`, ... — while the
//! frontend supplies the decode policy (see [`thumbnail_decoder`] and
//! [`load_artwork`]) and the byte budget.

mod decode;

#[cfg(test)]
mod tests;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::waker::{Waker as _, WakerHandle};

pub use decode::{
    embedded_artwork, folder_artwork_path, load_artwork, thumbnail_cache_path, thumbnail_decoder,
};

/// A decoded 8-bit RGBA image, row-major with unmultiplied alpha.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgba8Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba8Image {
    /// Number of bytes the pixel buffer occupies; the cache's byte budget is
    /// measured in these.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}

/// Turns a decoded image into a frontend-owned handle.
pub trait ImageSink {
    /// The frontend's texture/bitmap type.
    type Handle;

    /// Uploads `image`, returning the handle that owns it.
    ///
    /// `key` identifies the image within the cache so the frontend can name
    /// its resource deterministically and avoid collisions between caches.
    fn upload(&mut self, key: u64, image: &Rgba8Image) -> Self::Handle;
}

/// Decodes the image for a source path; runs on a worker thread.
///
/// `fallback_dir` is where folder-level images (`cover.jpg`, ...) may live
/// when the source itself is not the file to read from; `None` to search next
/// to `path` only. `None` as a return value means the image is unavailable and
/// the frontend draws its own placeholder.
pub type DecodeFn = Arc<dyn Fn(&Path, Option<&Path>) -> Option<Rgba8Image> + Send + Sync>;

/// Builds a stand-in image for a source path that is not on disk (e.g. mock
/// data), so headless renders stay deterministic and spawn no workers.
pub type PlaceholderFn = Arc<dyn Fn(&str) -> Rgba8Image + Send + Sync>;

/// Maximum number of images uploaded per [`ThumbCache::drain`].
///
/// A fast scroll reveals a burst of tiles at once, so many decodes can finish
/// together. Uploading all of them in one frame stalls it, so the rest stay
/// queued and are uploaded over the following frames.
const MAX_UPLOADS_PER_FRAME: usize = 4;

/// Number of threads decoding images.
const WORKERS: usize = 2;

/// A decode request handed to the worker pool.
struct Job {
    key: u64,
    source: PathBuf,
    fallback_dir: Option<PathBuf>,
    waker: WakerHandle,
}

/// A finished decode, sent back to the UI thread.
struct Finished {
    key: u64,
    image: Option<Rgba8Image>,
}

/// One cached handle plus its LRU stamp and decoded size.
struct Entry<H> {
    handle: H,
    used: u64,
    bytes: usize,
}

/// A bounded LRU cache of decoded images, shared by every frontend (#96).
pub struct ThumbCache<S: ImageSink> {
    entries: HashMap<u64, Entry<S::Handle>>,
    loading: HashSet<u64>,
    failed: HashSet<u64>,
    jobs: Option<Sender<Job>>,
    finished: Option<Receiver<Finished>>,
    /// Decodes finished but not yet uploaded, kept across frames so the
    /// per-frame upload cap is respected without losing work.
    pending: VecDeque<Finished>,
    /// Monotonic LRU clock; a larger value means more recently used.
    clock: u64,
    /// Decoded bytes currently held and the cap they are evicted to.
    bytes: usize,
    byte_budget: usize,
    decode: DecodeFn,
    placeholder: Option<PlaceholderFn>,
    waker: WakerHandle,
}

impl<S: ImageSink> ThumbCache<S> {
    /// Creates a cache that evicts least-recently-used images once the
    /// decoded bytes it holds exceed `byte_budget`, decoding through `decode`.
    #[must_use]
    pub fn new(byte_budget: usize, decode: DecodeFn) -> Self {
        Self {
            entries: HashMap::new(),
            loading: HashSet::new(),
            failed: HashSet::new(),
            jobs: None,
            finished: None,
            pending: VecDeque::new(),
            clock: 0,
            bytes: 0,
            byte_budget,
            decode,
            placeholder: None,
            waker: WakerHandle::default(),
        }
    }

    /// Sets the stand-in image for sources that are not on disk.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: PlaceholderFn) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    /// Sets the waker worker threads use to ask the UI to repaint.
    pub fn set_waker(&mut self, waker: WakerHandle) {
        self.waker = waker;
    }

    /// Uploads thumbnails the workers finished and trims the LRU. Call once
    /// per frame before [`Self::get`].
    ///
    /// At most [`MAX_UPLOADS_PER_FRAME`] images are uploaded per call; the
    /// rest stay queued and the UI is woken while work remains, so a burst of
    /// decodes is spread over several frames instead of stalling one.
    pub fn drain(&mut self, sink: &mut S) {
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
                    let handle = sink.upload(result.key, &image);
                    let bytes = image.byte_len();
                    self.insert(result.key, handle, bytes);
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
            self.waker.wake();
        }
    }

    /// Returns the handle for `source` if ready, requesting it otherwise.
    ///
    /// `source` is the file an image is read from. Paths that do not exist
    /// (e.g. mock data) never spawn a worker — they use the placeholder, if
    /// one was set, and otherwise report "not ready".
    ///
    /// `fallback_dir` lets the decoder find folder-level artwork when `source`
    /// is not itself the file to read (e.g. the now-playing panel passes the
    /// library track's directory); it is ignored by the placeholder path.
    pub fn get_with_fallback(
        &mut self,
        sink: &mut S,
        source: &str,
        fallback_dir: Option<&Path>,
    ) -> Option<&S::Handle> {
        let key = hash(source);
        if self.entries.contains_key(&key) {
            self.clock += 1;
            let used = self.clock;
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.used = used;
            }
            return self.entries.get(&key).map(|entry| &entry.handle);
        }
        if self.loading.contains(&key) || self.failed.contains(&key) {
            return None;
        }

        let path = Path::new(source);
        if !path.exists() {
            let placeholder = self.placeholder.clone()?;
            let image = placeholder(source);
            let handle = sink.upload(key, &image);
            let bytes = image.byte_len();
            self.insert(key, handle, bytes);
            return self.entries.get(&key).map(|entry| &entry.handle);
        }

        self.request(key, path, fallback_dir);
        None
    }

    /// Convenience for the common case with no extra folder-image search
    /// directory.
    pub fn get(&mut self, sink: &mut S, source: &str) -> Option<&S::Handle> {
        self.get_with_fallback(sink, source, None)
    }

    fn insert(&mut self, key: u64, handle: S::Handle, bytes: usize) {
        self.clock += 1;
        self.bytes += bytes;
        self.entries.insert(
            key,
            Entry {
                handle,
                used: self.clock,
                bytes,
            },
        );
    }

    /// Drops least-recently-used entries until the decoded bytes fit the
    /// budget, always keeping at least the most recent one.
    fn evict(&mut self) {
        while self.bytes > self.byte_budget && self.entries.len() > 1 {
            let Some(&key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&key) {
                self.bytes -= entry.bytes;
            }
        }
    }

    fn request(&mut self, key: u64, source: &Path, fallback_dir: Option<&Path>) {
        if !self.loading.insert(key) {
            return;
        }
        let sender = self.ensure_pool();
        let job = Job {
            key,
            source: source.to_path_buf(),
            fallback_dir: fallback_dir.map(Path::to_path_buf),
            waker: self.waker.clone(),
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
            let decode = Arc::clone(&self.decode);
            std::thread::spawn(move || worker(job_rx, done_tx, decode));
        }
        self.jobs = Some(job_tx.clone());
        self.finished = Some(done_rx);
        job_tx
    }
}

/// Pulls jobs off the shared queue until the sender is dropped.
fn worker(jobs: Arc<Mutex<Receiver<Job>>>, done: Sender<Finished>, decode: DecodeFn) {
    loop {
        let job = {
            let Ok(jobs) = jobs.lock() else { return };
            let Ok(job) = jobs.recv() else { return };
            job
        };
        let image = decode(&job.source, job.fallback_dir.as_deref());
        if done
            .send(Finished {
                key: job.key,
                image,
            })
            .is_err()
        {
            return;
        }
        job.waker.wake();
    }
}

/// 64-bit hash of a source path, used as the cache key and the on-disk
/// thumbnail file name. `DefaultHasher` is not guaranteed stable across Rust
/// releases, which only means the disk cache is rebuilt after a toolchain
/// change.
pub(super) fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
