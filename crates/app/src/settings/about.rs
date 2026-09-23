//! Settings → About tab (#188): version, git revision and credits.

use eframe::egui;

/// Crate version, kept in step with the release tag by the release workflow.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git revision captured by `build.rs`.
const REVISION: &str = env!("EMUSIC_GIT_REV");
const REPOSITORY: &str = "https://github.com/va1erian/emusic";

/// One-line version summary, e.g. `0.1.0 (rev abc123def)`.
pub fn version_string() -> String {
    format!("{VERSION} (rev {REVISION})")
}

pub fn show(ui: &mut egui::Ui) {
    ui.heading("emusic");
    ui.label(format!("Version {}", version_string()));
    ui.hyperlink_to("Source code & issues", REPOSITORY);

    ui.add_space(12.0);
    ui.label(egui::RichText::new("Credits").strong());
    ui.label("Audio playback: BASS by Un4seen Developments");
    ui.label("SID emulation: cRSID by Hermit");
    ui.label("Interface: egui / eframe");
    ui.label("Licensed under the MIT license.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_has_version_and_revision() {
        let s = version_string();
        assert!(s.starts_with(VERSION));
        assert!(s.contains("rev "));
    }
}
