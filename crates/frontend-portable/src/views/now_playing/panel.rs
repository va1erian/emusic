//! The now-playing right panel (#110, #451): the shared summary + queue pair
//! on the shell's trailing edge, shown beside every central view while
//! `PanelVisibility::right_panel` is on.
//!
//! It shows the now-playing [`Summary`](super::summary::Summary) above the
//! upcoming-tracks [`QueueList`](super::queue::QueueList), driven by the same
//! shared [`Model`] as the central Now Playing view so the two surfaces stay
//! consistent. Its queue routes jump/context/remove through
//! [`Msg::RightPanelQueueActivate`], [`Msg::RightPanelQueueContext`] and
//! [`Msg::RightPanelQueueRemove`], distinct from the central view's, so the two
//! lists can be told apart while both are visible.

use std::cell::Cell;

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::units::dip;

use crate::app::Msg;

use super::pair::SummaryQueue;

/// Height of the panel's queue list, in design units.
const QUEUE_HEIGHT: f32 = 200.0;

/// The right panel: the now-playing summary above the upcoming-queue list.
pub struct RightPanelView {
    ui: Ui<Msg>,
    pair: SummaryQueue,
    /// Whether the next-tracks list is shown; when off the summary takes its
    /// space. Kept here because [`set_bounds`](Self::set_bounds) reserves the
    /// list's height only while it is on.
    queue_visible: Cell<bool>,
}

impl RightPanelView {
    /// Creates the summary and the queue list, and builds the artwork cache
    /// woken by `waker`.
    pub fn new(ui: &Ui<Msg>, waker: WakerHandle) -> RightPanelView {
        let pair = SummaryQueue::new(
            ui,
            waker,
            |row| Some(Msg::RightPanelQueueActivate(row)),
            |row, at| Some(Msg::RightPanelQueueContext(row, at)),
        );
        RightPanelView {
            ui: ui.clone(),
            pair,
            queue_visible: Cell::new(true),
        }
    }

    /// Moves and sizes the panel: the summary fills the space above the fixed
    /// height queue list, or the whole panel while the list is hidden.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let queue = if self.queue_visible.get() {
            dip(QUEUE_HEIGHT).to_px(dpi).value().min(bounds.height())
        } else {
            0
        };
        let queue_top = bounds.bottom - queue;
        self.pair.set_bounds(
            Rect::new(bounds.left, bounds.top, bounds.right, queue_top),
            Rect::new(bounds.left, queue_top, bounds.right, bounds.bottom),
        );
    }

    /// Shows or hides the whole panel.
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
    }

    /// Shows or hides just the upcoming "next tracks" queue list.
    pub fn set_queue_visible(&self, visible: bool) {
        self.queue_visible.set(visible);
        self.pair.set_queue_visible(visible);
    }

    /// Pushes the model into the summary and the queue, polling the artwork
    /// every tick.
    pub fn sync(&mut self, model: &Model) {
        self.pair.sync(model);
    }

    /// The full queue entry index of the preview row raised by
    /// [`Msg::RightPanelQueueActivate`].
    pub fn entry_index(&self, row: usize) -> Option<usize> {
        self.pair.entry_index(row)
    }

    /// The queue list's top-left corner in window coordinates, for anchoring
    /// its context menu.
    pub fn context_origin(&self) -> Point {
        self.pair.context_origin()
    }
}
