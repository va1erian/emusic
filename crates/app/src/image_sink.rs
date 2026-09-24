//! egui's [`ImageSink`]: uploads decoded images as GPU textures (#96).

use eframe::egui;
use emusic_ui::image_cache::{ImageSink, Rgba8Image};

/// Uploads images into an [`egui::Context`] as named textures.
///
/// `prefix` namespaces texture names so the thumbnail and artwork caches
/// never collide on the same key.
pub struct EguiImageSink {
    ctx: egui::Context,
    prefix: &'static str,
}

impl EguiImageSink {
    #[must_use]
    pub fn new(ctx: egui::Context, prefix: &'static str) -> Self {
        Self { ctx, prefix }
    }
}

impl ImageSink for EguiImageSink {
    type Handle = egui::TextureHandle;

    fn upload(&mut self, key: u64, image: &Rgba8Image) -> egui::TextureHandle {
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.pixels,
        );
        self.ctx.load_texture(
            format!("{}_{key:016x}", self.prefix),
            color,
            egui::TextureOptions::LINEAR,
        )
    }
}
