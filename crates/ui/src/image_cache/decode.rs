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
    load_artwork_dynamic(path, None).map(Rgba8Image::from_dynamic)
}

/// A decoder that reads a resized thumbnail from disk, or decodes the source
/// artwork and writes the result back. `max_edge` caps the longest side.
///
/// The on-disk cache is keyed by the source alone, so `fallback_dir` only
/// affects a fresh decode; a folder-image-only cover is therefore cached under
/// the track's path (the grid always passes the same representative track).
#[must_use]
pub fn thumbnail_decoder(max_edge: u32) -> DecodeFn {
    Arc::new(move |source, fallback_dir| {
        if let Some(image) = read_cache(source) {
            return Some(image);
        }
        let image = load_artwork_dynamic(source, fallback_dir)?;
        let thumbnail = image.thumbnail(max_edge, max_edge);
        write_cache(source, &thumbnail);
        Some(Rgba8Image::from_dynamic(thumbnail))
    })
}

/// Embedded picture first, then `cover`/`folder`/`front` images next to the
/// file (or in `fallback_dir`).
fn load_artwork_dynamic(path: &Path, fallback_dir: Option<&Path>) -> Option<DynamicImage> {
    if let Ok(tagged) = lofty::read_from_path(path)
        && let Some(picture) = tagged.primary_tag().and_then(|tag| tag.pictures().first())
        && let Ok(img) = image::load_from_memory(picture.data())
    {
        return Some(img);
    }

    folder_image(path.parent()).or_else(|| folder_image(fallback_dir))
}

/// A `cover`/`folder`/`front` image in `dir`, if one exists.
fn folder_image(dir: Option<&Path>) -> Option<DynamicImage> {
    let dir = dir?;
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
    None
}

/// The bytes of the embedded cover picture in `path`'s primary tag, if any.
///
/// Frontends that decode with their own backend (e.g. WIC on Windows) use this
/// to read the artwork without going through the `image` crate.
#[must_use]
pub fn embedded_artwork(path: &Path) -> Option<Vec<u8>> {
    let tagged = lofty::read_from_path(path).ok()?;
    let picture = tagged.primary_tag()?.pictures().first()?;
    Some(picture.data().to_vec())
}

/// A `cover`/`folder`/`front` image file next to `path`, or in `fallback_dir`.
///
/// The non-decoding counterpart of [`load_artwork_dynamic`]'s folder search:
/// it returns the path so a frontend can decode the file itself.
#[must_use]
pub fn folder_artwork_path(path: &Path, fallback_dir: Option<&Path>) -> Option<PathBuf> {
    folder_image_path(path.parent()).or_else(|| folder_image_path(fallback_dir))
}

/// A `cover`/`folder`/`front` image in `dir`, if one exists.
fn folder_image_path(dir: Option<&Path>) -> Option<PathBuf> {
    let dir = dir?;
    for name in ["cover", "folder", "front"] {
        for ext in ["jpg", "jpeg", "png"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
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
///
/// Public so every frontend names the on-disk thumbnail for a source path the
/// same way and shares one cache.
#[must_use]
pub fn thumbnail_cache_path(source: &Path) -> Option<PathBuf> {
    let dir = dirs::data_local_dir()?.join("emusic").join("thumbs");
    Some(dir.join(format!("{:016x}.jpg", hash(&source.to_string_lossy()))))
}

fn read_cache(source: &Path) -> Option<Rgba8Image> {
    image::open(thumbnail_cache_path(source)?)
        .ok()
        .map(Rgba8Image::from_dynamic)
}

fn write_cache(source: &Path, image: &DynamicImage) {
    let Some(path) = thumbnail_cache_path(source) else {
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
