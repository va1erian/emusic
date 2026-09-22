//! Artwork loading for the now-playing panel.
//!
//! Real files are loaded off the UI thread via `lofty` (embedded picture)
//! followed by folder-image fallback (`cover`/`folder`/`front` `.jpg`/
//! `.png`, case-insensitive). The decoded RGBA data is sent back through a
//! channel and uploaded as an egui texture on the next frame. Paths that do
//! not exist on disk (e.g. mock data) get a deterministic generated
//! placeholder so screenshots stay stable.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use lofty::file::TaggedFileExt;

use crate::library_api::TrackInfo;
use crate::player_api::NowPlayingInfo;

/// Number of pixels in the generated placeholder square.
const PLACEHOLDER_SIZE: u32 = 256;

/// Completed artwork load, sent from the worker thread back to the UI.
struct LoadedArtwork {
    key: String,
    image: Option<ColorImage>,
}

/// Cache for the current track's artwork texture.
///
/// Holds already-uploaded textures and tracks in-flight loads so each path
/// is only decoded once.
#[derive(Default)]
pub struct ArtworkCache {
    textures: Vec<(String, TextureHandle)>,
    loading: HashSet<String>,
    rx: Option<Receiver<LoadedArtwork>>,
    tx: Option<Sender<LoadedArtwork>>,
}

impl ArtworkCache {
    /// Ensure the cache has a channel. Created lazily so the default
    /// constructor stays free.
    fn ensure_channel(&mut self) -> Sender<LoadedArtwork> {
        if self.tx.is_none() {
            let (tx, rx) = mpsc::channel();
            self.tx = Some(tx.clone());
            self.rx = Some(rx);
        }
        self.tx.clone().expect("channel was just created")
    }

    /// Drains any completed loads and uploads them as textures.
    fn drain_completed(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.rx.as_ref() else {
            return;
        };
        while let Ok(loaded) = rx.try_recv() {
            self.loading.remove(&loaded.key);
            if let Some(image) = loaded.image {
                let handle = ctx.load_texture(&loaded.key, image, TextureOptions::LINEAR);
                self.textures.push((loaded.key, handle));
            }
        }
    }

    /// Returns the cached texture for `path`, if one is ready.
    fn get(&self, path: &str) -> Option<&TextureHandle> {
        self.textures
            .iter()
            .find(|(k, _)| k == path)
            .map(|(_, t)| t)
    }
}

/// Show the artwork area: a square image (or placeholder) sized to the
/// panel width.
pub fn show(
    ui: &mut egui::Ui,
    cache: &mut ArtworkCache,
    np: Option<&NowPlayingInfo>,
    track: Option<&TrackInfo>,
) {
    cache.drain_completed(ui.ctx());

    let max_size = ui.available_width().min(320.0);
    let desired_size = egui::vec2(max_size, max_size);

    let key = match np {
        Some(info) => info.path.clone(),
        None => String::new(),
    };

    let texture = if key.is_empty() {
        None
    } else {
        cache.get(&key).cloned().or_else(|| {
            // No texture yet: request it from a worker thread (or generate
            // a placeholder synchronously for non-existent mock paths).
            request(cache, ui.ctx().clone(), &key, track);
            None
        })
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

    if let Some(info) = np {
        response.on_hover_text(&info.path);
    }
}

/// Start loading artwork for `key` off the UI thread, or cache a
/// placeholder immediately for mock paths.
fn request(cache: &mut ArtworkCache, ctx: egui::Context, key: &str, track: Option<&TrackInfo>) {
    if !cache.loading.insert(key.to_string()) {
        // Already in flight.
        return;
    }

    let path = PathBuf::from(key);
    if !path.exists() {
        // Mock / not-yet-scanned path: generate a deterministic placeholder
        // synchronously so screenshots and first frames are stable.
        let image = placeholder_for(key);
        let handle = ctx.load_texture(key, image, TextureOptions::LINEAR);
        cache.textures.push((key.to_string(), handle));
        cache.loading.remove(key);
        return;
    }

    let tx = cache.ensure_channel();
    let key = key.to_string();
    let fallback_dir = track.and_then(|t| Path::new(&t.path).parent().map(Path::to_path_buf));
    std::thread::spawn(move || {
        let image = load_artwork(&path, fallback_dir.as_deref()).map(color_image_from_dynamic);
        let _ = tx.send(LoadedArtwork { key, image });
        ctx.request_repaint();
    });
}

/// Try embedded artwork via `lofty`, then folder images.
///
/// Returns the raw decoded image (not yet converted to egui's [`ColorImage`])
/// so callers such as the album grid (#17) can resize it before uploading.
pub(crate) fn load_artwork(
    path: &Path,
    fallback_dir: Option<&Path>,
) -> Option<image::DynamicImage> {
    // 1. Embedded picture.
    if let Ok(tagged) = lofty::read_from_path(path)
        && let Some(picture) = tagged.primary_tag().and_then(|tag| tag.pictures().first())
        && let Ok(img) = image::load_from_memory(picture.data())
    {
        return Some(img);
    }

    // 2. Folder image fallback.
    if let Some(dir) = path.parent().or(fallback_dir) {
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

/// Convert a decoded `image::DynamicImage` into an egui `ColorImage`.
fn color_image_from_dynamic(img: image::DynamicImage) -> ColorImage {
    let rgba = img.to_rgba8();
    ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba)
}

/// Generate a deterministic placeholder gradient for a path.
fn placeholder_for(key: &str) -> ColorImage {
    let mut hash = 0u64;
    for byte in key.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }

    let w = PLACEHOLDER_SIZE;
    let h = PLACEHOLDER_SIZE;
    let mut pixels = Vec::with_capacity((w * h) as usize);

    let hue = ((hash % 360) as f32) / 360.0;
    let sat = 0.5 + ((hash / 360) % 100) as f32 / 200.0;
    let light = 0.25 + ((hash / 36_000) % 100) as f32 / 200.0;
    let base = hsl_to_rgb(hue, sat, light);
    let accent = hsl_to_rgb((hue + 0.5) % 1.0, sat, light + 0.15);

    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 / w as f32 + y as f32 / h as f32) / 2.0;
            let r = lerp(base[0], accent[0], t);
            let g = lerp(base[1], accent[1], t);
            let b = lerp(base[2], accent[2], t);
            pixels.push(egui::Color32::from_rgb(r, g, b));
        }
    }

    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for pixel in &pixels {
        rgba.push(pixel.r());
        rgba.push(pixel.g());
        rgba.push(pixel.b());
        rgba.push(pixel.a());
    }
    ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba)
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
