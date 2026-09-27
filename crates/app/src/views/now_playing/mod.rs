//! The Now Playing central view (#247), ported to the portable widget layer.
//!
//! The same artwork/metadata summary and upcoming-queue list the pre-migration
//! Win32 central view showed: a painted [`Summary`] over a virtual queue list,
//! both driven by the shared
//! [`NowPlayingView`](emusic_ui::views::now_playing::NowPlayingView) model.
//! Display strings and intents all come from the model; this module only draws,
//! routes clicks and decodes artwork through the shared image cache.

mod artwork;
mod queue;
mod summary;

pub use summary::SummaryEvent;

use std::path::Path;

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label};

use crate::app::Msg;

use queue::QueueList;
use summary::Summary;

/// Height of the queue list, in design units.
const QUEUE_HEIGHT: f32 = 320.0;
/// Height of the queue's "N tracks" header, in design units.
const HEADER_HEIGHT: f32 = 22.0;

/// The Now Playing central view: the summary above a full-width queue list.
pub struct NowPlayingView {
    ui: Ui<Msg>,
    summary: Summary,
    count: Label<Msg>,
    queue: QueueList,
    cache: artwork::ArtworkCache,
    /// The model revision the summary and queue were last built from.
    applied_revision: u64,
}

impl NowPlayingView {
    /// Creates the summary, the queue header and the (empty) queue list, and
    /// builds the artwork cache woken by `waker`.
    pub fn new(ui: &Ui<Msg>, waker: WakerHandle) -> NowPlayingView {
        let summary = Summary::new(ui);
        let count =
            Label::new(ui, Rect::default(), "0 tracks").expect("create now-playing queue header");
        let queue = QueueList::new(ui);
        NowPlayingView {
            ui: ui.clone(),
            summary,
            count,
            queue,
            cache: artwork::new_cache(waker),
            applied_revision: u64::MAX,
        }
    }

    /// Moves and sizes the view: the summary fills the space above the fixed
    /// height queue header and list.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let queue = dip(QUEUE_HEIGHT).to_px(dpi).value().min(bounds.height());
        let queue_top = bounds.bottom - queue;
        self.summary
            .set_bounds(Rect::new(bounds.left, bounds.top, bounds.right, queue_top));
        self.ui.apply_moves(&[(
            self.count.id(),
            Rect::new(bounds.left, queue_top, bounds.right, queue_top + header),
        )]);
        self.queue.set_bounds(Rect::new(
            bounds.left,
            queue_top + header,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the whole view.
    pub fn set_visible(&self, visible: bool) {
        self.summary.set_visible(visible);
        self.ui.set_visible(self.count.id(), visible);
        self.queue.set_visible(visible);
    }

    /// Pushes the model into the summary and the queue, polling the artwork
    /// every tick and rebuilding the texts only when the revision changed.
    pub fn sync(&mut self, model: &Model) {
        self.cache.drain(&mut artwork::ImageSinkImpl);
        let request = model.artwork();
        let image = if request.path.is_empty() {
            None
        } else {
            self.cache
                .get_with_fallback(
                    &mut artwork::ImageSinkImpl,
                    &request.path,
                    request.fallback_dir.as_deref().map(Path::new),
                )
                .cloned()
                .flatten()
        };
        self.summary.set_artwork(image);

        if model.revision() != self.applied_revision {
            self.applied_revision = model.revision();
            self.summary.sync(model);
            self.queue.sync(model.queue());
            self.count.set_text(&model.queue_count_text());
        }
    }

    /// The full queue entry index of the preview row raised by
    /// [`Msg::QueueActivate`](crate::app::Msg::QueueActivate).
    pub fn entry_index(&self, row: usize) -> Option<usize> {
        self.queue.entry_index(row)
    }
}
