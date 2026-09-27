//! The summary + queue widget pair shared by the now-playing surfaces
//! (#247, #451): the right panel and the full-width central view both show the
//! same owner-drawn [`Summary`] and upcoming-queue [`QueueList`], kept in step
//! with the shared [`Model`]; only the queue's event messages and the layout
//! around the pair differ per surface, so this is the one place that owns the
//! widgets, the artwork cache and the queue-row bookkeeping.

use std::path::Path;

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::{Point, Rect};

use crate::app::Msg;

use super::artwork::{self, ArtworkCache, ImageSinkImpl};
use super::queue::QueueList;
use super::summary::Summary;

/// A summary widget and a queue list plus the artwork cache and the
/// row-to-queue-entry bookkeeping they share.
pub(super) struct SummaryQueue {
    summary: Summary,
    queue: QueueList,
    cache: ArtworkCache,
    /// The model revision the summary and queue were last built from.
    applied_revision: u64,
}

impl SummaryQueue {
    /// Creates the summary and the queue list, wiring the queue's activation
    /// and context events to the messages `on_activate` and `on_context`
    /// return, and builds the artwork cache woken by `waker`.
    pub(super) fn new(
        ui: &Ui<Msg>,
        waker: WakerHandle,
        on_activate: impl Fn(usize) -> Option<Msg> + 'static,
        on_context: impl Fn(usize, Point) -> Option<Msg> + 'static,
    ) -> SummaryQueue {
        SummaryQueue {
            summary: Summary::new(ui),
            queue: QueueList::new(ui, on_activate, on_context),
            cache: artwork::new_cache(waker),
            applied_revision: u64::MAX,
        }
    }

    /// Pushes the model into the summary and the queue, polling the artwork
    /// every tick and rebuilding the texts only when the revision changed.
    ///
    /// Returns whether the revision changed, so a surface with extra chrome
    /// (the central view's track count) can rebuild it too.
    pub(super) fn sync(&mut self, model: &Model) -> bool {
        self.cache.drain(&mut ImageSinkImpl);
        let request = model.artwork();
        let image = if request.path.is_empty() {
            None
        } else {
            self.cache.get_with_fallback(
                &mut ImageSinkImpl,
                &request.path,
                request.fallback_dir.as_deref().map(Path::new),
            )
        };
        // Borrow the cached image for the identity check; only a changed image
        // is cloned into the summary, so this per-tick sync never copies the
        // pixels.
        self.summary
            .set_artwork(image.and_then(|handle| handle.as_ref()));

        if model.revision() == self.applied_revision {
            return false;
        }
        self.applied_revision = model.revision();
        self.summary.sync(model);
        self.queue.sync(model.queue());
        true
    }

    /// Moves the summary and the queue to `summary` and `queue`.
    pub(super) fn set_bounds(&self, summary: Rect, queue: Rect) {
        self.summary.set_bounds(summary);
        self.queue.set_bounds(queue);
    }

    /// Shows or hides both widgets.
    pub(super) fn set_visible(&self, visible: bool) {
        self.summary.set_visible(visible);
        self.queue.set_visible(visible);
    }

    /// Shows or hides just the upcoming-queue list.
    pub(super) fn set_queue_visible(&self, visible: bool) {
        self.queue.set_visible(visible);
    }

    /// The full queue entry index of preview `row`, if it exists.
    pub(super) fn entry_index(&self, row: usize) -> Option<usize> {
        self.queue.entry_index(row)
    }

    /// The queue list's top-left corner in window coordinates, for anchoring
    /// its context menu.
    pub(super) fn context_origin(&self) -> Point {
        self.queue.context_origin()
    }
}
