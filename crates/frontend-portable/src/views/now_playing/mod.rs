//! The Now Playing central view (#247) and the shared now-playing right panel
//! (#110, #451), ported to the portable widget layer.
//!
//! Both surfaces show the same artwork/metadata summary and upcoming-queue
//! list the pre-migration Win32 views showed: a painted summary over a virtual
//! queue list, driven by the shared
//! [`NowPlayingView`](emusic_ui::views::now_playing::NowPlayingView) model. The
//! widgets and the artwork cache live in the `pair` module, so the central view
//! and the right panel stay consistent instead of duplicating the drawing. Display
//! strings and intents all come from the model; these modules only draw, route
//! clicks and decode artwork through the shared image cache.

mod artwork;
mod pair;
mod panel;
mod queue;
mod summary;

pub use panel::RightPanelView;
pub use summary::SummaryEvent;

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label};

use crate::app::Msg;

use pair::SummaryQueue;

/// Height of the central view's queue list, in design units.
const QUEUE_HEIGHT: f32 = 320.0;
/// Height of the central view's queue "N tracks" header, in design units.
const HEADER_HEIGHT: f32 = 22.0;

/// The Now Playing central view: the summary above a full-width queue list.
pub struct NowPlayingView {
    ui: Ui<Msg>,
    pair: SummaryQueue,
    count: Label<Msg>,
}

impl NowPlayingView {
    /// Creates the summary, the queue header and the (empty) queue list, and
    /// builds the artwork cache woken by `waker`.
    pub fn new(ui: &Ui<Msg>, waker: WakerHandle) -> NowPlayingView {
        let pair = SummaryQueue::new(
            ui,
            waker,
            |row| Some(Msg::QueueActivate(row)),
            |row, at| Some(Msg::QueueContext(row, at)),
        );
        let count =
            Label::new(ui, Rect::default(), "0 tracks").expect("create now-playing queue header");
        NowPlayingView {
            ui: ui.clone(),
            pair,
            count,
        }
    }

    /// Moves and sizes the view: the summary fills the space above the fixed
    /// height queue header and list.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let queue = dip(QUEUE_HEIGHT).to_px(dpi).value().min(bounds.height());
        let queue_top = bounds.bottom - queue;
        self.pair.set_bounds(
            Rect::new(bounds.left, bounds.top, bounds.right, queue_top),
            Rect::new(bounds.left, queue_top + header, bounds.right, bounds.bottom),
        );
        self.ui.apply_moves(&[(
            self.count.id(),
            Rect::new(bounds.left, queue_top, bounds.right, queue_top + header),
        )]);
    }

    /// Shows or hides the whole view.
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
        self.ui.set_visible(self.count.id(), visible);
    }

    /// Pushes the model into the summary and the queue, polling the artwork
    /// every tick and rebuilding the count only when the revision changed.
    pub fn sync(&mut self, model: &Model) {
        if self.pair.sync(model) {
            self.count.set_text(&model.queue_count_text());
        }
    }

    /// The full queue entry index of the preview row raised by
    /// [`Msg::QueueActivate`](crate::app::Msg::QueueActivate).
    pub fn entry_index(&self, row: usize) -> Option<usize> {
        self.pair.entry_index(row)
    }

    /// The queue list's top-left corner in window coordinates, for anchoring
    /// its context menu.
    pub fn context_origin(&self) -> Point {
        self.pair.context_origin()
    }
}
