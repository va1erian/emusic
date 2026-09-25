//! The summary + queue widget pair shared by both now-playing surfaces
//! (#247): the right panel and the full-width central view. Both surfaces
//! show the same owner-drawn summary and the same upcoming-queue list, kept
//! in step with the shared [`emusic_ui::views::now_playing::NowPlayingView`]
//! model; only the queue's event messages and the layout around the pair
//! differ per surface, so this is the one place that owns the widgets, the
//! artwork cache and the queue-row bookkeeping.

use std::cell::Cell;
use std::path::Path;

use emusic_ui::views::now_playing::{NowPlayingView as Model, QueueRow};
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;

use crate::app::Msg;

use super::artwork::{self, ArtworkCache, Win32ImageSink};
use super::queue::{self, QueueItem};
use super::summary::SummaryWidget;

/// A summary widget and a queue list, plus the artwork cache and the
/// row-to-queue-index bookkeeping they share.
pub(super) struct WidgetPair {
    pub(super) summary: Custom<SummaryWidget, Msg>,
    pub(super) queue: ListView<QueueItem, Msg>,
    cache: ArtworkCache,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
    /// Queue entry index for each preview row, for jump/remove.
    indices: Vec<usize>,
    applied_revision: u64,
}

impl WidgetPair {
    /// Creates the summary widget and the queue list, wiring the queue's
    /// jump/context/remove events to the surface-specific messages the
    /// caller supplies, and builds the artwork cache woken by `waker`.
    pub(super) fn new(
        ui: &mut Ui<Msg>,
        waker: WakerHandle,
        on_activate: impl Fn(usize) -> Option<Msg> + 'static,
        on_context: impl Fn(usize) -> Option<Msg> + 'static,
        on_remove: impl Fn() -> Msg + 'static,
    ) -> win32ui::Result<Self> {
        let summary =
            Custom::new(ui, SummaryWidget::new())?.on_event(|event| Some(Msg::NowPlaying(event)));
        let queue = queue::build(ui, on_activate, on_context)?;
        Ok(Self {
            summary,
            queue,
            cache: artwork::new_cache(waker),
            context: queue::context_menu(on_remove),
            context_row: Cell::new(None),
            indices: Vec::new(),
            applied_revision: u64::MAX,
        })
    }

    /// Shows or hides both native controls.
    pub(super) fn set_visible(&self, visible: bool) {
        self.summary.set_visible(visible);
        self.queue.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag to the queue list
    /// and the summary fonts (#309).
    pub(super) fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.queue);
        self.summary
            .widget()
            .borrow_mut()
            .set_metrics(crate::appearance::metrics());
        self.summary.invalidate();
    }

    /// Pushes the model into the controls, decoding artwork through the cache.
    ///
    /// The artwork is polled every tick (a decode may finish between model
    /// changes); the summary and queue are only rebuilt when the model's
    /// revision changed.
    pub(super) fn sync(&mut self, model: &Model) {
        let mut sink = Win32ImageSink;
        self.cache.drain(&mut sink);
        let request = model.artwork();
        let bitmap = if request.path.is_empty() {
            None
        } else {
            self.cache
                .get_with_fallback(
                    &mut sink,
                    &request.path,
                    request.fallback_dir.as_deref().map(Path::new),
                )
                .cloned()
                .flatten()
        };
        let mut changed = self.summary.widget().borrow_mut().set_artwork(bitmap);

        if model.revision() != self.applied_revision {
            self.applied_revision = model.revision();
            self.summary.widget().borrow_mut().set_model(model);
            self.rebuild_queue(model);
            changed = true;
        }

        if changed {
            self.summary.invalidate();
        }
    }

    /// Replaces the queue model and its row-to-entry index map.
    fn rebuild_queue(&mut self, model: &Model) {
        self.indices = model.queue().iter().map(entry_index).collect();
        self.queue.set_model(queue::model(model));
    }

    /// The queue entry index for a preview row, for jump/remove.
    #[must_use]
    pub(super) fn queue_index(&self, row: usize) -> Option<usize> {
        self.indices.as_slice().get(row).copied()
    }

    /// Remembers the row a context menu was opened on.
    pub(super) fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// The queue entry index of the row a context menu was opened on.
    #[must_use]
    pub(super) fn context_index(&self) -> Option<usize> {
        self.context_row.get().and_then(|row| self.queue_index(row))
    }

    /// The queue's right-click menu.
    #[must_use]
    pub(super) fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }
}

/// The full-queue index of a preview row.
fn entry_index(row: &QueueRow) -> usize {
    row.index
}
