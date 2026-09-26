//! Win32 Now Playing central view (#247): the `View::NowPlaying` navigator
//! entry's full-width counterpart to the right panel.
//!
//! It reuses the same [`pair::WidgetPair`] — the owner-drawn summary and the
//! queue list — as the right panel, laid out full width instead of at
//! [`super::PANEL_WIDTH`]. Both surfaces stay in sync because both are
//! `sync`ed every tick from the one shared
//! [`emusic_ui::views::now_playing::NowPlayingView`] model; the right panel
//! keeps showing while this view is active (it is not hidden as redundant):
//! the right-hand panel and the central Now Playing view both render from the
//! same model at once.
//!
//! Like the right panel, this view has no progress bar: the top-bar
//! transport already shows elapsed/total time with a seek slider.

use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use xui::prelude::*;
use xui::{column, dip};

use crate::app::Msg;

use super::pair::WidgetPair;

/// Height of the queue list in the central view, in device-independent
/// pixels. Taller than the right panel's since the central area has more
/// vertical room.
const QUEUE_HEIGHT: f32 = 320.0;

/// The Win32 Now Playing central view: the summary widget above a full-width
/// queue list.
pub struct CentralNowPlayingView {
    pair: WidgetPair,
}

impl CentralNowPlayingView {
    /// Creates the summary widget and the queue list, and builds the artwork
    /// cache woken by `waker`. This view's queue routes jump/context/remove
    /// through [`Msg::CentralQueueJump`], [`Msg::CentralQueueContext`] and
    /// [`Msg::CentralQueueRemove`], distinct from the right panel's, so the
    /// two lists (both visible at once) can be told apart.
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> xui::Result<Self> {
        let pair = WidgetPair::new(
            ui,
            waker,
            false,
            |row| Some(Msg::CentralQueueJump(row)),
            |row| Some(Msg::CentralQueueContext(row)),
            || Msg::CentralQueueRemove,
            || Msg::CentralQueueRemoveSelected,
        )?;
        Ok(Self { pair })
    }

    /// The view as a layout column: the summary fills the space above a fixed
    /// height queue list, both full width.
    #[must_use]
    pub fn layout(&self) -> Layout {
        column![
            self.pair.summary.fill(1),
            self.pair.queue.height(dip(QUEUE_HEIGHT)),
        ]
    }

    /// Shows or hides the whole view (both native controls).
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        self.pair.apply_appearance();
    }

    /// Pushes the model into the controls, decoding artwork through the cache.
    pub fn sync(&mut self, model: &Model) {
        self.pair.sync(model);
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

    /// The queue entry index of the currently selected row, for
    /// Delete-to-remove.
    #[must_use]
    pub fn selected_queue_index(&self) -> Option<usize> {
        self.pair.selected_queue_index()
    }

    /// The queue's right-click menu.
    #[must_use]
    pub fn context_menu(&self) -> &Menu<Msg> {
        self.pair.context_menu()
    }
}
