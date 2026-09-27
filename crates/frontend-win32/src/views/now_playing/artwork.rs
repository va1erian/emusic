//! Win32 artwork cache for the now-playing panel (#110).
//!
//! The shared [`emusic_ui::image_cache`] engine decodes album art off the UI
//! thread and wakes the frontend through the [`Waker`](emusic_ui::waker::Waker).
//! This module supplies the frontend half: an [`ImageSink`] that keeps a
//! decoded RGBA image ready for the Direct2D canvas, and the cache configured
//! with the full-size artwork decoder (embedded picture, then a folder image
//! next to the playing file).
//!
//! Direct2D bitmaps live on the drawing surface, which the summary only has
//! while painting, so the sink keeps the decoded pixels and the widget uploads
//! them lazily on the first frame that draws them (see
//! [`summary`](super::summary)). Keeping the full-size image lets Direct2D
//! scale it into the artwork box, so the fixed-square CPU resize the GDI path
//! needed is gone.
//!
//! Artwork is decoded by the Windows Imaging Component (WIC) through
//! [`win32ui::imaging`], so the Win32 binary does not link the `image` crate
//! (#119).

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use emusic_ui::image_cache::{
    ImageSink, Rgba8Image, ThumbCache, embedded_artwork, folder_artwork_path,
};
use emusic_ui::waker::WakerHandle;
use win32ui::RgbaImage;
use win32ui::imaging;

/// Decoded bytes the artwork LRU may hold before evicting older covers.
const BYTE_BUDGET: usize = 16 * 1024 * 1024;

/// The now-playing panel's artwork cache: the shared cache over RGBA buffers.
pub type ArtworkCache = ThumbCache<Win32ImageSink>;

/// Keeps decoded images as RGBA buffers for [`D2dCanvas::image`].
///
/// [`D2dCanvas::image`]: win32ui::d2d::D2dCanvas::image
pub struct Win32ImageSink;

impl ImageSink for Win32ImageSink {
    type Handle = Option<Rc<RgbaImage>>;

    fn upload(&mut self, _key: u64, image: &Rgba8Image) -> Self::Handle {
        Some(Rc::new(RgbaImage {
            width: image.width,
            height: image.height,
            pixels: image.pixels.clone(),
        }))
    }
}

/// Builds the artwork cache with the WIC artwork decoder and `waker`.
#[must_use]
pub fn new_cache(waker: WakerHandle) -> ArtworkCache {
    let decode = |path: &Path, fallback_dir: Option<&Path>| load_artwork(path, fallback_dir);
    let mut cache = ThumbCache::new(BYTE_BUDGET, Arc::new(decode));
    cache.set_waker(waker);
    cache
}

/// Embedded picture first, then a `cover`/`folder`/`front` image next to the
/// file (or in `fallback_dir`).
fn load_artwork(path: &Path, fallback_dir: Option<&Path>) -> Option<Rgba8Image> {
    if let Some(bytes) = embedded_artwork(path)
        && let Ok(image) = imaging::decode(&bytes)
    {
        return Some(Rgba8Image {
            width: image.width,
            height: image.height,
            pixels: image.pixels,
        });
    }
    let file = folder_artwork_path(path, fallback_dir)?;
    let bytes = std::fs::read(file).ok()?;
    let image = imaging::decode(&bytes).ok()?;
    Some(Rgba8Image {
        width: image.width,
        height: image.height,
        pixels: image.pixels,
    })
}
