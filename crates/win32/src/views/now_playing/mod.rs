//! Win32 now-playing right panel (#110).
//!
//! A view struct per the #106 pattern: it owns two native controls — an
//! owner-drawn [`Custom`] summary (artwork, metadata label/value rows and
//! tracker-module info) and a virtual [`ListView`] over the upcoming queue —
//! and keeps them in step with the shared
//! [`emusic_ui::views::now_playing::NowPlayingView`] model. All display data
//! and user intents come from the model; this module only draws and routes.
//! The widgets themselves live in [`pair::WidgetPair`], shared with the
//! full-width central view (#247, [`central::CentralNowPlayingView`]) so
//! neither surface duplicates the owner-drawn summary or the queue list.
//!
//! The panel is placed as a nested layout column, so [`NowPlayingView::layout`]
//! returns the item the window's layout tree installs.
//!
//! No progress bar: the top-bar transport (#110's `top_bar`) already shows
//! elapsed/total time with a seek slider, so neither surface repeats it.

mod artwork;
mod central;
mod pair;
mod queue;
mod summary;

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;
use win32ui::{column, dip};

use crate::app::Msg;

use pair::WidgetPair;

/// Width of the right panel, in device-independent pixels.
pub const PANEL_WIDTH: f32 = 280.0;

pub use central::CentralNowPlayingView;
pub use summary::SummaryEvent;

/// The Win32 now-playing panel: the summary widget plus the queue list.
pub struct NowPlayingView {
    pair: WidgetPair,
}

impl NowPlayingView {
    /// Creates the summary widget and the queue list, and builds the artwork
    /// cache woken by `waker`. The panel's queue routes jump/context/remove
    /// through [`Msg::QueueJump`], [`Msg::QueueContext`] and
    /// [`Msg::QueueRemove`].
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> win32ui::Result<Self> {
        let pair = WidgetPair::new(
            ui,
            waker,
            |row| Some(Msg::QueueJump(row)),
            |row| Some(Msg::QueueContext(row)),
            || Msg::QueueRemove,
        )?;
        Ok(Self { pair })
    }

    /// The panel as a layout column: the summary fills the space above a fixed
    /// height queue list.
    #[must_use]
    pub fn layout(&self) -> Layout {
        column![
            self.pair.summary.fill(1),
            self.pair.queue.height(dip(queue::QUEUE_HEIGHT)),
        ]
    }

    /// Shows or hides the whole panel (both native controls).
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
    }

    /// Pushes the model into the controls, decoding artwork through the cache.
    pub fn sync(&mut self, model: &Model, dpi: u32) {
        self.pair.sync(model, dpi);
    }

    /// The queue entry index for a preview row, for jump/remove.
    #[must_use]
    pub fn queue_index(&self, row: usize) -> Option<usize> {
        self.pair.queue_index(row)
    }

    /// Remembers the row a context menu was opened on.
    pub fn set_context_row(&self, row: usize) {
        self.pair.set_context_row(row);
    }

    /// The queue entry index of the row a context menu was opened on.
    #[must_use]
    pub fn context_index(&self) -> Option<usize> {
        self.pair.context_index()
    }

    /// The queue's right-click menu.
    #[must_use]
    pub fn context_menu(&self) -> &Menu<Msg> {
        self.pair.context_menu()
    }
}
