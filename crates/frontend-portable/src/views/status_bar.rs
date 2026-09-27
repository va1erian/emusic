//! The bottom status bar (#375): the shared part texts on the portable
//! [`MaterialStatusBar`], with the owner-drawn visualizer strip (#25) at its
//! right end.
//!
//! Render-only: the strings come from
//! [`emusic_ui::panels::status_bar::StatusBar`]; this view maps them onto the
//! bar's three parts (result count, player state, progress/notice). The
//! visualizer strip is a painted [`Control`](xui::xui_core::widget::Control)
//! over the portable `Canvas`, fed from the player each frame.

use std::cell::Cell;

use emusic_ui::panels::status_bar::StatusBar as StatusBarModel;
use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::VisualizerMode;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::widget::MaterialStatusBar;

use crate::app::Msg;
use crate::views::visualizer_strip::{
    HEIGHT as STRIP_HEIGHT, VisualizerStrip, WIDTH as STRIP_WIDTH,
};

/// The bar's parts, in order.
const PARTS: usize = 3;
/// Part 0: the track / search-result count.
const RESULT_COUNT: usize = 0;
/// Part 1: the player state (`Playing`, `Paused`, `Ready`).
const STATUS: usize = 1;
/// Part 2: scan / auto-tag / notice progress.
const PROGRESS: usize = 2;

/// The bottom bar's three text parts plus its visualizer strip.
pub struct StatusBarView {
    ui: Ui<Msg>,
    bar: MaterialStatusBar<Msg>,
    strip: VisualizerStrip,
    /// Whether the status band itself is shown, so the strip's own visibility
    /// can combine it with the visualizer mode.
    visible: Cell<bool>,
    /// Whether a mode other than Off is enabled, so the strip is shown at all.
    strip_active: Cell<bool>,
    applied_revision: u64,
    applied_progress: String,
}

impl StatusBarView {
    /// Creates the bar with three empty parts and the visualizer strip;
    /// [`sync`](Self::sync) fills the texts and [`feed`](Self::feed) drives the
    /// strip.
    pub fn new(ui: &Ui<Msg>) -> StatusBarView {
        let empty = [""; PARTS];
        let bar = MaterialStatusBar::new(ui, Rect::default(), &empty).expect("create status bar");
        let strip = VisualizerStrip::new(ui);
        strip.set_visible(false);
        StatusBarView {
            ui: ui.clone(),
            bar,
            strip,
            visible: Cell::new(true),
            strip_active: Cell::new(false),
            applied_revision: u64::MAX,
            applied_progress: String::new(),
        }
    }

    /// Moves/resizes the bar and places the visualizer strip at its right end.
    ///
    /// The strip overlaps the bar's right (progress/notice) cell while the
    /// visualizer is on; the bar's own text is left-aligned, so it never shows
    /// under the strip in practice.
    pub fn set_bounds(&self, bounds: Rect) {
        self.ui.apply_moves(&[(self.bar.id(), bounds)]);
        let dpi = self.ui.dpi();
        let width = STRIP_WIDTH.to_px(dpi).value();
        let height = STRIP_HEIGHT.to_px(dpi).value().min(bounds.height());
        let right = bounds.right - 4;
        let left = (right - width).max(bounds.left);
        let top = bounds.top + (bounds.height() - height) / 2;
        self.strip
            .set_bounds(Rect::new(left, top, right, top + height));
    }

    /// Shows or hides the bar and its strip.
    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        self.ui.set_visible(self.bar.id(), visible);
        self.apply_strip_visibility();
    }

    /// Feeds the visualizer strip this frame; the mode and its enable flag come
    /// from the shell state.
    pub fn feed(&self, enabled: bool, mode: VisualizerMode, player: &dyn PlayerApi) {
        let active = enabled && mode != VisualizerMode::Off;
        if self.strip_active.replace(active) != active {
            self.apply_strip_visibility();
        }
        self.strip.feed(enabled, mode, player);
    }

    /// Shows the strip only when the band is visible and a mode is active.
    fn apply_strip_visibility(&self) {
        self.strip
            .set_visible(self.visible.get() && self.strip_active.get());
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
