//! Win32 status bar (#109): the shared part texts on a native `StatusBar`.
//!
//! Render-only: the strings come from
//! [`emusic_ui::panels::status_bar::StatusBar`]; this view only maps them onto
//! the control's parts.

use emusic_ui::panels::status_bar::StatusBar as StatusBarModel;
use win32ui::prelude::*;
use win32ui::{StatusBar, dip};

use crate::app::Msg;

/// Right edge of the first part (result count), in device-independent pixels.
const FIRST_PART: f32 = 200.0;
/// Right edge of the second part (player state), in device-independent pixels.
const SECOND_PART: f32 = 320.0;

/// The Win32 status bar: three parts — result count, player state, progress.
pub struct StatusBarView {
    bar: StatusBar<Msg>,
    applied_revision: u64,
    applied_progress: String,
}

impl StatusBarView {
    /// Creates the bar and fixes its part edges.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let bar = StatusBar::new(ui)?;
        let dpi = ui.dpi();
        bar.set_parts(&[
            dip(FIRST_PART).to_px(dpi).value(),
            dip(SECOND_PART).to_px(dpi).value(),
            -1,
        ]);
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
        self.bar.set_text(0, model.result_count());
        self.bar.set_text(1, model.status());
        self.bar.set_text(2, &self.applied_progress);
    }
}

impl AsControl for StatusBarView {
    fn control(&self) -> &Control {
        self.bar.control()
    }
}
