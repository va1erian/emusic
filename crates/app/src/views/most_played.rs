//! The Most Played view (#245), ported to the portable widget layer: a
//! time-window tab strip above the shared track table ranked by completed play
//! count.
//!
//! The window and the table's sort/selection live in `emusic-ui`'s
//! [`MostPlayedState`](emusic_ui::views::most_played::MostPlayedState); the
//! ranking itself is queried per window from the backend. This module only
//! owns the controls and hands the ranked tracks to the reusable [`TrackView`],
//! as Music/Folders/Starred do.

use std::cell::Cell;

use emusic_ui::library_api::{LibraryDataSource, StatsWindow, TrackInfo};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Ctx;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label, Tabs};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// Height of the window-tab band, in design units (matches the tab strip).
const HEADER_HEIGHT: f32 = 32.0;
/// Height of the "Top N tracks" label, in design units.
const LABEL_HEIGHT: f32 = 20.0;

/// The Most Played view: the window tabs and the ranked table.
pub struct MostPlayedView {
    ui: Ui<Msg>,
    tabs: Tabs<Msg>,
    count: Label<Msg>,
    table: TrackView,
    /// The window and table revision the controls were last built from.
    applied: Cell<(Option<StatsWindow>, u64)>,
}

impl MostPlayedView {
    /// Creates the tab strip, the count label and the (empty) track table.
    pub fn new(ui: &Ui<Msg>) -> MostPlayedView {
        let tabs = Tabs::new(ui, Rect::default())
            .expect("create most-played tabs")
            .page(StatsWindow::AllTime.label(), &[])
            .page(StatsWindow::Last30Days.label(), &[])
            .page(StatsWindow::LastYear.label(), &[])
            .on_change(|index| StatsWindow::ALL.get(index).copied().map(Msg::MostPlayed));
        let count = Label::new(
            ui,
            Rect::default(),
            "No completed plays in this window yet.",
        )
        .expect("create most-played count");
        MostPlayedView {
            ui: ui.clone(),
            tabs,
            count,
            table: TrackView::new(ui),
            applied: Cell::new((None, u64::MAX)),
        }
    }

    /// Moves and sizes the whole view: the tab strip on top, the count label
    /// below it, the table filling the rest.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let label = dip(LABEL_HEIGHT).to_px(dpi).value();
        self.tabs.set_bounds(Rect::new(
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.top + header,
        ));
        self.ui.apply_moves(&[(
            self.count.id(),
            Rect::new(
                bounds.left,
                bounds.top + header,
                bounds.right,
                bounds.top + header + label,
            ),
        )]);
        self.table.set_bounds(Rect::new(
            bounds.left,
            bounds.top + header + label,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the whole view (its tabs, count label and table).
    pub fn set_visible(&self, visible: bool) {
        self.tabs.set_visible(visible);
        self.ui.set_visible(self.count.id(), visible);
        self.table.set_visible(visible);
    }

    /// Refreshes the shared table model from the library's ranking for the
    /// selected window, and rebuilds the controls when the window or the
    /// ranking changed.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
    ) {
        self.tabs.select(window_index(state.most_played.window));

        let ranked = library.most_played(state.most_played.window);
        let tracks: Vec<&TrackInfo> = ranked.iter().collect();
        state
            .most_played
            .table
            .refresh(&Ctx::new(&tracks, playing_id));

        let applied = (
            Some(state.most_played.window),
            state.most_played.table.revision(),
        );
        if self.applied.get() != applied {
            self.applied.set(applied);
            self.count.set_text(&count_label(&tracks));
            self.table.set_rows(&tracks, state.most_played.table.sort);
        }
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort.
    pub fn resort(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        let ranked = library.most_played(state.most_played.window);
        let tracks: Vec<&TrackInfo> = ranked.iter().collect();
        self.table.set_rows(&tracks, state.most_played.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index`.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.table.toggle_star(index)
    }

    /// The track at `index`, for a context action.
    pub fn track(&self, index: usize) -> Option<TrackInfo> {
        self.table.track(index).cloned()
    }
}

/// The index of `window` in [`StatsWindow::ALL`], for [`Tabs::select`].
fn window_index(window: StatsWindow) -> usize {
    StatsWindow::ALL
        .iter()
        .position(|candidate| *candidate == window)
        .unwrap_or(0)
}

/// The label above the table: the number of ranked tracks, or the empty hint.
fn count_label(tracks: &[&TrackInfo]) -> String {
    if tracks.is_empty() {
        "No completed plays in this window yet.".to_string()
    } else {
        format!("Top {} tracks", tracks.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_label_pluralizes_the_ranking_size() {
        assert_eq!(count_label(&[]), "No completed plays in this window yet.");
        let track = TrackInfo::default();
        assert_eq!(count_label(&[&track]), "Top 1 tracks");
    }

    #[test]
    fn window_index_matches_the_strip_order() {
        assert_eq!(window_index(StatsWindow::AllTime), 0);
        assert_eq!(window_index(StatsWindow::Last30Days), 1);
        assert_eq!(window_index(StatsWindow::LastYear), 2);
    }
}
