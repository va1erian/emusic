//! Installs the fonts already present on the machine, so the app ships no
//! bundled fonts (and therefore no third-party font licences). Segoe UI is
//! the primary UI font, with CJK/symbol/emoji fonts appended as fallbacks so
//! track/artist names in those scripts render instead of showing tofu boxes.
//!
//! Fonts are read from the Windows fonts directory. When none of the expected
//! fonts are available (non-Windows dev box, stripped CI image) a warning is
//! logged and egui's own defaults are used instead.

use std::path::Path;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use tracing::warn;

/// Directory holding the installed fonts on Windows.
const WINDOWS_FONTS_DIR: &str = r"C:\Windows\Fonts";

/// Primary proportional (UI) font candidates, in priority order.
const PROPORTIONAL: &[(&str, &str)] = &[("segoe-ui", "segoeui.ttf")];

/// Primary monospace font candidates, in priority order.
const MONOSPACE: &[(&str, &str)] = &[("consolas", "consola.ttf"), ("courier-new", "cour.ttf")];

/// Fallbacks appended to *both* families: CJK, then symbols and emoji.
const FALLBACKS: &[(&str, &str)] = &[
    ("msyh", "msyh.ttc"),         // Simplified Chinese
    ("yugothm", "YuGothM.ttc"),   // Japanese
    ("malgun", "malgun.ttf"),     // Korean
    ("seguisym", "seguisym.ttf"), // symbols (transport glyphs, arrows, ...)
    ("seguiemj", "seguiemj.ttf"), // emoji
];

/// Installs system fonts into `ctx`, falling back to egui's defaults (with a
/// warning) when the machine has none of the expected fonts.
pub fn install(ctx: &egui::Context) {
    match build(Path::new(WINDOWS_FONTS_DIR)) {
        Some(fonts) => ctx.set_fonts(fonts),
        None => {
            warn!(
                "no usable system fonts under {WINDOWS_FONTS_DIR}; \
                 falling back to egui's bundled defaults"
            );
            ctx.set_fonts(FontDefinitions::default());
        }
    }
}

/// Builds font definitions from the fonts installed in `dir`. Returns `None`
/// when no usable proportional font is present, so the caller can fall back.
fn build(dir: &Path) -> Option<FontDefinitions> {
    let mut fonts = FontDefinitions::empty();

    let fallbacks = load_all(dir, FALLBACKS, &mut fonts);

    let mut proportional = vec![load_first(dir, PROPORTIONAL, &mut fonts)?];
    proportional.extend(fallbacks.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional.clone());

    let mut monospace: Vec<String> = load_first(dir, MONOSPACE, &mut fonts).into_iter().collect();
    if monospace.is_empty() {
        // No monospace font installed: reuse the proportional chain so
        // monospace text (e.g. module Order/Row) still renders.
        monospace = proportional;
    } else {
        monospace.extend(fallbacks);
    }
    fonts.families.insert(FontFamily::Monospace, monospace);

    Some(fonts)
}

/// Loads every font in `candidates` that exists under `dir`, returning the
/// keys that were registered, in order.
fn load_all(dir: &Path, candidates: &[(&str, &str)], fonts: &mut FontDefinitions) -> Vec<String> {
    let mut keys = Vec::new();
    for (key, file) in candidates {
        if load(dir, key, file, fonts) {
            keys.push((*key).to_owned());
        }
    }
    keys
}

/// Loads the first font in `candidates` that exists under `dir`, returning its
/// key (`None` when none exist).
fn load_first(
    dir: &Path,
    candidates: &[(&str, &str)],
    fonts: &mut FontDefinitions,
) -> Option<String> {
    candidates
        .iter()
        .find_map(|(key, file)| load(dir, key, file, fonts).then(|| (*key).to_owned()))
}

/// Reads `dir/file` and registers it under `key`; returns whether it existed.
fn load(dir: &Path, key: &str, file: &str, fonts: &mut FontDefinitions) -> bool {
    let Ok(bytes) = std::fs::read(dir.join(file)) else {
        return false;
    };
    fonts
        .font_data
        .insert(key.to_owned(), FontData::from_owned(bytes).into());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_returns_none_without_fonts() {
        let dir = std::env::temp_dir().join(format!("emusic-no-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let built = build(&dir);
        std::fs::remove_dir(&dir).ok();
        assert!(
            built.is_none(),
            "empty dir should yield no font definitions"
        );
    }

    #[test]
    fn build_registers_windows_fonts_when_present() {
        let dir = Path::new(WINDOWS_FONTS_DIR);
        if !dir.join("segoeui.ttf").is_file() {
            eprintln!("skipping: {WINDOWS_FONTS_DIR} has no segoeui.ttf");
            return;
        }
        let fonts = build(dir).expect("system fonts should build");
        let proportional = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family");
        assert_eq!(proportional.first().map(String::as_str), Some("segoe-ui"));
        assert!(
            !fonts
                .families
                .get(&FontFamily::Monospace)
                .expect("monospace family")
                .is_empty()
        );
    }
}
