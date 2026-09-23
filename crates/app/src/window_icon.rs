//! The OS window/taskbar icon (#125), decoded from the committed app icon
//! and handed to egui via `ViewportBuilder::with_icon`.
//!
//! The PNG is embedded at compile time with `include_bytes!`, so the window
//! icon works no matter where the executable is run from — unlike the
//! file-association icons, it does not have to sit next to the exe.

use eframe::egui;

/// RGBA pixels for [`egui::IconData`] built from the 256px app icon, or
/// `None` if it fails to decode (logged, not fatal: a missing window icon is
/// purely cosmetic).
pub fn window_icon() -> Option<egui::IconData> {
    const PNG: &[u8] = include_bytes!("../../../assets/png/256/emusic-app.png");
    let image = match image::load_from_memory(PNG) {
        Ok(image) => image.to_rgba8(),
        Err(err) => {
            tracing::warn!(%err, "could not decode the app window icon");
            return None;
        }
    };
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
