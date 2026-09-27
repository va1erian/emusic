//! Settings → About page (#188, #115): version, repository link and credits.
//!
//! The text is a [`FlowText`] line so the runs wrap and the repository is a
//! clickable link. When DirectWrite is unavailable, a plain wrapping label
//! carries the same information.

use win32ui::prelude::*;

use crate::app::Msg;

use super::{FormRow, ScrollPanel};

/// Short repository URL shown in the credits.
const REPOSITORY: &str = "https://github.com/va1erian/emusic";
/// Height reserved for the wrapped flow line, in design units: the eight lines
/// the flow renders (the repository link is set off by blank lines).
const FLOW_HEIGHT: f32 = 160.0;
/// Height reserved for the plain-label fallback, in design units.
const FALLBACK_HEIGHT: f32 = 120.0;

/// The About page's text.
pub(super) struct AboutPage {
    form: ScrollPanel,
    flow: Option<FlowText<Msg>>,
    fallback: Option<Label>,
}

impl AboutPage {
    /// Builds the flow line, or a plain label when DirectWrite is unavailable.
    pub(super) fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let form = ScrollPanel::new(ui)?;
        let mut panel = form.ui(ui);
        let (flow, fallback) = match build_flow(&mut panel) {
            Ok(flow) => (Some(flow), None),
            Err(_) => (
                None,
                Some(Label::new(&mut panel, Rect::default(), FALLBACK_TEXT)?),
            ),
        };
        let page = Self {
            form,
            flow,
            fallback,
        };
        page.apply(ui);
        Ok(page)
    }

    /// The page's scrollable form as one tab-strip page.
    pub(super) fn page(&self) -> LayoutItem {
        self.form.page()
    }

    /// The page's text as a form row.
    fn rows(&self) -> Vec<FormRow> {
        match (&self.flow, &self.fallback) {
            (Some(flow), _) => vec![(flow.height(dip(FLOW_HEIGHT)), FLOW_HEIGHT)],
            (_, Some(label)) => vec![(label.height(dip(FALLBACK_HEIGHT)), FALLBACK_HEIGHT)],
            _ => Vec::new(),
        }
    }

    /// Reinstalls the page's form (used after a visibility change).
    fn apply(&self, ui: &Ui<Msg>) {
        self.form.apply(ui, self.rows());
    }

    /// Shows or hides the whole page.
    pub(super) fn set_visible(&self, visible: bool) {
        self.form.set_visible(visible);
    }
}

/// Builds the flowing About line: name, version, repository link and credits.
fn build_flow(ui: &mut Ui<Msg>) -> win32ui::Result<FlowText<Msg>> {
    Ok(FlowText::new(ui)?
        .run(Run::normal("emusic").weight(600))
        .separator("   ")
        .run(Run::weak(format!("Version {}", env!("CARGO_PKG_VERSION"))))
        .line_break()
        .line_break()
        .run(Run::link(REPOSITORY).on_click(|| {
            open_url(REPOSITORY);
            None
        }))
        .line_break()
        .line_break()
        .run(Run::weak("Audio playback: BASS by Un4seen Developments"))
        .line_break()
        .run(Run::weak("SID emulation: cRSID by Hermit"))
        .line_break()
        .run(Run::weak("Interface: win32ui (native Win32)"))
        .line_break()
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
    Interface: win32ui (native Win32)\n\
    Licensed under the MIT license.";
