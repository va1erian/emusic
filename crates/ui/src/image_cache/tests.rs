//! Fake-sink tests for the shared cache's upload cap, LRU and byte budget.

use super::*;

/// Records every upload so tests can assert what reached the frontend.
#[derive(Default)]
struct FakeSink {
    uploads: Vec<(u64, Rgba8Image)>,
}

impl ImageSink for FakeSink {
    type Handle = usize;

    fn upload(&mut self, key: u64, image: &Rgba8Image) -> usize {
        self.uploads.push((key, image.clone()));
        self.uploads.len() - 1
    }
}

/// A 1x1 image whose byte length is exactly `bytes`.
fn image(bytes: usize) -> Rgba8Image {
    Rgba8Image {
        width: 1,
        height: 1,
        pixels: vec![0; bytes],
    }
}

fn cache(byte_budget: usize) -> ThumbCache<FakeSink> {
    ThumbCache::new(byte_budget, Arc::new(|_| None))
}

fn queue(
    cache: &mut ThumbCache<FakeSink>,
    keys: impl IntoIterator<Item = (u64, Option<Rgba8Image>)>,
) {
    for (key, image) in keys {
        cache.pending.push_back(Finished { key, image });
    }
}

#[test]
fn drain_caps_uploads_per_call_and_keeps_the_rest_queued() {
    let mut cache = cache(usize::MAX);
    let mut sink = FakeSink::default();
    let total = MAX_UPLOADS_PER_FRAME * 3;
    queue(
        &mut cache,
        (0..total as u64).map(|key| (key, Some(image(4)))),
    );

    cache.drain(&mut sink);
    assert_eq!(sink.uploads.len(), MAX_UPLOADS_PER_FRAME);
    assert_eq!(cache.pending.len(), total - MAX_UPLOADS_PER_FRAME);

    cache.drain(&mut sink);
    cache.drain(&mut sink);
    assert_eq!(sink.uploads.len(), total);
    assert!(cache.pending.is_empty());
}

#[test]
fn drain_records_failures_without_uploading_them() {
    let mut cache = cache(usize::MAX);
    let mut sink = FakeSink::default();
    let successes = (0..MAX_UPLOADS_PER_FRAME as u64).map(|key| (key, Some(image(4))));
    let failures = (100..110u64).map(|key| (key, None));
    queue(&mut cache, successes.chain(failures));

    cache.drain(&mut sink);

    assert_eq!(sink.uploads.len(), MAX_UPLOADS_PER_FRAME);
    assert_eq!(cache.failed.len(), 10);
    assert!(cache.pending.is_empty());
}

#[test]
fn lru_evicts_to_stay_within_the_byte_budget() {
    // Room for three 10-byte images; the fourth evicts the least recent.
    let mut cache = cache(30);
    let mut sink = FakeSink::default();
    queue(&mut cache, (0..4u64).map(|key| (key, Some(image(10)))));
    cache.drain(&mut sink);

    assert_eq!(sink.uploads.len(), 4);
    assert_eq!(cache.entries.len(), 3);
    assert_eq!(cache.bytes, 30);
    assert!(!cache.entries.contains_key(&0));
    assert!(cache.entries.contains_key(&3));
}

#[test]
fn a_single_oversized_image_is_kept() {
    let mut cache = cache(5);
    let mut sink = FakeSink::default();
    queue(&mut cache, [(0, Some(image(10)))]);

    cache.drain(&mut sink);

    assert_eq!(cache.entries.len(), 1);
}

#[test]
fn missing_sources_use_the_placeholder_without_spawning_a_worker() {
    let placeholder = Arc::new(|_: &str| image(8));
    let mut cache = cache(usize::MAX).with_placeholder(placeholder);
    let mut sink = FakeSink::default();

    let handle = cache.get(&mut sink, "definitely/not/on/disk.jpg");

    assert!(handle.is_some());
    assert_eq!(sink.uploads.len(), 1);
    assert!(cache.jobs.is_none());
}
