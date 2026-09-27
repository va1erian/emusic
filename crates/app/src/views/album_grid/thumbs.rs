//! The album grid's cover cache (#96), ported to the portable layer.
//!
//! The shared [`ThumbCache`] decodes artwork off the UI thread and keeps a
//! bounded LRU of uploaded handles; here a handle is a portable
//! [`Image`](xui::xui_core::Image), which every backend caches by identity, so
//! drawing the same cover on each repaint re-uploads nothing.

use std::rc::Rc;

use emusic_ui::image_cache::{ImageSink, Rgba8Image, ThumbCache, thumbnail_decoder};
use emusic_ui::waker::WakerHandle;
use xui::xui_core::Image;

/// Longest edge of a decoded cover, in pixels.
const THUMB_SIZE: u32 = 256;
/// Decoded bytes the LRU may hold before evicting the least recently used.
const BYTE_BUDGET: usize = 32 * 1024 * 1024;

/// The album grid's thumbnail cache over portable images.
pub(super) type CoverCache = ThumbCache<CoverSink>;

/// Turns a decoded RGBA buffer into a portable [`Image`], counting uploads so
/// the grid can repaint exactly when a cover becomes available.
pub(super) struct CoverSink {
    uploads: u64,
}

impl ImageSink for CoverSink {
    type Handle = Rc<Image>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Rc<Image> {
        self.uploads += 1;
        Rc::new(to_image(image))
    }
}

/// The grid's shared thumbnail state: the cache and the sink its uploads go
/// through. Shared behind an `Rc<RefCell<..>>` because the tile painter (a
/// `Fn`) requests covers while the grid paints.
pub(super) struct ThumbState {
    cache: CoverCache,
    sink: CoverSink,
}

impl ThumbState {
    /// A cache whose workers decode covers and wake the UI through `waker`.
    pub(super) fn new(waker: WakerHandle) -> Self {
        let mut cache = ThumbCache::new(BYTE_BUDGET, thumbnail_decoder(THUMB_SIZE));
        cache.set_waker(waker);
        Self {
            cache,
            sink: CoverSink { uploads: 0 },
        }
    }

    /// The cached cover for `source`, requesting a decode on a miss. The
    /// handle is cloned out so the painter can upload it after the borrow.
    pub(super) fn cover(&mut self, source: &str) -> Option<Rc<Image>> {
        let Self { cache, sink } = self;
        cache.get(sink, source).map(Rc::clone)
    }

    /// Uploads the decodes the workers finished. Returns whether any cover
    /// became available, so the grid repaints exactly when it must.
    pub(super) fn drain(&mut self) -> bool {
        let before = self.sink.uploads;
        let Self { cache, sink } = self;
        cache.drain(sink);
        self.sink.uploads > before
    }
}

/// Wraps a decoded buffer as a portable image. The dimensions come straight
/// from the decoder, so they always match the pixel buffer.
fn to_image(image: &Rgba8Image) -> Image {
    Image::from_rgba(image.width, image.height, image.pixels.clone())
        .expect("a decoded image's pixels match its dimensions")
}
