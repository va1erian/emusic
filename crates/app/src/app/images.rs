//! The egui frontend's image caches (#96).
//!
//! The shared decode/LRU engine is toolkit-agnostic (`emusic_ui::image_cache`),
//! but the uploaded handles are egui textures, so the caches live here and are
//! passed into the views that draw with them. The Win32 frontend will own its
//! own caches over the same engine.

use crate::panels::now_playing::artwork::{self, ArtworkCache};
use crate::views::album_grid::thumbs::{self, ThumbnailCache};
use emusic_ui::waker::WakerHandle;

/// The egui frontend's album-thumbnail and now-playing-artwork caches.
pub struct ImageCaches {
    pub album_thumbs: ThumbnailCache,
    pub artwork: ArtworkCache,
}

impl ImageCaches {
    /// Builds both caches and points their workers at `waker`, so a finished
    /// decode wakes egui when no frame is otherwise due.
    #[must_use]
    pub fn new(waker: WakerHandle) -> Self {
        let mut album_thumbs = thumbs::new_cache();
        album_thumbs.set_waker(waker.clone());
        let mut artwork = artwork::new_cache();
        artwork.set_waker(waker);
        Self {
            album_thumbs,
            artwork,
        }
    }
}
