//! Image decoding shared by every frontend (#96): embedded/folder artwork via
//! `lofty`/`image`, plus the resized, on-disk thumbnail cache.
//!
//! The decode work runs on the cache's worker threads, never on the UI thread
//! (see [`super::ThumbCache`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::DynamicImage;
use lofty::file::TaggedFileExt as _;

use super::{DecodeFn, Rgba8Image, hash};

/// Tries embedded artwork via `lofty`, then a sibling folder image.
///
/// Returns the raw RGBA image so callers can resize it before uploading.
#[must_use]
pub fn load_artwork(path: &Path) -> Option<Rgba8Image> {
    load_artwork_dynamic(path).map(Rgba8Image::from_dynamic)
}

/// A decoder that reads a resized thumbnail from disk, or decodes the source
/// artwork and writes the result back. `max_edge` caps the longest side.
#[must_use]
pub fn thumbnail_decoder(max_edge: u32) -> DecodeFn {
    Arc::new(move |source| {
        if let Some(image) = read_cache(source) {
            return Some(image);
        }
        let image = load_artwork_dynamic(source)?;
        let thumbnail = image.thumbnail(max_edge, max_edge);
        write_cache(source, &thumbnail);
        Some(Rgba8Image::from_dynamic(thumbnail))
    })
}

/// Embedded picture first, then `cover`/`folder`/`front` images next to the
/// file.
fn load_artwork_dynamic(path: &Path) -> Option<DynamicImage> {
    if let Ok(tagged) = lofty::read_from_path(path)
        && let Some(picture) = tagged.primary_tag().and_then(|tag| tag.pictures().first())
        && let Ok(img) = image::load_from_memory(picture.data())
    {
        return Some(img);
    }

    if let Some(dir) = path.parent() {
        for name in ["cover", "folder", "front"] {
            for ext in ["jpg", "jpeg", "png"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.exists()
                    && let Ok(img) = image::open(&candidate)
                {
                    return Some(img);
                }
            }
        }
    }

    None
}

impl Rgba8Image {
    pub(super) fn from_dynamic(image: DynamicImage) -> Self {
        let rgba = image.to_rgba8();
        Self {
            width: rgba.width(),
            height: rgba.height(),
            pixels: rgba.into_raw(),
        }
    }
}

/// `%LOCALAPPDATA%\emusic\thumbs\<hash>.jpg`, or `None` if there is no local
/// data directory (thumbnails are then decoded on every request).
fn cache_path(source: &Path) -> Option<PathBuf> {
    let dir = dirs::data_local_dir()?.join("emusic").join("thumbs");
    Some(dir.join(format!("{:016x}.jpg", hash(&source.to_string_lossy()))))
}

fn read_cache(source: &Path) -> Option<Rgba8Image> {
    image::open(cache_path(source)?)
        .ok()
        .map(Rgba8Image::from_dynamic)
}

fn write_cache(source: &Path, image: &DynamicImage) {
    let Some(path) = cache_path(source) else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = image
        .to_rgb8()
        .save_with_format(&path, image::ImageFormat::Jpeg);
}
