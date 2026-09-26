//! Win32 status bar (#109): the shared part texts on xui's
//! `MaterialStatusBar` (drawn on the window's bottom backdrop band), falling
//! back to a child `StatusBar` when the material bar is unavailable.
//!
//! Render-only: the strings come from
//! [`emusic_ui::panels::status_bar::StatusBar`]; this view only maps them onto
//! the control's parts.

use emusic_ui::panels::status_bar::StatusBar as StatusBarModel;
use xui::prelude::*;
use xui::{MaterialStatusBar, StatusBar, dip};

use crate::app::Msg;

/// Right edge of the first part (result count), in device-independent pixels.
const FIRST_PART: f32 = 200.0;
/// Right edge of the second part (player state), in device-independent pixels.
const SECOND_PART: f32 = 320.0;

/// The native bar behind the view.
enum Bar {
    /// Painted on the window's bottom material band; not a layout item.
    Material(MaterialStatusBar<Msg>),
    /// A child status bar window, laid out as the last row.
    Child(StatusBar<Msg>),
}

/// The Win32 status bar: three parts — result count, player state, progress.
pub struct StatusBarView {
    bar: Bar,
    applied_revision: u64,
    applied_progress: String,
}

impl StatusBarView {
    /// Creates the bar (the material one when the window supports it) and
    /// fixes its part edges.
    pub fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let dpi = ui.dpi();
        let parts = [
            dip(FIRST_PART).to_px(dpi).value(),
            dip(SECOND_PART).to_px(dpi).value(),
            -1,
        ];
        let bar = match MaterialStatusBar::new(ui) {
            Ok(bar) => {
                bar.set_parts(&parts);
                Bar::Material(bar)
            }
            Err(error) => {
                tracing::debug!(%error, "material status bar unavailable, using the child bar");
                let bar = StatusBar::new(ui)?;
                bar.set_parts(&parts);
                Bar::Child(bar)
            }
        };
        Ok(Self {
            bar,
            applied_revision: u64::MAX,
            applied_progress: String::new(),
        })
    }

    /// Pushes the model's part texts, skipping the update when nothing changed.
    ///
    /// `notice` is the shell's one-line startup/backend notice, which takes
    /// priority over the model's progress line (it is shell state, not part of
    /// the status-bar model).
    pub fn sync(&mut self, model: &StatusBarModel, notice: Option<&str>) {
        let progress = notice
            .map(str::to_owned)
            .or_else(|| model.scan().map(str::to_owned))
            .or_else(|| model.auto_tag().map(str::to_owned))
            .or_else(|| model.status_message().map(str::to_owned))
            .unwrap_or_default();
        if model.revision() == self.applied_revision && progress == self.applied_progress {
            return;
        }
        self.applied_revision = model.revision();
        self.applied_progress = progress;
        self.set_text(0, model.result_count());
        self.set_text(1, model.status());
        self.set_text(2, &self.applied_progress);
    }

    /// Shows or hides the bar; the caller relayouts afterwards.
    ///
    /// The material bar can't be hidden yet (va1erian/xui#129), so it
    /// stays shown.
    pub fn set_visible(&self, visible: bool) {
        if let Bar::Child(bar) = &self.bar {
            bar.set_visible(visible);
        }
    }

    /// The child bar as the layout's last row, or `None` for the material bar
    /// (which reserves its band through [`bottom_margin`](Self::bottom_margin)).
    pub fn layout_item(&self) -> Option<&StatusBar<Msg>> {
        match &self.bar {
            Bar::Child(bar) => Some(bar),
            Bar::Material(_) => None,
        }
    }

    /// The bottom layout margin that reserves the material band (zero for the
    /// child bar).
    pub fn bottom_margin(&self, ui: &Ui<Msg>) -> Dip {
        match &self.bar {
            Bar::Material(_) => ui.material_status_bar_height(),
            Bar::Child(_) => dip(0.0),
        }
    }

    fn set_text(&self, part: usize, text: &str) {
        match &self.bar {
            Bar::Material(bar) => bar.set_text(part, text),
            Bar::Child(bar) => bar.set_text(part, text),
        }
    }
}
