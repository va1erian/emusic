//! Virtualized, sortable track table (#15): the shared widget backing the
//! Music view today, and later the album/artist/genre/folder/history views
//! once they need to show a list of tracks too.
//!
//! Rows are virtualized via `egui_extras::TableBuilder` (only visible rows
//! are laid out/painted), so it stays smooth even at large library sizes.
//!
//! Double-click, Enter and the context menu's Play/Play next/Add to queue
//! surface as [`TrackAction`]s that the caller turns into
//! [`crate::state::Command`]s, applied to the real player by
//! `app::apply_player_command`. `TrackAction::Play` carries the table's
//! current visible/sorted order alongside the clicked id (#134), so the
//! resulting command can replace the queue with the whole list rather than
//! just the one track. The context menu's Copy path / Open file location /
//! Properties are handled directly by the table (#136).

pub(crate) mod columns;
mod context_menu;
mod properties;
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
    /// The track whose Properties dialog is open, if any. Owned here (rather
    /// than by the shell) so each embedding table gets its own dialog; the
    /// dialog is rendered by [`show`] itself.
    pub properties: Option<TrackInfo>,
}

/// A playback action requested from a row this frame (double-click, Enter,
/// or a context-menu item), to be turned into a `Command` by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackAction {
    /// `context` is the table's current visible/sorted track ids (#134), so
    /// the caller can hand it straight to [`crate::state::Command::play_track`]
    /// and every embedding view gets a correctly populated queue for free.
    Play {
        id: u64,
        context: Vec<u64>,
    },
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
        action = Some(TrackAction::Play {
            id: tracks[track_idx].id,
            context: order_ids.clone(),
        });
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
                ui.add(egui::Label::new(egui::RichText::new("#").weak()).selectable(false));
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
                    ui.add(
                        egui::Label::new(egui::RichText::new((pos + 1).to_string()).weak())
                            .selectable(false),
                    );
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
                    action = Some(TrackAction::Play {
                        id: track.id,
                        context: order_ids.clone(),
                    });
                }
                if let Some(context_action) = context_menu::show(&response, track) {
                    match context_action {
                        ContextAction::Play => {
                            action = Some(TrackAction::Play {
                                id: track.id,
                                context: order_ids.clone(),
                            });
                        }
                        ContextAction::PlayNext => action = Some(TrackAction::PlayNext(track.id)),
                        ContextAction::AddToQueue => {
                            action = Some(TrackAction::AddToQueue(track.id));
                        }
                        ContextAction::Properties => state.properties = Some(track.clone()),
                    }
                }
            });
        });

    properties::show(ui.ctx(), &mut state.properties);

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
