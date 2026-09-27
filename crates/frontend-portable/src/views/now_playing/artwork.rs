//! The now-playing artwork cache: the shared [`emusic_ui::image_cache`] engine
//! over portable [`Image`]s.
//!
//! The shared cache decodes album art off the UI thread and wakes the UI
//! through the [`Waker`](emusic_ui::waker::Waker). This module supplies the
//! frontend half — an [`ImageSink`] that builds an [`Image`] from the decoded
//! RGBA bytes — and configures the cache with the artwork decoder (embedded
//! picture, then a folder image next to the playing file).

use emusic_ui::image_cache::{ImageSink, Rgba8Image, ThumbCache, thumbnail_decoder};
use emusic_ui::waker::WakerHandle;
use xui::xui_core::Image;

/// Decoded bytes the artwork LRU may hold before evicting older covers.
const BYTE_BUDGET: usize = 16 * 1024 * 1024;
/// Longest edge the artwork is decoded at; the summary box is much smaller.
const MAX_EDGE: u32 = 512;

/// The now-playing artwork cache over portable images.
pub type ArtworkCache = ThumbCache<ImageSinkImpl>;

/// Turns a decoded RGBA buffer into a portable [`Image`].
pub struct ImageSinkImpl;

impl ImageSink for ImageSinkImpl {
    type Handle = Option<Image>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Self::Handle {
        Image::from_rgba(image.width, image.height, image.pixels.clone()).ok()
    }
}

/// Builds the artwork cache with the thumbnail decoder and `waker`.
#[must_use]
pub fn new_cache(waker: WakerHandle) -> ArtworkCache {
    let mut cache = ThumbCache::new(BYTE_BUDGET, thumbnail_decoder(MAX_EDGE));
    cache.set_waker(waker);
    cache
}
