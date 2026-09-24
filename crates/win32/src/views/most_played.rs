//! Win32 Most Played view (#245): a time-window selector above the shared
//! track table ranked by completed play count.
//!
//! The window and the table's sort/selection live in `emusic-ui`'s
//! [`MostPlayedState`](emusic_ui::views::most_played::MostPlayedState); the
//! ranking itself is queried per window from the backend. This module only
//! owns the native controls and hands the ranked tracks to the reusable
//! [`TrackView`], as Music/Folders/Starred do.

use std::cell::Cell;

use emusic_ui::library_api::{LibraryDataSource, StatsWindow, TrackInfo};
use emusic_ui::state::AppState;
use emusic_ui::views::Ctx;
use win32ui::prelude::*;
use win32ui::{Control, Label, Menu, RadioGroup, column, dip, row};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// Height of the window-selector band, in design units.
const HEADER_HEIGHT: f32 = 30.0;
/// Width of the "Window" label, in design units.
const LABEL_WIDTH: f32 = 56.0;
/// Height of the "Top N tracks" label, in design units.
const LABEL_HEIGHT: f32 = 20.0;

/// The Win32 Most Played view: the window selector and the ranked table.
pub struct MostPlayedView {
    window_label: Label,
    window: RadioGroup<StatsWindow, Msg>,
    count: Label,
    table: TrackView,
    /// The window and table revision the controls were last built from.
    applied: Cell<(Option<StatsWindow>, u64)>,
}

impl MostPlayedView {
    /// Creates the selector, the count label and the (empty) track table.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let window = RadioGroup::new(ui, StatsWindow::ALL.map(|window| (window.label(), window)))?
            .selected(StatsWindow::AllTime)
            .on_select(|window| Some(Msg::MostPlayed(*window)));
        Ok(Self {
            window_label: Label::new(ui, Rect::default(), "Window")?,
            window,
            count: Label::new(
                ui,
                Rect::default(),
                "No completed plays in this window yet.",
            )?,
            table: TrackView::new(ui)?,
            applied: Cell::new((None, u64::MAX)),
        })
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
        if self.window.selected_value() != Some(state.most_played.window) {
            self.window.set_selected(&state.most_played.window);
        }

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
    pub fn activate(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.activate(index)
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(
        &self,
        action: crate::views::track_table::ContextAction,
        hwnd: win32ui::Hwnd,
    ) -> Option<emusic_ui::state::Command> {
        self.table.run_context(action, hwnd)
    }

    /// Remembers the row whose context menu was opened, so `run_context` can
    /// act on it.
    pub fn set_context_row(&self, row: usize) {
        self.table.set_context_row(row);
    }

    /// The track table's context menu.
    pub fn context_menu(&self) -> &Menu<Msg> {
        self.table.context_menu()
    }

    /// Shows or hides the whole view (its selector, count label and table).
    pub fn set_visible(&self, visible: bool) {
        self.window_label.set_visible(visible);
        self.window.set_visible(visible);
        self.count.set_visible(visible);
        self.table.set_visible(visible);
    }

    /// The window-selector band and count label above the track table.
    pub fn layout(&self) -> Layout {
        column![
            row![
                self.window_label.width(dip(LABEL_WIDTH)),
                window_row(&self.window),
            ]
            .height(dip(HEADER_HEIGHT)),
            self.count.height(dip(LABEL_HEIGHT)),
            self.table.fill(1),
        ]
    }
}

impl AsControl for MostPlayedView {
    fn control(&self) -> &Control {
        self.table.control()
    }
}

/// Lays the window options out in a single horizontal row.
fn window_row(group: &RadioGroup<StatsWindow, Msg>) -> Layout {
    let mut row = Layout::row().spacing(dip(16.0));
    for option in group.options() {
        row = row.item(option);
    }
    row
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
}
