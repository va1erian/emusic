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

pub(crate) mod artwork;
mod central;
mod pair;
mod queue;
mod summary;

use std::cell::Cell;

use emusic_ui::state::projectm::{VizCommand, VizDock};
use emusic_ui::views::now_playing::NowPlayingView as Model;
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;
use win32ui::{column, dip};

use crate::app::Msg;
use crate::views::projectm::{ProjectMGesture, ProjectMView};

use pair::WidgetPair;

/// Width of the right panel, in device-independent pixels.
pub const PANEL_WIDTH: f32 = 280.0;
/// Height of the projectM section, in device-independent pixels: 4:3 at
/// [`PANEL_WIDTH`].
pub const VIZ_HEIGHT: f32 = PANEL_WIDTH * 3.0 / 4.0;

pub use central::CentralNowPlayingView;
pub use summary::SummaryEvent;

/// The Win32 now-playing panel: the projectM section between the summary and
/// the queue list.
pub struct NowPlayingView {
    pair: WidgetPair,
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
    /// double-click route through [`Msg::Viz`].
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> win32ui::Result<Self> {
        let pair = WidgetPair::new(
            ui,
            waker,
            true,
            |row| Some(Msg::QueueJump(row)),
            |row| Some(Msg::QueueContext(row)),
            || Msg::QueueRemove,
        )?;
        let viz = ProjectMView::new(ui)?.with_gestures(|gesture| match gesture {
            ProjectMGesture::PopOut => Some(Msg::Viz(VizCommand::SetDock(VizDock::Window))),
            ProjectMGesture::Fullscreen | ProjectMGesture::DoubleClick => {
                Some(Msg::Viz(VizCommand::SetFullscreen(true)))
            }
            ProjectMGesture::Hide => Some(Msg::Viz(VizCommand::SetVisible(false))),
            ProjectMGesture::ContextMenu => Some(Msg::VizMenu),
        });
        viz.set_visible(false);
        Ok(Self {
            pair,
            viz,
            viz_visible: Cell::new(false),
        })
    }

    /// The panel as a layout column: the summary fills the space above the
    /// projectM section (when shown) and the fixed-height queue list.
    #[must_use]
    pub fn layout(&self) -> Layout {
        column![
            self.pair.summary.fill(1),
            self.viz.height(dip(viz_row_height(self.viz_visible.get()))),
            self.pair.queue.height(dip(queue::QUEUE_HEIGHT)),
        ]
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

    /// Shows or hides the whole panel (both native controls).
    pub fn set_visible(&self, visible: bool) {
        self.pair.set_visible(visible);
    }

    /// Shows or hides just the upcoming "next tracks" queue list, leaving the
    /// summary and visualization in place.
    pub fn set_queue_visible(&self, visible: bool) {
        self.pair.set_queue_visible(visible);
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

    /// The queue's right-click menu.
    #[must_use]
    pub fn context_menu(&self) -> &Menu<Msg> {
        self.pair.context_menu()
    }
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
}
