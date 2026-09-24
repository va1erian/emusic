//! Album-thumbnail cache (#17, #96): the shared [`emusic_ui::image_cache`]
//! engine configured with the thumbnail decoder (disk cache + resize).
//!
//! Thumbnails are decoded from embedded/folder artwork on a small worker pool,
//! never on the UI thread, and the raw result is cached on disk under
//! `%LOCALAPPDATA%\emusic\thumbs\<hash>.jpg` (about [`THUMB_SIZE`] px on the
//! long edge). The uploaded GPU textures are kept in a bounded LRU by the
//! shared cache.

use emusic_ui::image_cache::{ThumbCache, thumbnail_decoder};

use crate::image_sink::EguiImageSink;

/// Longest edge of a cached thumbnail, in pixels.
const THUMB_SIZE: u32 = 200;

/// Decoded bytes the thumbnail LRU may hold before evicting the least
/// recently used covers.
const BYTE_BUDGET: usize = 32 * 1024 * 1024;

/// The album grid's thumbnail cache: the shared cache over egui textures.
pub type ThumbnailCache = ThumbCache<EguiImageSink>;

/// Builds the album grid's cache, decoding (and disk-caching) thumbnails to
/// [`THUMB_SIZE`].
#[must_use]
pub fn new_cache() -> ThumbnailCache {
    ThumbCache::new(BYTE_BUDGET, thumbnail_decoder(THUMB_SIZE))
}
