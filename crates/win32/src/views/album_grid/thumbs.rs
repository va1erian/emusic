//! The Win32 frontend's [`ImageSink`] (#96): uploads decoded images as
//! DIB-section bitmaps the GDI [`Canvas`](win32ui::gdi::Canvas) can blit.
//!
//! The shared [`ThumbCache`] decodes, resizes and caches artwork off the UI
//! thread and keeps a bounded LRU of uploaded handles; here the handle is a
//! top-down 32-bpp [`Bitmap`] already at the grid's current cover size. That
//! size is baked into the decode ([`cover_decoder`]) rather than the upload, so
//! the UI thread only allocates the DIB. The cache is rebuilt when the cover
//! size changes (see [`ThumbState::new`]).

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use emusic_ui::image_cache::{DecodeFn, ImageSink, Rgba8Image, ThumbCache, thumbnail_decoder};
use emusic_ui::waker::WakerHandle;
use win32ui::gdi::Bitmap;

/// Longest edge of a decoded thumbnail, in pixels (matches the egui frontend).
const THUMB_SIZE: u32 = 200;

/// Decoded bytes the thumbnail LRU may hold before evicting the least
/// recently used covers.
const BYTE_BUDGET: usize = 32 * 1024 * 1024;

/// The album grid's thumbnail cache over Win32 bitmaps.
pub(super) type BitmapCache = ThumbCache<BitmapSink>;

/// Uploads decoded images as top-down 32-bpp bitmaps. The image is already the
/// cover size (see [`cover_decoder`]), so this only allocates the DIB.
pub(super) struct BitmapSink;

impl ImageSink for BitmapSink {
    /// `None` when GDI could not allocate the DIB; the grid then draws its
    /// placeholder, exactly as it does while a decode is still in flight.
    type Handle = Option<Rc<Bitmap>>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Option<Rc<Bitmap>> {
        Bitmap::from_rgba(image.width as i32, image.height as i32, &image.pixels)
            .ok()
            .map(Rc::new)
    }
}

/// The grid's shared thumbnail state: the cache and the sink its uploads go
/// through. Kept behind one `Rc<RefCell<..>>` because the `content` painter
/// (a `Fn`) requests covers while the grid paints.
pub(super) struct ThumbState {
    pub(super) cache: BitmapCache,
    pub(super) sink: BitmapSink,
}

impl ThumbState {
    /// A cache whose worker decodes and resizes covers to `cover_px`, waking
    /// the UI through `waker` when a decode finishes.
    pub(super) fn new(cover_px: i32, waker: WakerHandle) -> Self {
        let mut cache = ThumbCache::new(BYTE_BUDGET, cover_decoder(cover_px.max(1) as u32));
        cache.set_waker(waker);
        Self {
            cache,
            sink: BitmapSink,
        }
    }

    /// Returns the cached cover for `source`, requesting a decode on a miss.
    /// A clone of the handle outlives the borrow so the painter can blit it.
    pub(super) fn cover(&mut self, source: &str) -> Option<Rc<Bitmap>> {
        let Self { cache, sink } = self;
        cache.get(sink, source).and_then(|handle| handle.clone())
    }

    /// Uploads the decodes the workers finished, then trims the LRU. Called
    /// once per frame before the grid paints. The shared cache caps uploads at
    /// four per call and wakes the UI while more are pending, so a burst of
    /// finished covers is spread over several frames instead of stalling one.
    pub(super) fn drain(&mut self) {
        let Self { cache, sink } = self;
        cache.drain(sink);
    }
}

/// The worker-side decoder: the shared disk-cached 200 px thumbnail decoder,
/// resized to the grid's `edge`-by-`edge` cover. Resizing here keeps it off the
/// UI thread, so [`BitmapSink::upload`] only creates the DIB.
fn cover_decoder(edge: u32) -> DecodeFn {
    let base = thumbnail_decoder(THUMB_SIZE);
    Arc::new(move |source: &Path, fallback_dir: Option<&Path>| {
        let image = base(source, fallback_dir)?;
        Some(resize(&image, edge, edge))
    })
}

/// Resizes `image` to `width`-by-`height` with the `image` crate's bilinear
/// filter. Aspect ratio is not preserved: like egui, covers are drawn stretched
/// into the square tile.
fn resize(image: &Rgba8Image, width: u32, height: u32) -> Rgba8Image {
    if image.width == width && image.height == height {
        return image.clone();
    }
    let Some(source) = image::RgbaImage::from_raw(image.width, image.height, image.pixels.clone())
    else {
        return image.clone();
    };
    let resized = image::imageops::resize(
        &source,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    Rgba8Image {
        width,
        height,
        pixels: resized.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, pixel: [u8; 4]) -> Rgba8Image {
        Rgba8Image {
            width,
            height,
            pixels: pixel
                .iter()
                .copied()
                .cycle()
                .take((width * height * 4) as usize)
                .collect(),
        }
    }

    #[test]
    fn same_size_is_a_plain_copy() {
        let source = image(2, 2, [10, 20, 30, 40]);
        let scaled = resize(&source, 2, 2);
        assert_eq!(scaled.pixels, source.pixels);
    }

    #[test]
    fn resize_fills_the_target_dimensions() {
        let source = image(4, 4, [255, 0, 0, 255]);
        let scaled = resize(&source, 8, 6);
        assert_eq!((scaled.width, scaled.height), (8, 6));
        assert_eq!(scaled.pixels.len(), 8 * 6 * 4);
        // A uniform source stays that colour after resampling.
        assert!(
            scaled
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|px| *px == [255, 0, 0, 255])
        );
    }
}
