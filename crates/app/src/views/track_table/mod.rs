//! Virtualized, sortable track table (#15): the shared widget backing the
//! Music view today, and later the album/artist/genre/folder/history views
//! once they need to show a list of tracks too.
//!
//! Rows are virtualized via `egui_extras::TableBuilder` (only visible rows
//! are laid out/painted), so it stays smooth even at large library sizes.
//!
//! Playback isn't wired up yet: [`PlayerApi`](crate::player_api::PlayerApi)
//! has no way to load an arbitrary track or replace its queue (that lands
//! with the real player, #4). Double-click, Enter and the context menu's
//! Play/Play next/Add to queue therefore surface as [`TrackAction`]s that
//! the caller turns into [`crate::state::Command`]s; those commands are
//! currently no-ops in the shell (see `app::apply_player_command`) until #4
//! lands. Selection, sorting, keyboard navigation, resizing and the context
//! menu's Copy path / Open file location are fully functional today.

mod columns;
mod context_menu;
mod selection;
mod sort;

use std::collections::HashSet;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

pub use context_menu::ContextAction;
pub use selection::ClickModifiers;
pub use sort::SortState;

use crate::library_api::TrackInfo;

/// Persistent per-instance state (sort order + selection). Each embedding
/// view owns one of these across frames.
#[derive(Debug, Default)]
pub struct TrackTableState {
    pub sort: SortState,
    pub selection: selection::SelectionState,
}

/// A playback action requested from a row this frame (double-click, Enter,
/// or a context-menu item), to be turned into a `Command` by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackAction {
    Play(u64),
    PlayNext(u64),
    AddToQueue(u64),
}

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const INDEX_COL_WIDTH: f32 = 34.0;

/// Placeholder spec for the Title column, which is sized separately (it
/// fills the remaining space rather than taking a fixed initial width) but
/// still needs a label/id to share the header-click-to-sort code path.
const TITLE_COLUMN: columns::ColumnSpec = columns::ColumnSpec {
    id: columns::ColumnId::Title,
    label: "Title",
    initial_width: 0.0,
    min_width: columns::TITLE_MIN_WIDTH,
};

/// Renders the table for `tracks` (already filtered by the caller, e.g. by
/// search query) and returns any playback action requested this frame.
///
/// `id_salt` must be unique per embedding view so multiple tables on screen
/// (or the same view shown twice, as `emusic-shot --all` does) don't share
/// column-resize/scroll state.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    state: &mut TrackTableState,
    tracks: &[&TrackInfo],
    playing_id: Option<u64>,
) -> Option<TrackAction> {
    let visible_ids: HashSet<u64> = tracks.iter().map(|t| t.id).collect();
    state.selection.retain_existing(&visible_ids);

    let order = sort::sorted_indices(tracks, state.sort);
    let order_ids: Vec<u64> = order.iter().map(|&i| tracks[i].id).collect();

    // Arrow-key navigation / Enter-to-play, but only when no other widget
    // (e.g. the top bar's search box) currently holds keyboard focus, so we
    // don't steal cursor movement from a text field.
    let any_widget_focused = ui.memory(|m| m.focused().is_some());
    let (arrow_delta, enter_pressed) = ui.input(|i| {
        let delta = if i.key_pressed(egui::Key::ArrowDown) {
            1
        } else if i.key_pressed(egui::Key::ArrowUp) {
            -1
        } else {
            0
        };
        (delta, i.key_pressed(egui::Key::Enter))
    });
    if !any_widget_focused && arrow_delta != 0 {
        state
            .selection
            .move_focus(&order_ids, arrow_delta, order_ids.len());
    }

    let mut action = None;
    if !any_widget_focused
        && enter_pressed
        && let Some(focus) = state.selection.focus
        && let Some(&track_idx) = order.get(focus)
    {
        action = Some(TrackAction::Play(tracks[track_idx].id));
    }

    let available_height = ui.available_height();
    let mut builder = TableBuilder::new(ui)
        .id_salt(id_salt)
        .striped(true)
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .column(Column::exact(INDEX_COL_WIDTH))
        .column(
            Column::remainder()
                .at_least(columns::TITLE_MIN_WIDTH)
                .clip(true),
        );
    for col in columns::COLUMNS {
        builder = builder.column(
            Column::initial(col.initial_width)
                .at_least(col.min_width)
                .resizable(true)
                .clip(true),
        );
    }

    builder
        .header(HEADER_HEIGHT, |mut header| {
            header.col(|ui| {
                ui.add(egui::Label::new(egui::RichText::new("#").weak()));
            });
            header.col(|ui| header_cell(ui, &mut state.sort, &TITLE_COLUMN));
            for col in columns::COLUMNS {
                header.col(|ui| header_cell(ui, &mut state.sort, col));
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, order.len(), |mut row| {
                let pos = row.index();
                let track = tracks[order[pos]];
                let is_playing = playing_id == Some(track.id);
                row.set_selected(state.selection.is_selected(track.id));

                row.col(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new((pos + 1).to_string()).weak(),
                    ));
                });
                row.col(|ui| columns::show_cell(ui, columns::ColumnId::Title, track, is_playing));
                for col in columns::COLUMNS {
                    row.col(|ui| columns::show_cell(ui, col.id, track, is_playing));
                }

                let response = row.response();
                if response.clicked() {
                    let modifiers = response.ctx.input(|i| ClickModifiers {
                        ctrl: i.modifiers.command,
                        shift: i.modifiers.shift,
                    });
                    state.selection.click(&order_ids, pos, track.id, modifiers);
                }
                if response.double_clicked() {
                    action = Some(TrackAction::Play(track.id));
                }
                if let Some(context_action) = context_menu::show(&response, track) {
                    action = Some(match context_action {
                        ContextAction::Play => TrackAction::Play(track.id),
                        ContextAction::PlayNext => TrackAction::PlayNext(track.id),
                        ContextAction::AddToQueue => TrackAction::AddToQueue(track.id),
                    });
                }
            });
        });

    action
}

fn header_cell(ui: &mut egui::Ui, sort: &mut SortState, col: &columns::ColumnSpec) {
    let text = match sort.indicator(col.id) {
        Some(arrow) => format!("{} {arrow}", col.label),
        None => col.label.to_string(),
    };
    let response = ui.add(
        egui::Label::new(egui::RichText::new(text).strong())
            .sense(egui::Sense::click())
            .selectable(false),
    );
    if response.clicked() {
        sort.toggle(col.id);
    }
    let _ = response.on_hover_cursor(egui::CursorIcon::PointingHand);
}
