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
//! A [`splitter::QueueSplitter`] sits above the queue list (#514): dragging it
//! resizes the queue (clamped by [`emusic_ui::state::drag_queue_height`]) and
//! double-clicking it restores the default height. The app keeps the preferred
//! height in [`emusic_ui::state::AppState::queue_height`]; the panel lays out
//! [`emusic_ui::state::displayed_queue_height`] of it, refitted whenever the
//! summary is resized ([`Msg::QueueFit`]), so a small window never squeezes
//! the summary below its minimum.
//!
//! No progress bar: the top-bar transport (#110's `top_bar`) already shows
//! elapsed/total time with a seek slider, so neither surface repeats it.

pub(crate) mod artwork;
mod central;
mod pair;
mod queue;
mod splitter;
mod summary;

use std::cell::Cell;

use emusic_ui::state::{DEFAULT_QUEUE_HEIGHT, displayed_queue_height, drag_queue_height};
use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;
use win32ui::{Dip, Px, Rect};
use win32ui::{column, dip};

use crate::app::Msg;
use crate::views::projectm::{ProjectMView, panel_gesture};

use pair::WidgetPair;
use splitter::{QueueSplitter, SPLITTER_HEIGHT};

/// Width of the right panel, in device-independent pixels.
pub const PANEL_WIDTH: f32 = 280.0;
/// Height of the projectM section, in device-independent pixels: 4:3 at
/// [`PANEL_WIDTH`].
pub const VIZ_HEIGHT: f32 = PANEL_WIDTH * 3.0 / 4.0;

pub use central::CentralNowPlayingView;
pub use splitter::SplitterEvent;
pub use summary::SummaryEvent;

/// The Win32 now-playing panel: the projectM section between the summary and
/// the queue list, and the splitter that resizes the queue.
pub struct NowPlayingView {
    pair: WidgetPair,
    /// The drag handle above the queue list (#514).
    splitter: Custom<QueueSplitter, Msg>,
    /// The queue list's displayed height, in DIP: the preference fitted to the
    /// panel. The layout reads it, so a change needs a relayout.
    queue_height: Cell<f32>,
    /// The single projectM surface; only this panel hosts one (#302).
    viz: ProjectMView,
    /// Whether the projectM row is expanded. The layout reads it, so a change
    /// needs a relayout.
    viz_visible: Cell<bool>,
}

impl NowPlayingView {
    /// Creates the summary widget, the projectM surface and the queue list, and
    /// builds the artwork cache woken by `waker`. The panel's queue routes
    /// jump/context/remove through [`Msg::QueueJump`], [`Msg::QueueContext`]
    /// and [`Msg::QueueRemove`]; the visualization's hover buttons and
    /// double-click route through [`Msg::Viz`], and the queue splitter's
    /// gestures through [`Msg::QueueSplitter`]. The queue starts at its
    /// default height; the app restores the saved one with
    /// [`Self::set_queue_height`]. Every resize of the summary raises
    /// [`Msg::QueueFit`] so the app can refit the queue to the panel.
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> win32ui::Result<Self> {
        let pair = WidgetPair::new(
            ui,
            waker,
            true,
            |row| Some(Msg::QueueJump(row)),
            |row| Some(Msg::QueueContext(row)),
            || Msg::QueueRemove,
            || Msg::QueueRemoveSelected,
        )?;
        let viz = ProjectMView::new(ui)?.with_gestures(panel_gesture);
        viz.set_visible(false);
        let proxy = ui.proxy();
        pair.summary.on_resize(move |_| {
            let _ = proxy.send(Msg::QueueFit);
        });
        let splitter = Custom::new(ui, QueueSplitter::new(DEFAULT_QUEUE_HEIGHT))?
            .on_event(|event| Some(Msg::QueueSplitter(event)));
        Ok(Self {
            pair,
            splitter,
            queue_height: Cell::new(DEFAULT_QUEUE_HEIGHT),
            viz,
            viz_visible: Cell::new(false),
        })
    }

    /// The panel as a layout column: the summary fills the space above the
    /// projectM section (when shown), the splitter and the queue list at its
    /// current height.
    #[must_use]
    pub fn layout(&self) -> Layout {
        column![
            self.pair.summary.fill(1),
            self.viz.height(dip(viz_row_height(self.viz_visible.get()))),
            self.splitter.height(dip(SPLITTER_HEIGHT)),
            self.pair.queue.height(dip(self.queue_height.get())),
        ]
    }

    /// Sets the queue list's displayed height, in DIP; the caller relayouts.
    pub fn set_queue_height(&self, height: f32) {
        self.queue_height.set(height);
        self.splitter.widget().borrow().set_queue_height(height);
    }

    /// Fits the displayed queue height to the panel for the user's
    /// `preferred` height. Returns whether it changed, in which case the
    /// caller relayouts. Does nothing until the panel has been laid out with
    /// its queue visible.
    pub fn fit_queue_height(&self, preferred: f32, dpi: u32) -> bool {
        let Some(span) = self.queue_span(dpi) else {
            return false;
        };
        let height = displayed_queue_height(preferred, span);
        if (height - self.queue_height.get()).abs() < 0.5 {
            return false;
        }
        self.set_queue_height(height);
        true
    }

    /// The queue height a splitter drag by `delta` leads to: the displayed
    /// height moved by the drag, clamped so the summary and the queue both
    /// keep their minimum of the height they share. `dpi` is the window's.
    #[must_use]
    pub fn dragged_queue_height(&self, delta: Dip, dpi: u32) -> f32 {
        let current = self.queue_height.get();
        let span = self.queue_span(dpi).unwrap_or(f32::INFINITY);
        drag_queue_height(current, delta.value(), span)
    }

    /// The height the summary and the queue share, in DIP: the whole panel
    /// column minus the projectM row and the splitter at their nominal sizes.
    /// Measured from the summary's top to the queue's bottom, so it is exact
    /// even when the column overflowed and the layout shrank its rows. `None`
    /// while the queue is hidden or not laid out yet.
    fn queue_span(&self, dpi: u32) -> Option<f32> {
        if !self.pair.queue.is_visible() {
            return None;
        }
        let column = panel_column(self.pair.summary.bounds(), self.pair.queue.bounds())?;
        let column = Px(column).to_dip(dpi).value();
        Some(column - viz_row_height(self.viz_visible.get()) - SPLITTER_HEIGHT)
    }

    /// Expands or collapses the projectM row; the caller relayouts.
    pub fn set_viz_visible(&self, visible: bool) {
        if self.viz_visible.replace(visible) == visible {
            return;
        }
        self.viz.set_visible(visible);
    }

    /// The panel's projectM surface, for the shell to feed and poll.
    #[must_use]
    pub fn viz(&self) -> &ProjectMView {
        &self.viz
    }

    /// Shows or hides the whole panel (its native controls and the splitter).
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
        self.splitter.set_visible(visible);
    }

    /// Shows or hides just the upcoming "next tracks" queue list and its
    /// splitter, leaving the summary and visualization in place.
    pub fn set_queue_visible(&self, visible: bool) {
        self.pair.set_queue_visible(visible);
        self.splitter.set_visible(visible);
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

/// The panel column's height in pixels, from the summary's top edge to the
/// queue's bottom edge, or `None` before the two have been laid out.
fn panel_column(summary: Rect, queue: Rect) -> Option<i32> {
    let height = queue.bottom - summary.top;
    (height > 0 && queue.bottom > queue.top).then_some(height)
}

/// The projectM row height for a panel that is expanded or collapsed: the
/// 4:3 [`VIZ_HEIGHT`], or zero while the visualization is hidden.
fn viz_row_height(visible: bool) -> f32 {
    if visible { VIZ_HEIGHT } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projectm_row_is_4_3_when_expanded_and_collapses_to_zero() {
        assert_eq!(viz_row_height(true), 210.0);
        assert_eq!(viz_row_height(false), 0.0);
    }

    #[test]
    fn the_panel_column_spans_summary_top_to_queue_bottom() {
        let summary = Rect::new(0, 100, 280, 600);
        let queue = Rect::new(0, 605, 280, 805);
        assert_eq!(panel_column(summary, queue), Some(705));
        assert_eq!(panel_column(summary, Rect::default()), None);
    }
}
