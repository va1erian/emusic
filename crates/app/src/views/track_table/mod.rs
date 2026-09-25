//! egui renderer for the shared track table (#15, #98): the widget backing
//! the Music/Albums/Folders/Starred/Most-played views.
//!
//! All state and logic live in [`TrackTable`] (`emusic-ui`); this module only
//! draws it. Rows are virtualized via `egui_extras::TableBuilder` (only
//! visible rows are laid out/painted), so it stays smooth even at large
//! library sizes.
//!
//! Every row starts with a star toggle (#131): a filled star when starred, an
//! outline otherwise. Double-click, Enter and the context menu's
//! Play/Play next/Add to queue/Star-Unstar surface as [`TrackTableMsg`]s that
//! the model turns into [`crate::state::Command`]s. The context menu's Copy
//! path / Open file location are handled directly here (#136).

pub(crate) mod columns;
mod context_menu;
mod properties;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use emusic_ui::views::track_table::{ContextAction, KeyNav, NavKey, TrackTable, TrackTableMsg};
use emusic_ui::views::{Commands, Ctx};

pub use emusic_ui::views::track_table::selection::{ClickModifiers, SelectionState};
pub use emusic_ui::views::track_table::sort::SortState;

use super::EguiView;

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

impl EguiView for TrackTable {
    fn show(&mut self, ui: &mut egui::Ui, id_salt: &str, cx: &Ctx, out: &mut Commands) {
        self.refresh(cx);

        // Arrow-key navigation / Enter-to-play, but only when no other widget
        // (e.g. the top bar's search box) currently holds keyboard focus, so we
        // don't steal cursor movement from a text field.
        let any_widget_focused = ui.memory(|m| m.focused().is_some());
        if !any_widget_focused {
            let (nav, enter_pressed) = ui.input(|i| {
                let nav = if i.key_pressed(egui::Key::ArrowDown) {
                    Some(KeyNav::new(NavKey::Down))
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    Some(KeyNav::new(NavKey::Up))
                } else {
                    None
                };
                (nav, i.key_pressed(egui::Key::Enter))
            });
            if let Some(nav) = nav {
                self.update(TrackTableMsg::Nav(nav), cx, out);
            }
            if enter_pressed && let Some(focus) = self.selection.focus {
                self.update(TrackTableMsg::RowActivated(focus), cx, out);
            }
        }

        let available_height = ui.available_height();
        let metrics = crate::appearance::metrics();
        let zebra = crate::appearance::zebra();
        // The table lays its columns out to whatever width it is given and
        // clips any that don't fit. Give it at least the sum of the columns'
        // minimum widths, so a narrow window scrolls horizontally (the central
        // view wraps this in a `ScrollArea`) instead of crushing the columns
        // away.
        let table_width = ui
            .available_width()
            .max(min_table_width(ui.spacing().item_spacing.x));
        let row_count = self.len();
        // Messages produced while drawing are applied afterwards, so the body
        // can borrow `self` immutably (one message per event, Elm-style).
        let mut messages: Vec<TrackTableMsg> = Vec::new();

        ui.allocate_ui_with_layout(
            egui::vec2(table_width, available_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let mut builder = TableBuilder::new(ui)
                    .id_salt(id_salt)
                    .striped(zebra)
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
                            ui.add(
                                egui::Label::new(egui::RichText::new("#").weak()).selectable(false),
                            );
                        });
                        header.col(|_ui| {});
                        header.col(|ui| header_cell(ui, &self.sort, &TITLE_COLUMN, &mut messages));
                        for col in columns::COLUMNS {
                            header.col(|ui| header_cell(ui, &self.sort, col, &mut messages));
                        }
                    })
                    .body(|body| {
                        body.rows(metrics.row_height, row_count, |mut row| {
                            let pos = row.index();
                            let Some(view) = self.row(pos, cx) else {
                                return;
                            };
                            let track = view.track;
                            row.set_selected(self.selection.is_selected(track.id));

                            row.col(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new((view.index + 1).to_string()).weak(),
                                    )
                                    .selectable(false),
                                );
                            });
                            let mut star_clicked = false;
                            row.col(|ui| {
                                if columns::star_cell(ui, track) {
                                    star_clicked = true;
                                    messages.push(TrackTableMsg::Context {
                                        row: pos,
                                        action: ContextAction::ToggleStar,
                                    });
                                }
                            });
                            row.col(|ui| {
                                columns::show_cell(
                                    ui,
                                    columns::ColumnId::Title,
                                    track,
                                    view.is_playing,
                                )
                            });
                            for col in columns::COLUMNS {
                                row.col(|ui| {
                                    columns::show_cell(ui, col.id, track, view.is_playing)
                                });
                            }

                            // A click on the star is that cell's toggle, not a
                            // row selection or a double-click-to-play.
                            let response = row.response();
                            if !star_clicked && response.clicked() {
                                let mods = response.ctx.input(|i| ClickModifiers {
                                    ctrl: i.modifiers.command,
                                    shift: i.modifiers.shift,
                                });
                                messages.push(TrackTableMsg::RowClicked { row: pos, mods });
                            }
                            if !star_clicked && response.double_clicked() {
                                messages.push(TrackTableMsg::RowActivated(pos));
                            }
                            if let Some(action) = context_menu::show(&response, track) {
                                messages.push(TrackTableMsg::Context { row: pos, action });
                            }
                        });
                    });
            },
        );

        for msg in messages {
            self.update(msg, cx, out);
        }

        properties::show(ui.ctx(), &mut self.properties);
    }
}

/// Renders the track Properties dialog for `track`, if any. Exposed so the
/// shell can show it from surfaces that don't embed a track table (e.g. the
/// now-playing panel's "Properties" link).
pub fn show_properties(ctx: &egui::Context, track: &mut Option<crate::library_api::TrackInfo>) {
    properties::show(ctx, track);
}

/// Minimum width the table can be laid out at without clipping columns: the
/// `#`/star/title minimums, every data column's minimum, and the gaps
/// between them.
fn min_table_width(spacing: f32) -> f32 {
    let data: f32 = columns::COLUMNS.iter().map(|col| col.min_width).sum();
    let gaps = spacing * (columns::COLUMNS.len() as f32 + 2.0);
    INDEX_COL_WIDTH + STAR_COL_WIDTH + columns::TITLE_MIN_WIDTH + data + gaps
}

/// Draws a sortable column header and records a header click as a message.
fn header_cell(
    ui: &mut egui::Ui,
    sort: &SortState,
    col: &columns::ColumnSpec,
    messages: &mut Vec<TrackTableMsg>,
) {
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
        messages.push(TrackTableMsg::HeaderClicked(col.id));
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
