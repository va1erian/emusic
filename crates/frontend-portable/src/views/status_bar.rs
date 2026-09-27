//! The bottom status bar (#375): the shared part texts on the portable
//! [`MaterialStatusBar`].
//!
//! Render-only: the strings come from
//! [`emusic_ui::panels::status_bar::StatusBar`]; this view maps them onto the
//! bar's three parts (result count, player state, progress/notice).

use emusic_ui::panels::status_bar::StatusBar as StatusBarModel;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::MaterialStatusBar;

use crate::app::Msg;

/// The bar's parts, in order.
const PARTS: usize = 3;
/// Part 0: the track / search-result count.
const RESULT_COUNT: usize = 0;
/// Part 1: the player state (`Playing`, `Paused`, `Ready`).
const STATUS: usize = 1;
/// Part 2: scan / auto-tag / notice progress.
const PROGRESS: usize = 2;

/// The bottom bar's three text parts.
pub struct StatusBarView {
    ui: Ui<Msg>,
    bar: MaterialStatusBar<Msg>,
    applied_revision: u64,
    applied_progress: String,
}

impl StatusBarView {
    /// Creates the bar with three empty parts; [`sync`](Self::sync) fills them.
    pub fn new(ui: &Ui<Msg>) -> StatusBarView {
        let empty = [""; PARTS];
        let bar = MaterialStatusBar::new(ui, Rect::default(), &empty).expect("create status bar");
        StatusBarView {
            ui: ui.clone(),
            bar,
            applied_revision: u64::MAX,
            applied_progress: String::new(),
        }
    }

    /// Moves/resizes the bar.
    pub fn set_bounds(&self, bounds: Rect) {
        self.ui.apply_moves(&[(self.bar.id(), bounds)]);
    }

    /// Shows or hides the bar.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.bar.id(), visible);
    }

    /// Pushes the model's part texts, skipping the update when nothing changed.
    ///
    /// `notice` is the shell's one-line startup/backend notice; it takes
    /// priority over the model's progress line.
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
        self.bar.set_text(RESULT_COUNT, model.result_count());
        self.bar.set_text(STATUS, model.status());
        self.bar.set_text(PROGRESS, &self.applied_progress);
    }
}
