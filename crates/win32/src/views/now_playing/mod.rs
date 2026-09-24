//! Win32 now-playing right panel (#110).
//!
//! A view struct per the #106 pattern: it owns two native controls — an
//! owner-drawn [`Custom`] summary (artwork, metadata label/value rows and
//! tracker-module info) and a virtual [`ListView`] over the upcoming queue —
//! and keeps them in step with the shared
//! [`emusic_ui::views::now_playing::NowPlayingView`] model. All display data
//! and user intents come from the model; this module only draws and routes.
//!
//! The panel is placed as a nested layout column, so [`NowPlayingView::layout`]
//! returns the item the window's layout tree installs.

mod artwork;
mod queue;
mod summary;

use std::cell::Cell;
use std::path::Path;

use emusic_ui::views::now_playing::{NowPlayingView as Model, QueueRow};
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;
use win32ui::{column, dip};

use crate::app::Msg;

use artwork::{ArtworkCache, Win32ImageSink};
use queue::QueueItem;

/// Width of the right panel, in device-independent pixels.
pub const PANEL_WIDTH: f32 = 280.0;

pub use summary::SummaryEvent;

/// The Win32 now-playing panel: the summary widget plus the queue list.
pub struct NowPlayingView {
    summary: Custom<summary::SummaryWidget, Msg>,
    queue: ListView<QueueItem, Msg>,
    cache: ArtworkCache,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
    /// Queue entry index for each preview row, for jump/remove.
    indices: Vec<usize>,
    applied_revision: u64,
}

impl NowPlayingView {
    /// Creates the summary widget and the queue list, and builds the artwork
    /// cache woken by `waker`.
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> win32ui::Result<Self> {
        let summary = Custom::new(ui, summary::SummaryWidget::new(ui.dpi()))?
            .on_event(|event| Some(Msg::NowPlaying(event)));
        let queue = queue::build(ui)?;
        Ok(Self {
            summary,
            queue,
            cache: artwork::new_cache(waker),
            context: queue::context_menu(),
            context_row: Cell::new(None),
            indices: Vec::new(),
            applied_revision: u64::MAX,
        })
    }

    /// The panel as a layout column: the summary fills the space above a fixed
    /// height queue list.
    #[must_use]
    pub fn layout(&self) -> Layout {
        column![
            self.summary.fill(1),
            self.queue.height(dip(queue::QUEUE_HEIGHT)),
        ]
    }

    /// Shows or hides the whole panel (both native controls).
    pub fn set_visible(&self, visible: bool) {
        self.summary.set_visible(visible);
        self.queue.set_visible(visible);
    }

    /// Pushes the model into the controls, decoding artwork through the cache.
    ///
    /// The artwork is polled every tick (a decode may finish between model
    /// changes); the summary and queue are only rebuilt when the model's
    /// revision changed.
    pub fn sync(&mut self, model: &Model, dpi: u32) {
        let mut sink = Win32ImageSink::new(summary::artwork_edge_px(dpi));
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
    pub fn queue_index(&self, row: usize) -> Option<usize> {
        self.indices.as_slice().get(row).copied()
    }

    /// Remembers the row a context menu was opened on.
    pub fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// The queue entry index of the row a context menu was opened on.
    #[must_use]
    pub fn context_index(&self) -> Option<usize> {
        self.context_row.get().and_then(|row| self.queue_index(row))
    }

    /// The queue's right-click menu.
    #[must_use]
    pub fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }
}

/// The full-queue index of a preview row.
fn entry_index(row: &QueueRow) -> usize {
    row.index
}
