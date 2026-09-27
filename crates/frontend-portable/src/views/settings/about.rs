//! Settings -> About (#188, #115, #376): version, repository link and credits,
//! on the portable [`FlowText`] so the runs wrap and the repository is a
//! clickable link.

use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::{FlowText, Run};

use crate::app::Msg;

/// Short repository URL shown in the credits.
pub const REPOSITORY: &str = "https://github.com/va1erian/emusic";

/// Builds the About flow line: name, version, repository link and credits.
pub fn build(ui: &Ui<Msg>, bounds: Rect) -> FlowText<Msg> {
    FlowText::new(ui, bounds)
        .expect("create about text")
        .run(Run::normal("emusic").weight(600).size(16.0))
        .separator("   ")
        .run(Run::weak(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(12.0))
        .run(Run::weak("\n\nA music player and library for Windows.\n\n"))
        .run(Run::link(REPOSITORY).size(12.0).on_click(|| {
            open_url(REPOSITORY);
            None
        }))
        .run(Run::weak(format!(
            "\n\nAudio playback: BASS by Un4seen Developments\n\
             SID emulation: cRSID by Hermit\n\
             Interface: xui\n\
             Rendering backend: {}\n\
             Licensed under the MIT license.",
            crate::backend::label()
        )))
}

/// Opens `url` in the default browser (a `FlowText` link cannot carry a
/// frontend message for this, so it is handled directly).
fn open_url(url: &str) {
    #[cfg(windows)]
    let launched = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(windows))]
    let launched = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = launched;
}
