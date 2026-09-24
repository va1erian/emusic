//! Settings → About page (#188, #115): version, repository link and credits.
//!
//! The text is a [`FlowText`] line so the runs wrap and the repository is a
//! clickable link, matching the egui About tab's rich text. When DirectWrite is
//! unavailable, a plain wrapping label carries the same information.

use win32ui::prelude::*;

use crate::app::Msg;

/// Short repository URL shown in the credits.
const REPOSITORY: &str = "https://github.com/va1erian/emusic";
/// Height reserved for the wrapped flow line, in design units.
const FLOW_HEIGHT: f32 = 96.0;
/// Height reserved for the plain-label fallback, in design units.
const FALLBACK_HEIGHT: f32 = 120.0;

/// The About page's text.
pub(super) struct AboutPage {
    flow: Option<FlowText<Msg>>,
    fallback: Option<Label>,
}

impl AboutPage {
    /// Builds the flow line, or a plain label when DirectWrite is unavailable.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        match build_flow(ui) {
            Ok(flow) => Ok(Self {
                flow: Some(flow),
                fallback: None,
            }),
            Err(_) => Ok(Self {
                flow: None,
                fallback: Some(Label::new(ui, Rect::default(), FALLBACK_TEXT)?),
            }),
        }
    }

    /// The page's text as a layout item.
    pub(super) fn items(&self) -> Vec<LayoutItem> {
        match (&self.flow, &self.fallback) {
            (Some(flow), _) => vec![flow.height(dip(FLOW_HEIGHT))],
            (_, Some(label)) => vec![label.height(dip(FALLBACK_HEIGHT))],
            _ => Vec::new(),
        }
    }

    /// Shows or hides the text.
    pub(super) fn set_visible(&self, visible: bool) {
        if let Some(flow) = &self.flow {
            flow.set_visible(visible);
        }
        if let Some(label) = &self.fallback {
            label.set_visible(visible);
        }
    }
}

/// Builds the flowing About line: name, version, repository link and credits.
fn build_flow(ui: &mut Ui<Msg>) -> win32ui::Result<FlowText<Msg>> {
    Ok(FlowText::new(ui)?
        .run(Run::normal("emusic").weight(600))
        .separator("   ")
        .run(Run::weak(format!("Version {}", env!("CARGO_PKG_VERSION"))))
        .separator("   ")
        .run(Run::link(REPOSITORY).on_click(|| {
            open_url(REPOSITORY);
            None
        }))
        .separator("   ")
        .run(Run::weak("Audio playback: BASS by Un4seen Developments"))
        .separator(" · ")
        .run(Run::weak("SID emulation: cRSID by Hermit"))
        .separator(" · ")
        .run(Run::weak("Interface: win32ui (native Win32) / egui"))
        .separator(" · ")
        .run(Run::weak("Licensed under the MIT license.")))
}

/// Opens `url` in the default browser (a `FlowText` link cannot carry a
/// frontend message for this, so it is handled directly).
fn open_url(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

/// Plain-text About content for the no-DirectWrite fallback.
const FALLBACK_TEXT: &str = "emusic\n\
    Version (see the repository for the current release)\n\
    https://github.com/va1erian/emusic\n\
    Audio playback: BASS by Un4seen Developments\n\
    SID emulation: cRSID by Hermit\n\
    Interface: win32ui (native Win32) / egui\n\
    Licensed under the MIT license.";
