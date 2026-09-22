//! One cover tile in the album grid: square artwork (or a placeholder),
//! optional selection outline, and the album/artist/year caption beneath.

use eframe::egui;

use crate::library_api::AlbumInfo;

/// Height reserved under the cover for the caption.
pub(super) const CAPTION_HEIGHT: f32 = 42.0;
const CORNER_RADIUS: f32 = 4.0;

/// Draws one album tile and returns its click response.
pub(super) fn show(
    ui: &mut egui::Ui,
    album: &AlbumInfo,
    texture: Option<&egui::TextureHandle>,
    size: f32,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(size, size + CAPTION_HEIGHT),
        egui::Sense::click(),
    );
    let cover = egui::Rect::from_min_size(rect.min, egui::vec2(size, size));

    ui.painter()
        .rect_filled(cover, CORNER_RADIUS, ui.visuals().extreme_bg_color);
    match texture {
        Some(texture) => {
            ui.painter().image(
                texture.id(),
                cover,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            ui.painter()
                .rect_filled(cover, CORNER_RADIUS, placeholder_color(&album.name));
            ui.painter().text(
                cover.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(size * 0.28),
                egui::Color32::from_white_alpha(200),
            );
        }
    }
    if selected {
        ui.painter().rect_stroke(
            cover,
            CORNER_RADIUS,
            egui::Stroke::new(2.0, crate::theme::current_accent()),
            egui::StrokeKind::Inside,
        );
    }

    let caption = egui::Rect::from_min_max(egui::pos2(rect.min.x, cover.max.y + 2.0), rect.max);
    let mut caption_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(caption)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    caption_ui.add(egui::Label::new(egui::RichText::new(&album.name).strong()).truncate());
    caption_ui.add(egui::Label::new(egui::RichText::new(&album.artist).weak()).truncate());
    if let Some(year) = album.year {
        caption_ui.add(egui::Label::new(
            egui::RichText::new(year.to_string()).weak().small(),
        ));
    }

    let hover = match album.year {
        Some(year) => format!("{}\n{}\n{}", album.name, album.artist, year),
        None => format!("{}\n{}", album.name, album.artist),
    };
    response.on_hover_text(hover)
}

/// Deterministic cover colour derived from an album name, shown until real
/// artwork is decoded (or permanently when the album has none). Kept in the
/// low-saturation/mid-value range so the white note glyph stays readable in
/// both themes.
pub(super) fn placeholder_color(seed: &str) -> egui::Color32 {
    let mut value = 0u64;
    for byte in seed.bytes() {
        value = value.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    let hue = (value % 360) as f32 / 360.0;
    let saturation = 0.35 + ((value / 360) % 40) as f32 / 100.0;
    let brightness = 0.35 + ((value / 14_400) % 30) as f32 / 100.0;
    egui::ecolor::Hsva::new(hue, saturation, brightness, 1.0).into()
}
