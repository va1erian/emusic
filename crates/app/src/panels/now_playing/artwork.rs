//! Artwork loading for the now-playing panel (#96).
//!
//! Uses the shared [`emusic_ui::image_cache`] engine: real files are loaded off
//! the UI thread via `lofty` (embedded picture) followed by folder-image
//! fallback, and the decoded RGBA data is uploaded as an egui texture on a
//! later frame. Paths that do not exist on disk (e.g. mock data) get a
//! deterministic generated placeholder so screenshots stay stable.

use std::path::Path;
use std::sync::Arc;

use eframe::egui;
use emusic_ui::image_cache::{Rgba8Image, ThumbCache, load_artwork};
use emusic_ui::views::now_playing::NowPlayingView;

use crate::image_sink::EguiImageSink;

/// Number of pixels in the generated placeholder square.
const PLACEHOLDER_SIZE: u32 = 256;

/// Decoded bytes the artwork LRU may hold before evicting older covers.
const BYTE_BUDGET: usize = 16 * 1024 * 1024;

/// The now-playing panel's artwork cache: the shared cache over egui textures.
pub type ArtworkCache = ThumbCache<EguiImageSink>;

/// Builds the artwork cache with the full-size decoder and the deterministic
/// placeholder for paths that are not on disk.
#[must_use]
pub fn new_cache() -> ArtworkCache {
    let decode =
        |path: &Path, fallback_dir: Option<&Path>| load_artwork_with_fallback(path, fallback_dir);
    ThumbCache::new(BYTE_BUDGET, Arc::new(decode)).with_placeholder(Arc::new(placeholder_for))
}

/// Full-size artwork for `path`, falling back to a folder image in
/// `fallback_dir` (the library track's directory) when `path` itself has
/// neither embedded art nor a sibling image.
fn load_artwork_with_fallback(path: &Path, fallback_dir: Option<&Path>) -> Option<Rgba8Image> {
    load_artwork(path).or_else(|| fallback_dir.and_then(load_artwork))
}

/// Show the artwork area: a square image (or placeholder) sized to the
/// panel width.
pub fn show(ui: &mut egui::Ui, cache: &mut ArtworkCache, view: &NowPlayingView) {
    let mut sink = EguiImageSink::new(ui.ctx().clone(), "artwork");
    cache.drain(&mut sink);

    let max_size = ui.available_width().min(320.0);
    let desired_size = egui::vec2(max_size, max_size);

    let request = view.artwork();
    let key = request.path.as_str();
    let texture = if key.is_empty() {
        None
    } else {
        // No texture yet: `get` requests it from a worker thread (or generates
        // a placeholder synchronously for non-existent mock paths). The
        // library track's directory is searched for a folder image when the
        // playing path has none.
        let fallback_dir = request.fallback_dir.as_deref().map(Path::new);
        cache
            .get_with_fallback(&mut sink, key, fallback_dir)
            .cloned()
    };

    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();

    // Background / placeholder.
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);

    if let Some(handle) = texture {
        painter.image(
            handle.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "♪",
            egui::FontId::proportional(48.0),
            ui.visuals().weak_text_color(),
        );
    }

    if !request.path.is_empty() {
        response.on_hover_text(&request.path);
    }
}

/// Generate a deterministic placeholder gradient for a path.
fn placeholder_for(key: &str) -> Rgba8Image {
    let mut hash = 0u64;
    for byte in key.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }

    let hue = (hash % 360) as f32 / 360.0;
    let sat = 0.5 + ((hash / 360) % 100) as f32 / 200.0;
    let light = 0.25 + ((hash / 36_000) % 100) as f32 / 200.0;
    let base = hsl_to_rgb(hue, sat, light);
    let accent = hsl_to_rgb((hue + 0.5) % 1.0, sat, light + 0.15);

    let size = PLACEHOLDER_SIZE;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let t = (x as f32 / size as f32 + y as f32 / size as f32) / 2.0;
            pixels.push(lerp(base[0], accent[0], t));
            pixels.push(lerp(base[1], accent[1], t));
            pixels.push(lerp(base[2], accent[2], t));
            pixels.push(255);
        }
    }

    Rgba8Image {
        width: size,
        height: size,
        pixels,
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 + (b as f32 - a as f32) * t) as u8
}

/// Convert HSL (components in 0..1) to sRGB bytes.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = match (h * 6.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    [
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    ]
}
