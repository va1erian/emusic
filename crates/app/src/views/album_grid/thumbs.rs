//! The app's [`ImageSink`] (#96): keeps decoded cover art as RGBA
//! buffers the Direct2D canvas uploads into its own bitmap cache.
//!
//! The shared [`ThumbCache`] decodes, resizes and caches artwork off the UI
//! thread and keeps a bounded LRU of uploaded handles; here the handle is a
//! [`RgbaImage`] already at the grid's current cover size. That size is baked
//! into the decode ([`cover_decoder`]) rather than the upload, so the UI thread
//! only clones the buffer. The cache is rebuilt when the cover size changes
//! (see [`ThumbState::new`]).
//!
//! Decoding and scaling go through the Windows Imaging Component (WIC) via
//! [`win32ui::imaging`], not the `image` crate, so the Win32 binary does not
//! link a bundled decoder (#119).

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use emusic_ui::image_cache::{
    DecodeFn, ImageSink, Rgba8Image, ThumbCache, embedded_artwork, folder_artwork_path,
    thumbnail_cache_path,
};
use emusic_ui::waker::WakerHandle;
use win32ui::RgbaImage;
use win32ui::imaging;

/// Longest edge of a decoded thumbnail, in pixels.
const THUMB_SIZE: u32 = 200;

/// Decoded bytes the thumbnail LRU may hold before evicting the least
/// recently used covers.
const BYTE_BUDGET: usize = 32 * 1024 * 1024;

/// The album grid's thumbnail cache over decoded RGBA buffers.
pub(super) type CoverCache = ThumbCache<RgbaSink>;

/// Keeps decoded images as RGBA buffers for the Direct2D canvas. The image is
/// already the cover size (see [`cover_decoder`]), so this only clones it.
pub(super) struct RgbaSink;

impl ImageSink for RgbaSink {
    /// `None` only if the buffer is unusable; the grid then draws its
    /// placeholder, exactly as it does while a decode is still in flight.
    type Handle = Option<Rc<RgbaImage>>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Option<Rc<RgbaImage>> {
        Some(Rc::new(to_win32(image)))
    }
}

/// The grid's shared thumbnail state: the cache and the sink its uploads go
/// through. Kept behind one `Rc<RefCell<..>>` because the `content` painter
/// (a `Fn`) requests covers while the grid paints.
pub(super) struct ThumbState {
    pub(super) cache: CoverCache,
    pub(super) sink: RgbaSink,
}

impl ThumbState {
    /// A cache whose worker decodes and resizes covers to `cover_px`, waking
    /// the UI through `waker` when a decode finishes.
    pub(super) fn new(cover_px: i32, waker: WakerHandle) -> Self {
        let mut cache = ThumbCache::new(BYTE_BUDGET, cover_decoder(cover_px.max(1) as u32));
        cache.set_waker(waker);
        Self {
            cache,
            sink: RgbaSink,
        }
    }

    /// Returns the cached cover for `source`, requesting a decode on a miss.
    /// A clone of the handle outlives the borrow so the painter can upload it.
    pub(super) fn cover(&mut self, source: &str) -> Option<Rc<RgbaImage>> {
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
/// UI thread, so [`RgbaSink::upload`] only clones the buffer.
fn cover_decoder(edge: u32) -> DecodeFn {
    let base = thumbnail_decoder(THUMB_SIZE);
    Arc::new(move |source: &Path, fallback_dir: Option<&Path>| {
        let image = base(source, fallback_dir)?;
        Some(resize(&image, edge, edge))
    })
}

/// A WIC-backed replacement for the shared `image`-based thumbnail decoder:
/// reads the on-disk cache, otherwise decodes the artwork and writes a resized
/// JPEG back, sharing one on-disk thumbnail cache.
fn thumbnail_decoder(max_edge: u32) -> DecodeFn {
    Arc::new(move |source, fallback_dir| {
        if let Some(image) = read_cache(source) {
            return Some(image);
        }
        let image = load_artwork(source, fallback_dir)?;
        let thumbnail = thumbnail(&image, max_edge);
        write_cache(source, &thumbnail);
        Some(thumbnail)
    })
}

/// Embedded picture first, then a `cover`/`folder`/`front` image next to the
/// file (or in `fallback_dir`).
fn load_artwork(path: &Path, fallback_dir: Option<&Path>) -> Option<Rgba8Image> {
    if let Some(bytes) = embedded_artwork(path)
        && let Some(image) = decode(&bytes)
    {
        return Some(image);
    }
    let file = folder_artwork_path(path, fallback_dir)?;
    decode(&std::fs::read(file).ok()?)
}

/// Decodes `bytes` into the shared image type, or `None` if WIC cannot.
fn decode(bytes: &[u8]) -> Option<Rgba8Image> {
    let image = imaging::decode(bytes).ok()?;
    Some(Rgba8Image {
        width: image.width,
        height: image.height,
        pixels: image.pixels,
    })
}

/// Scales `image` down so its longest edge is `max_edge`, preserving the aspect
/// ratio; an image already within the bound is returned unchanged.
fn thumbnail(image: &Rgba8Image, max_edge: u32) -> Rgba8Image {
    if image.width <= max_edge && image.height <= max_edge {
        return image.clone();
    }
    let scale = (max_edge as f64 / image.width as f64).min(max_edge as f64 / image.height as f64);
    let width = ((image.width as f64 * scale).round() as u32).max(1);
    let height = ((image.height as f64 * scale).round() as u32).max(1);
    resize(image, width, height)
}

/// Reads the on-disk thumbnail for `source`, if it exists and decodes.
fn read_cache(source: &Path) -> Option<Rgba8Image> {
    let bytes = std::fs::read(thumbnail_cache_path(source)?).ok()?;
    decode(&bytes)
}

/// Writes `image` to the shared thumbnail cache as a JPEG, best effort.
fn write_cache(source: &Path, image: &Rgba8Image) {
    let Some(path) = thumbnail_cache_path(source) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(bytes) = imaging::encode_jpeg(&to_win32(image)) {
        let _ = std::fs::write(path, bytes);
    }
}

/// Borrows the shared image type as the `win32ui` RGBA buffer WIC takes.
fn to_win32(image: &Rgba8Image) -> win32ui::RgbaImage {
    win32ui::RgbaImage {
        width: image.width,
        height: image.height,
        pixels: image.pixels.clone(),
    }
}

/// Resizes `image` to `width`-by-`height` via WIC. Aspect ratio is not
/// preserved: covers are drawn stretched into the square tile.
/// Falls back to the source image if WIC rejects the buffer.
fn resize(image: &Rgba8Image, width: u32, height: u32) -> Rgba8Image {
    if image.width == width && image.height == height {
        return image.clone();
    }
    match imaging::resize(&to_win32(image), width, height) {
        Ok(resized) => Rgba8Image {
            width: resized.width,
            height: resized.height,
            pixels: resized.pixels,
        },
        Err(error) => {
            tracing::warn!(%error, "thumbnail resize");
            image.clone()
        }
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

    #[test]
    fn thumbnail_preserves_the_aspect_ratio_and_shrinks() {
        let source = image(400, 200, [1, 2, 3, 255]);
        let thumb = thumbnail(&source, 200);
        assert_eq!((thumb.width, thumb.height), (200, 100));
    }

    #[test]
    fn thumbnail_keeps_small_images() {
        let source = image(50, 50, [1, 2, 3, 255]);
        let thumb = thumbnail(&source, 200);
        assert_eq!((thumb.width, thumb.height), (50, 50));
    }
}
