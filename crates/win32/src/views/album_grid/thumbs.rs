//! The Win32 frontend's [`ImageSink`] (#96): uploads decoded images as
//! DIB-section bitmaps the GDI [`Canvas`](win32ui::gdi::Canvas) can blit.
//!
//! The shared [`ThumbCache`] decodes artwork off the UI thread and keeps a
//! bounded LRU of uploaded handles; here the handle is a scaled, top-down
//! 32-bpp [`Bitmap`]. `draw_bitmap` blits 1:1, so [`BitmapSink`] resizes every
//! decoded image to the cover edge the grid is currently using; the cache is
//! rebuilt when that edge changes (see [`ThumbState::new`]).

use std::rc::Rc;

use emusic_ui::image_cache::{ImageSink, Rgba8Image, ThumbCache, thumbnail_decoder};
use emusic_ui::waker::WakerHandle;
use win32ui::gdi::Bitmap;

/// Longest edge of a decoded thumbnail, in pixels (matches the egui frontend).
const THUMB_SIZE: u32 = 200;

/// Decoded bytes the thumbnail LRU may hold before evicting the least
/// recently used covers.
const BYTE_BUDGET: usize = 32 * 1024 * 1024;

/// The album grid's thumbnail cache over Win32 bitmaps.
pub(super) type BitmapCache = ThumbCache<BitmapSink>;

/// Uploads decoded images as top-down 32-bpp bitmaps, resized to the cover.
pub(super) struct BitmapSink {
    /// The cover edge, in device pixels, every uploaded image is resized to.
    edge: i32,
}

impl BitmapSink {
    /// A sink that resizes every image to an `edge`-by-`edge` cover.
    pub(super) fn new(edge: i32) -> Self {
        Self { edge: edge.max(1) }
    }
}

impl ImageSink for BitmapSink {
    /// `None` when GDI could not allocate the DIB; the grid then draws its
    /// placeholder, exactly as it does while a decode is still in flight.
    type Handle = Option<Rc<Bitmap>>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Option<Rc<Bitmap>> {
        let scaled = resize(image, self.edge as u32, self.edge as u32);
        Bitmap::from_rgba(scaled.width as i32, scaled.height as i32, &scaled.pixels)
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
    /// A cache that resizes covers to `cover_px` and wakes the UI through
    /// `waker` when a decode finishes.
    pub(super) fn new(cover_px: i32, waker: WakerHandle) -> Self {
        let mut cache = ThumbCache::new(BYTE_BUDGET, thumbnail_decoder(THUMB_SIZE));
        cache.set_waker(waker);
        Self {
            cache,
            sink: BitmapSink::new(cover_px),
        }
    }

    /// Returns the cached cover for `source`, requesting a decode on a miss.
    /// A clone of the handle outlives the borrow so the painter can blit it.
    pub(super) fn cover(&mut self, source: &str) -> Option<Rc<Bitmap>> {
        let Self { cache, sink } = self;
        cache.get(sink, source).and_then(|handle| handle.clone())
    }

    /// Uploads the decodes the workers finished, then trims the LRU. Called
    /// once per frame before the grid paints.
    pub(super) fn drain(&mut self) {
        let Self { cache, sink } = self;
        cache.drain(sink);
    }
}

/// Bilinearly resizes `image` to `width`-by-`height`, preserving neither the
/// aspect ratio nor the alpha-premultiplication (the source is unmultiplied
/// and `AlphaBlend` expects the same). The source is a small decoded
/// thumbnail, so the naive filter is fast enough on the UI thread.
fn resize(image: &Rgba8Image, width: u32, height: u32) -> Rgba8Image {
    if image.width == width && image.height == height {
        return image.clone();
    }
    let (sw, sh) = (image.width.max(1) as i32, image.height.max(1) as i32);
    let src = image.pixels.as_slice();
    let mut pixels = vec![0u8; width as usize * height as usize * 4];

    for y in 0..height as i32 {
        let sy = ((y as f32 + 0.5) * sh as f32 / height as f32 - 0.5).max(0.0);
        let y0 = sy.floor() as i32;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = sy - y0 as f32;
        for x in 0..width as i32 {
            let sx = ((x as f32 + 0.5) * sw as f32 / width as f32 - 0.5).max(0.0);
            let x0 = sx.floor() as i32;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = sx - x0 as f32;

            let out = ((y * width as i32 + x) * 4) as usize;
            for channel in 0..4 {
                let top = lerp(
                    channel_at(src, sw, x0, y0, channel),
                    channel_at(src, sw, x1, y0, channel),
                    fx,
                );
                let bottom = lerp(
                    channel_at(src, sw, x0, y1, channel),
                    channel_at(src, sw, x1, y1, channel),
                    fx,
                );
                pixels[out + channel] = lerp(top, bottom, fy).round() as u8;
            }
        }
    }

    Rgba8Image {
        width,
        height,
        pixels,
    }
}

/// The `channel` byte of the pixel at `(x, y)` in a row-major RGBA buffer.
fn channel_at(src: &[u8], stride: i32, x: i32, y: i32, channel: usize) -> f32 {
    let index = ((y * stride + x) * 4) as usize + channel;
    f32::from(src.get(index).copied().unwrap_or(0))
}

/// Linear interpolation, `t` clamped to `0.0..=1.0`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
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
