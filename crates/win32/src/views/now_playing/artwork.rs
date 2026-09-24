//! Win32 artwork cache for the now-playing panel (#110).
//!
//! The shared [`emusic_ui::image_cache`] engine decodes album art off the UI
//! thread and wakes the frontend through the [`Waker`](emusic_ui::waker::Waker).
//! This module supplies the frontend half: an [`ImageSink`] that turns a decoded
//! RGBA image into a DIB-section [`Bitmap`] (the `HBITMAP` from issue #110),
//! and the cache configured with the full-size artwork decoder (embedded
//! picture, then a folder image next to the playing file).

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use emusic_ui::image_cache::{ImageSink, Rgba8Image, ThumbCache, load_artwork};
use emusic_ui::waker::WakerHandle;
use image::RgbaImage;
use image::imageops::FilterType;
use win32ui::gdi::Bitmap;

/// Decoded bytes the artwork LRU may hold before evicting older covers.
const BYTE_BUDGET: usize = 16 * 1024 * 1024;

/// The now-playing panel's artwork cache: the shared cache over DIB bitmaps.
pub type ArtworkCache = ThumbCache<Win32ImageSink>;

/// Uploads decoded images as DIB-section bitmaps.
///
/// `Canvas::draw_bitmap` blits a bitmap at its own size (it crops rather than
/// scales), so the image is stretched to a square `edge` here, matching the egui
/// panel which draws the cover into a square rect. A failed GDI allocation
/// yields `None`, which the panel renders as its placeholder.
pub struct Win32ImageSink {
    edge: u32,
}

impl Win32ImageSink {
    /// Creates a sink uploading each image as an `edge`×`edge` square.
    #[must_use]
    pub fn new(edge: u32) -> Self {
        Self { edge: edge.max(1) }
    }
}

impl ImageSink for Win32ImageSink {
    type Handle = Option<Rc<Bitmap>>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Self::Handle {
        let square = to_square(image, self.edge);
        match Bitmap::from_rgba(
            square.width() as i32,
            square.height() as i32,
            square.as_raw(),
        ) {
            Ok(bitmap) => Some(Rc::new(bitmap)),
            Err(error) => {
                tracing::warn!(%error, "now-playing artwork bitmap");
                None
            }
        }
    }
}

/// Stretches `image` to an `edge`×`edge` square, or a blank square if the
/// source dimensions are unusable.
fn to_square(image: &Rgba8Image, edge: u32) -> RgbaImage {
    let Some(source) = RgbaImage::from_raw(image.width, image.height, image.pixels.clone()) else {
        return RgbaImage::new(edge, edge);
    };
    image::imageops::resize(&source, edge, edge, FilterType::Lanczos3)
}

/// Builds the artwork cache with the full-size decoder and `waker`.
#[must_use]
pub fn new_cache(waker: WakerHandle) -> ArtworkCache {
    let decode = |path: &Path, fallback_dir: Option<&Path>| {
        load_artwork(path).or_else(|| fallback_dir.and_then(load_artwork))
    };
    let mut cache = ThumbCache::new(BYTE_BUDGET, Arc::new(decode));
    cache.set_waker(waker);
    cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_square_stretches_to_the_requested_edge() {
        let image = Rgba8Image {
            width: 4,
            height: 2,
            pixels: vec![255; 4 * 2 * 4],
        };
        let square = to_square(&image, 8);
        assert_eq!(square.dimensions(), (8, 8));
    }

    #[test]
    fn to_square_tolerates_a_malformed_buffer() {
        let image = Rgba8Image {
            width: 4,
            height: 4,
            pixels: vec![0; 3],
        };
        let square = to_square(&image, 5);
        assert_eq!(square.dimensions(), (5, 5));
    }
}
