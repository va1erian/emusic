//! A reusable virtual list of names with right-aligned count columns (#248),
//! ported to the portable [`ListView`]. Shared by the Artists and Genres views.
//!
//! The list is a model `ListView` over pre-built [`NameCountRow`]s, so
//! thousands of rows cost nothing until painted: the widget asks the model for
//! the text of the visible cells only. The caller decides which rows are shown
//! and when to rebuild them; this module owns the control and the count
//! columns.
//!
//! The original Win32 view opened a "Shuffle play" context menu on a row. The
//! portable context menu is #376's, so the same action is reachable by
//! double-clicking or pressing Enter on a row ([`Msg::NameCountShuffle`]).

use std::rc::Rc;

use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Fill, ListModel, ListView};

use crate::app::Msg;

/// A count column of a [`NameCountsView`]: its header and fixed width (in
/// design units). Counts are right-aligned.
pub struct CountColumn {
    /// The header label.
    pub title: &'static str,
    /// The column width in design units.
    pub width: f32,
}

/// One name+counts row, pre-formatted for the virtual list.
pub struct NameCountRow {
    name: String,
    counts: Vec<String>,
}

impl NameCountRow {
    /// Builds a row from its name and one already-formatted string per count
    /// column, in column order.
    pub fn new(name: impl Into<String>, counts: impl IntoIterator<Item = String>) -> Self {
        Self {
            name: name.into(),
            counts: counts.into_iter().collect(),
        }
    }

    /// The row's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The formatted count for `column`, or `""` past the end.
    pub fn count(&self, column: usize) -> &str {
        self.counts
            .as_slice()
            .get(column)
            .map_or("", String::as_str)
    }
}

/// The model: the rows in display order.
struct NameCountModel {
    rows: Rc<Vec<NameCountRow>>,
}

impl ListModel for NameCountModel {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let row = self.rows.get(row)?;
        Some(if column == 0 {
            row.name.as_str()
        } else {
            row.count(column - 1)
        })
    }
}

/// A virtual name+counts list whose activation raises [`Msg::NameCountShuffle`]
/// for the row.
pub struct NameCountsView {
    ui: Ui<Msg>,
    list: ListView<Msg>,
    rows: Rc<Vec<NameCountRow>>,
}

impl NameCountsView {
    /// Creates the list with a fill-width `name_title` column and the given
    /// right-aligned count columns.
    pub fn new(ui: &Ui<Msg>, name_title: &str, count_columns: &[CountColumn]) -> NameCountsView {
        let mut list = ListView::new(ui, Rect::default(), &[])
            .expect("create name-counts list")
            .column(name_title, Fill);
        for column in count_columns {
            list = list.column_right(column.title, dip(column.width));
        }
        let list = list
            .on_activate(|row| Some(Msg::NameCountShuffle(row)))
            .on_context(|row, at| Some(Msg::ContextRow(row, at)));

        NameCountsView {
            ui: ui.clone(),
            list,
            rows: Rc::new(Vec::new()),
        }
    }

    /// Moves/resizes the list.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.list.id(), rect)]);
    }

    /// Shows or hides the list.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.list.id(), visible);
    }

    /// Rebuilds the virtual list from `rows`.
    pub fn set_rows(&mut self, rows: Vec<NameCountRow>) {
        self.rows = Rc::new(rows);
        self.list.set_model(NameCountModel {
            rows: Rc::clone(&self.rows),
        });
    }

    /// The name of row `index`, if it exists.
    pub fn name(&self, index: usize) -> Option<String> {
        self.rows.get(index).map(|row| row.name.clone())
    }
}
