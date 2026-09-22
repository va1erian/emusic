//! Loads Segoe UI plus CJK/symbol fallbacks from `C:\Windows\Fonts` when
//! present, so track/artist names in those scripts render instead of
//! showing tofu boxes. Falls back silently to egui's bundled fonts when a
//! file is missing (e.g. non-Windows dev machines, minimal CI images).

use std::path::Path;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

const WINDOWS_FONTS_DIR: &str = r"C:\Windows\Fonts";

/// (font key, file name, whether to also register as a monospace fallback)
const CANDIDATES: &[(&str, &str)] = &[
    ("segoe-ui", "segoeui.ttf"),
    ("msyh", "msyh.ttc"),         // Simplified Chinese
    ("yugothm", "YuGothM.ttc"),   // Japanese
    ("malgun", "malgun.ttf"),     // Korean
    ("seguisym", "seguisym.ttf"), // symbols/emoji-ish glyphs
];

/// Installs fonts into `ctx`. Safe to call with the Windows fonts directory
/// absent (falls through to egui's defaults).
pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir = Path::new(WINDOWS_FONTS_DIR);
    let mut loaded_any = false;
    let mut fallback_order = Vec::new();

    for (key, file) in CANDIDATES {
        let path = dir.join(file);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        fonts
            .font_data
            .insert((*key).to_owned(), FontData::from_owned(bytes).into());
        fallback_order.push((*key).to_owned());
        loaded_any = true;
    }

    if !loaded_any {
        return; // keep egui's bundled defaults entirely
    }

    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        // Segoe UI first (primary Windows UI font), then CJK/symbol
        // fallbacks, then egui's existing bundled fonts stay at the end
        // for anything not covered above.
        for key in fallback_order.into_iter().rev() {
            proportional.insert(0, key);
        }
    }

    ctx.set_fonts(fonts);
}
