//! Virtualized, sortable track table (#15): the shared widget backing the
//! Music view today, and later the album/artist/genre/folder/history views
//! once they need to show a list of tracks too.
//!
//! Rows are virtualized via `egui_extras::TableBuilder` (only visible rows
//! are laid out/painted), so it stays smooth even at large library sizes.
//!
//! Every row starts with a star toggle (#131): a filled star when starred,
//! an outline otherwise. Double-click, Enter and the context menu's
//! Play/Play next/Add to queue/Star-Unstar surface as [`TrackAction`]s that
//! the caller turns into
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
    /// Flip whether `id` is starred (#131), from its star column or the
    /// Star/Unstar context-menu entry.
    ToggleStar(u64),
    /// Open the tag editor for `id` (#172), from the context menu's "Edit
    /// tags…" entry.
    EditTags(u64),
}

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const INDEX_COL_WIDTH: f32 = 34.0;
const STAR_COL_WIDTH: f32 = 24.0;

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
    // The table lays its columns out to whatever width it is given and clips
    // any that don't fit. Give it at least the sum of the columns' minimum
    // widths, so a narrow window scrolls horizontally (the central view wraps
    // this in a `ScrollArea`) instead of crushing the columns away.
    let table_width = ui
        .available_width()
        .max(min_table_width(ui.spacing().item_spacing.x));
    ui.allocate_ui_with_layout(
        egui::vec2(table_width, available_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            let mut builder = TableBuilder::new(ui)
                .id_salt(id_salt)
                .striped(true)
                .sense(egui::Sense::click())
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height)
                .column(Column::exact(INDEX_COL_WIDTH))
                .column(Column::exact(STAR_COL_WIDTH))
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
                    header.col(|_ui| {});
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
                        let mut star_clicked = false;
                        row.col(|ui| {
                            if columns::star_cell(ui, track) {
                                star_clicked = true;
                                action = Some(TrackAction::ToggleStar(track.id));
                            }
                        });
                        row.col(|ui| {
                            columns::show_cell(ui, columns::ColumnId::Title, track, is_playing)
                        });
                        for col in columns::COLUMNS {
                            row.col(|ui| columns::show_cell(ui, col.id, track, is_playing));
                        }

                        // A click on the star is that cell's toggle, not a row
                        // selection or a double-click-to-play.
                        let response = row.response();
                        if !star_clicked && response.clicked() {
                            let modifiers = response.ctx.input(|i| ClickModifiers {
                                ctrl: i.modifiers.command,
                                shift: i.modifiers.shift,
                            });
                            state.selection.click(&order_ids, pos, track.id, modifiers);
                        }
                        if !star_clicked && response.double_clicked() {
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
                                ContextAction::PlayNext => {
                                    action = Some(TrackAction::PlayNext(track.id))
                                }
                                ContextAction::AddToQueue => {
                                    action = Some(TrackAction::AddToQueue(track.id));
                                }
                                ContextAction::ToggleStar => {
                                    action = Some(TrackAction::ToggleStar(track.id))
                                }
                                ContextAction::Properties => state.properties = Some(track.clone()),
                                ContextAction::EditTags => {
                                    action = Some(TrackAction::EditTags(track.id));
                                }
                            }
                        }
                    });
                });
        },
    );

    properties::show(ui.ctx(), &mut state.properties);

    action
}

/// Minimum width the table can be laid out at without clipping columns: the
/// `#`/star/title minimums, every data column's minimum, and the gaps
/// between them.
fn min_table_width(spacing: f32) -> f32 {
    let data: f32 = columns::COLUMNS.iter().map(|col| col.min_width).sum();
    let gaps = spacing * (columns::COLUMNS.len() as f32 + 2.0);
    INDEX_COL_WIDTH + STAR_COL_WIDTH + columns::TITLE_MIN_WIDTH + data + gaps
}

/// Renders the track Properties dialog for `track`, if any. Exposed so the
/// shell can show it from surfaces that don't embed a track table (e.g. the
/// now-playing panel's "Properties" link).
pub fn show_properties(ctx: &egui::Context, track: &mut Option<TrackInfo>) {
    properties::show(ctx, track);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_table_width_covers_every_column_minimum() {
        let sum = INDEX_COL_WIDTH
            + STAR_COL_WIDTH
            + columns::TITLE_MIN_WIDTH
            + columns::COLUMNS
                .iter()
                .map(|col| col.min_width)
                .sum::<f32>();
        // No spacing means exactly the sum of the columns' minimums...
        assert_eq!(min_table_width(0.0), sum);
        // ...and positive spacing only ever widens it.
        assert!(min_table_width(8.0) > sum);
    }
}
